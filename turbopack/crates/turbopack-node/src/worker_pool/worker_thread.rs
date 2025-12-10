use std::{collections::VecDeque, sync::Arc};

use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi_derive::napi;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use tokio::sync::oneshot;
use turbo_rcstr::RcStr;

use crate::worker_pool::{WorkerOptions, operation::WORKER_POOL_OPERATION};

static WORKER_CREATOR: OnceCell<ThreadsafeFunction<NapiWorkerCreation, ErrorStrategy::Fatal>> =
    OnceCell::new();

static WORKER_TERMINATOR: OnceCell<
    ThreadsafeFunction<NapiWorkerTermination, ErrorStrategy::Fatal>,
> = OnceCell::new();

static PENDING_CREATIONS: OnceCell<Mutex<VecDeque<oneshot::Sender<u32>>>> = OnceCell::new();

#[napi]
#[allow(unused)]
pub fn register_worker_scheduler(
    creator: ThreadsafeFunction<NapiWorkerCreation, ErrorStrategy::Fatal>,
    terminator: ThreadsafeFunction<NapiWorkerTermination, ErrorStrategy::Fatal>,
) -> napi::Result<()> {
    WORKER_CREATOR
        .set(creator)
        .map_err(|_| napi::Error::from_reason("Worker creator already registered"))?;
    WORKER_TERMINATOR
        .set(terminator)
        .map_err(|_| napi::Error::from_reason("Worker terminator already registered"))
}

pub async fn create_worker(options: Arc<WorkerOptions>, _task_id: u32) -> anyhow::Result<u32> {
    let (tx, rx) = oneshot::channel();

    {
        let pending = PENDING_CREATIONS.get_or_init(|| Mutex::new(VecDeque::new()));
        pending.lock().push_back(tx);
    }

    if let Some(creator) = WORKER_CREATOR.get() {
        creator.call(
            NapiWorkerCreation {
                options: options.into(),
            },
            ThreadsafeFunctionCallMode::NonBlocking,
        );
    } else {
        return Err(anyhow::anyhow!("Worker creator not registered"));
    }

    let worker_id = rx.await?;
    Ok(worker_id)
}

#[napi]
#[allow(unused)]
pub fn worker_created(worker_id: u32) {
    if let Some(pending) = PENDING_CREATIONS.get()
        && let Some(tx) = pending.lock().pop_front()
    {
        let _ = tx.send(worker_id);
    }
}

pub fn terminate_worker(options: Arc<WorkerOptions>, worker_id: u32) {
    if let Some(terminator) = WORKER_TERMINATOR.get() {
        terminator.call(
            NapiWorkerTermination {
                options: options.into(),
                worker_id,
            },
            ThreadsafeFunctionCallMode::NonBlocking,
        );
    }
}

#[napi(object)]
#[allow(unused)]
#[derive(Clone)]
pub struct NapiWorkerCreation {
    pub options: NapiWorkerOptions,
}

#[napi(object)]
#[allow(unused)]
#[derive(Clone)]
pub struct NapiWorkerOptions {
    pub filename: RcStr,
    pub cwd: RcStr,
}

impl<T> From<T> for NapiWorkerOptions
where
    T: AsRef<WorkerOptions>,
{
    fn from(pool_options: T) -> Self {
        let WorkerOptions { filename, cwd } = pool_options.as_ref();
        NapiWorkerOptions {
            filename: filename.clone(),
            cwd: cwd.clone(),
        }
    }
}

#[napi(object)]
#[allow(unused)]
pub struct NapiWorkerTermination {
    pub options: NapiWorkerOptions,
    pub worker_id: u32,
}

#[napi(object)]
#[allow(unused)]
pub struct WorkerMessage {
    pub task_id: u32,
    pub message: String,
}

#[napi]
#[allow(unused)]
// TODO: use zero-copy externaled type array
pub async fn recv_task_message_in_worker(worker_id: u32) -> napi::Result<WorkerMessage> {
    let (task_id, message) = WORKER_POOL_OPERATION
        .recv_task_message_in_worker(worker_id)
        .await?;
    Ok(WorkerMessage { task_id, message })
}

#[napi]
#[allow(unused)]
pub async fn send_task_message(task_id: u32, message: String) -> napi::Result<()> {
    Ok(WORKER_POOL_OPERATION
        .send_task_message(task_id, message)
        .await?)
}
