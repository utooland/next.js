use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi_derive::napi;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use rustc_hash::FxHashMap;
use tokio::sync::oneshot;
use turbo_rcstr::RcStr;

use crate::worker_pool::{PoolOptions, operation::WORKER_POOL_OPERATION};

static WORKER_CREATOR: OnceCell<ThreadsafeFunction<WorkerCreationParams, ErrorStrategy::Fatal>> =
    OnceCell::new();

static WORKER_TERMINATOR: OnceCell<ThreadsafeFunction<WorkerTermination, ErrorStrategy::Fatal>> =
    OnceCell::new();

static PENDING_CREATIONS: OnceCell<Mutex<FxHashMap<u32, oneshot::Sender<u32>>>> = OnceCell::new();

#[napi]
#[allow(unused)]
pub fn register_worker_creator(
    callback: ThreadsafeFunction<WorkerCreationParams, ErrorStrategy::Fatal>,
) -> napi::Result<()> {
    WORKER_CREATOR
        .set(callback)
        .map_err(|_| napi::Error::from_reason("Worker creator already registered"))
}

#[napi]
#[allow(unused)]
pub fn register_worker_terminator(
    callback: ThreadsafeFunction<WorkerTermination, ErrorStrategy::Fatal>,
) -> napi::Result<()> {
    WORKER_TERMINATOR
        .set(callback)
        .map_err(|_| napi::Error::from_reason("Worker terminator already registered"))
}

pub async fn create_worker(options: NapiPoolOptions, task_id: u32) -> anyhow::Result<u32> {
    let (tx, rx) = oneshot::channel();

    {
        let pending = PENDING_CREATIONS.get_or_init(|| Mutex::new(FxHashMap::default()));
        pending.lock().insert(task_id, tx);
    }

    if let Some(creator) = WORKER_CREATOR.get() {
        creator.call(
            WorkerCreationParams { options, task_id },
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
pub fn worker_created(task_id: u32, worker_id: u32) {
    if let Some(pending) = PENDING_CREATIONS.get()
        && let Some(tx) = pending.lock().remove(&task_id)
    {
        let _ = tx.send(worker_id);
    }
}

pub fn terminate_worker(pool_id: RcStr, worker_id: u32) {
    if let Some(terminator) = WORKER_TERMINATOR.get() {
        terminator.call(
            WorkerTermination {
                filename: pool_id,
                worker_id,
            },
            ThreadsafeFunctionCallMode::NonBlocking,
        );
    }
}

#[napi(object)]
#[allow(unused)]
#[derive(Clone)]
pub struct NapiPoolOptions {
    pub filename: RcStr,
    pub cwd: RcStr,
}

#[napi(object)]
#[derive(Clone)]
pub struct WorkerCreationParams {
    pub options: NapiPoolOptions,
    pub task_id: u32,
}

impl From<PoolOptions> for NapiPoolOptions {
    fn from(pool_options: PoolOptions) -> Self {
        let PoolOptions { filename, cwd } = pool_options;
        NapiPoolOptions { filename, cwd }
    }
}

#[napi(object)]
#[allow(unused)]
pub struct WorkerTermination {
    pub filename: RcStr,
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
pub async fn recv_message_in_worker(worker_id: u32) -> napi::Result<WorkerMessage> {
    let (task_id, message) = WORKER_POOL_OPERATION
        .recv_message_in_worker(worker_id)
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
