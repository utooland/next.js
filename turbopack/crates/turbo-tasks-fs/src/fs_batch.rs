use std::{
    io::{self, ErrorKind},
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    sync::Arc,
};

use parking_lot::Mutex;
use tokio::sync::{
    mpsc::{UnboundedSender, unbounded_channel},
    oneshot,
};

const MAX_RETRY_ATTEMPTS: usize = 10;

type Job = Box<dyn FnOnce() + Send>;

static BATCHER: Mutex<Option<UnboundedSender<Job>>> = Mutex::new(None);

fn get_batcher_sender() -> UnboundedSender<Job> {
    let mut guard = BATCHER.lock();
    if let Some(tx) = guard.as_ref()
        && !tx.is_closed()
    {
        return tx.clone();
    }

    let (tx, mut rx) = unbounded_channel();
    let tx_ret = tx.clone();
    *guard = Some(tx);

    tokio::spawn(async move {
        let mut batch: Vec<Job> = Vec::new();
        loop {
            let job = rx.recv().await;
            match job {
                Some(job) => {
                    batch.push(job);
                    // Drain available items up to a limit
                    while batch.len() < 100 {
                        match rx.try_recv() {
                            Ok(job) => batch.push(job),
                            Err(_) => break,
                        }
                    }
                }
                None => break,
            }

            if !batch.is_empty() {
                let current_batch = std::mem::take(&mut batch);
                // Use tokio::task::spawn_blocking instead of turbo_tasks::spawn_blocking
                // because we are in a background task without turbo-tasks context.
                let _ = tokio::task::spawn_blocking(move || {
                    for job in current_batch {
                        let _ = catch_unwind(AssertUnwindSafe(job));
                    }
                })
                .await;
            }
        }
    });

    tx_ret
}

pub(crate) async fn retry_blocking<R, F>(path: PathBuf, func: F) -> io::Result<R>
where
    F: Fn(&Path) -> io::Result<R> + Send + Sync + 'static,
    R: Send + 'static,
{
    let func = Arc::new(func);
    let mut attempt = 0;

    loop {
        attempt += 1;
        let (tx, rx) = oneshot::channel();
        let func = func.clone();
        let path_cloned = path.clone();

        let job = Box::new(move || {
            let result = catch_unwind(AssertUnwindSafe(|| func(&path_cloned)));
            let result = result.map_err(|e| {
                if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                }
            });
            let _ = tx.send(result);
        });

        get_batcher_sender()
            .send(job)
            .expect("Batcher channel closed");

        match rx.await {
            Ok(Ok(Ok(r))) => return Ok(r),
            Ok(Ok(Err(err))) => {
                if attempt >= MAX_RETRY_ATTEMPTS || !can_retry(&err) {
                    return Err(err);
                }
            }
            Ok(Err(msg)) => panic!("{}", msg),
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
