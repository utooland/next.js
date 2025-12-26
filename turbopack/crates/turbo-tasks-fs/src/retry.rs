use std::{
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    sync::LazyLock,
    thread::sleep,
    time::Duration,
};

use rayon::ThreadPoolBuilder;

const MAX_RETRY_ATTEMPTS: usize = 10;

static FS_THREAD_POOL: LazyLock<rayon::ThreadPool> = LazyLock::new(|| {
    ThreadPoolBuilder::new()
        .num_threads(512)
        .thread_name(|i| format!("turbo-fs-io-{}", i))
        .build()
        .expect("Failed to create FS thread pool")
});

pub(crate) async fn retry_blocking<R, F>(path: PathBuf, func: F) -> io::Result<R>
where
    F: Fn(&Path) -> io::Result<R> + Send + 'static,
    R: Send + 'static,
{
    let span = tracing::Span::current();
    let (tx, rx) = tokio::sync::oneshot::channel();

    FS_THREAD_POOL.spawn(move || {
        let _guard = span.entered();
        let mut attempt = 1;

        let res = loop {
            match func(&path) {
                Ok(r) => break Ok(r),
                Err(err) => {
                    if attempt < MAX_RETRY_ATTEMPTS && can_retry(&err) {
                        sleep(get_retry_wait_time(attempt));
                        attempt += 1;
                        continue;
                    }

                    break Err(err);
                }
            };
        };
        let _ = tx.send(res);
    });

    rx.await
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "FS task cancelled"))?
}

fn can_retry(err: &io::Error) -> bool {
    if matches!(
        err.kind(),
        ErrorKind::PermissionDenied | ErrorKind::WouldBlock
    ) {
        return true;
    }
    #[cfg(unix)]
    if let Some(code) = err.raw_os_error() {
        // EMFILE = 24, ENFILE = 23
        if code == 24 || code == 23 {
            return true;
        }
    }
    false
}

fn get_retry_wait_time(attempt: usize) -> Duration {
    Duration::from_millis((attempt as u64) * 100)
}
