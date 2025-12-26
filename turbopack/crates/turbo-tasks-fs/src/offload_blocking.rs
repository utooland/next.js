use std::{
    io::{self, ErrorKind},
    sync::{Arc, LazyLock},
};

use parking_lot::Mutex;
use tokio::sync::{
    mpsc::{Sender, channel},
    oneshot,
};

const MAX_RETRY_ATTEMPTS: usize = 10;
const MAX_BATCH_SIZE: usize = 4;

type Job = Box<dyn FnOnce() + Send + Sync>;

static BATCHER: Mutex<Option<Sender<Job>>> = Mutex::new(None);

// Use an individual tokio runtime to offload IO tasks from turbo_tasks
static BATCHER_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create batcher runtime")
});

fn get_batcher_sender() -> Sender<Job> {
    let mut guard = BATCHER.lock();
    if let Some(tx) = guard.as_ref()
        && !tx.is_closed()
    {
        return tx.clone();
    }

    let (tx, mut rx) = channel(MAX_BATCH_SIZE);
    let tx_ret = tx.clone();
    *guard = Some(tx);

    BATCHER_RUNTIME.spawn(async move {
        let mut batch: Vec<Job> = Vec::with_capacity(MAX_BATCH_SIZE);
        loop {
            let count = rx.recv_many(&mut batch, MAX_BATCH_SIZE).await;
            if count == 0 {
                break;
            }

            let current_batch = std::mem::replace(&mut batch, Vec::with_capacity(MAX_BATCH_SIZE));
            // Use tokio::task::spawn_blocking instead of turbo_tasks::spawn_blocking
            // because we are in a background task without turbo-tasks context.
            tokio::task::spawn_blocking(move || {
                for job in current_batch {
                    job();
                }
            });
        }
    });

    tx_ret
}

pub(crate) async fn offload_blocking<I, T, R, F>(input: I, func: F) -> io::Result<R>
where
    I: Into<T>,
    T: Send + Sync + 'static,
    F: Fn(&T) -> io::Result<R> + Send + Sync + 'static,
    R: Send + 'static,
{
    let arg: Arc<T> = Arc::new(input.into());
    let func = Arc::new(func);
    let mut attempt = 0;

    loop {
        attempt += 1;
        let (tx, rx) = oneshot::channel();
        let func = func.clone();
        let arg_cloned = arg.clone();

        let job = Box::new(move || {
            let result = func(&arg_cloned);
            let _ = tx.send(result);
        });

        get_batcher_sender()
            .send(job)
            .await
            .expect("Batcher channel closed");

        match rx.await {
            Ok(Ok(r)) => return Ok(r),
            Ok(Err(err)) => {
                if attempt >= MAX_RETRY_ATTEMPTS || !can_retry(&err) {
                    return Err(err);
                }
            }
            Err(_) => {
                // The batcher task might have been dropped (e.g. because the runtime it was spawned
                // on shut down). We can just retry in this case.
                continue;
            }
        }
    }
}

fn can_retry(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::PermissionDenied | ErrorKind::WouldBlock
    )
}
