use std::{collections::VecDeque, sync::Arc};

use napi::{
    Env,
    threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode},
};
use napi_derive::napi;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use tokio::sync::oneshot;
use turbo_rcstr::RcStr;

use crate::worker_pool::{
    WorkerOptions,
    operation::{TaskMessage, WORKER_POOL_OPERATION},
};

static WORKER_CREATOR: OnceCell<ThreadsafeFunction<NapiWorkerCreation, ErrorStrategy::Fatal>> =
    OnceCell::new();

static WORKER_TERMINATOR: OnceCell<
    ThreadsafeFunction<NapiWorkerTermination, ErrorStrategy::Fatal>,
> = OnceCell::new();

static PENDING_CREATIONS: OnceCell<Mutex<VecDeque<(Arc<WorkerOptions>, oneshot::Sender<u32>)>>> =
    OnceCell::new();

#[napi]
#[allow(unused)]
pub fn register_worker_scheduler(
    env: Env,
    creator: ThreadsafeFunction<NapiWorkerCreation, ErrorStrategy::Fatal>,
    terminator: ThreadsafeFunction<NapiWorkerTermination, ErrorStrategy::Fatal>,
) -> napi::Result<()> {
    // Unref ThreadsafeFunction so it doesn't keep the Node.js event loop alive.
    // Call unref on the functions before storing them globally.
    let creator_unrefed = {
        let mut c = creator;
        // Safe to call unref; if the napi crate provides this method it will drop the ref
        // preventing the ThreadsafeFunction from keeping the loop alive.
        let _ = c.unref(&env);
        c
    };
    let terminator_unrefed = {
        let mut t = terminator;
        let _ = t.unref(&env);
        t
    };

    WORKER_CREATOR.get_or_init(|| creator_unrefed);
    WORKER_TERMINATOR.get_or_init(|| terminator_unrefed);

    Ok(())
}

pub async fn create_worker(options: Arc<WorkerOptions>) -> anyhow::Result<u32> {
    let (tx, rx) = oneshot::channel();

    {
        let pending = PENDING_CREATIONS.get_or_init(|| Mutex::new(VecDeque::new()));
        // ensure pool entry exists for these options so scale ops can observe it
        WORKER_POOL_OPERATION
            .pools
            .lock()
            .entry(options.clone())
            .or_default();
        pending.lock().push_back((options.clone(), tx));
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
    if let Some(pending) = PENDING_CREATIONS.get() {
        if let Some((options, tx)) = pending.lock().pop_front() {
            // record into global pool
            WORKER_POOL_OPERATION
                .pools
                .lock()
                .entry(options.clone())
                .or_default()
                .idle_workers
                .lock()
                .push(worker_id);
            let _ = tx.send(worker_id);
        }
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
pub struct NapiWorkerCreation {
    pub options: NapiWorkerOptions,
}

#[napi(object)]
#[allow(unused)]
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
pub struct NapiTaskMessage {
    pub task_id: u32,
    pub data: String,
}

impl From<NapiTaskMessage> for TaskMessage {
    fn from(message: NapiTaskMessage) -> Self {
        let NapiTaskMessage { task_id, data } = message;
        TaskMessage { task_id, data }
    }
}

#[napi]
#[allow(unused)]
// TODO: use zero-copy externaled type array
pub async fn recv_task_message_in_worker(worker_id: u32) -> napi::Result<NapiTaskMessage> {
    let (task_id, message) = WORKER_POOL_OPERATION
        .recv_task_message_in_worker(worker_id)
        .await?;
    Ok(NapiTaskMessage {
        task_id,
        data: message,
    })
}

#[napi]
#[allow(unused)]
pub async fn send_task_message(message: NapiTaskMessage) -> napi::Result<()> {
    Ok(WORKER_POOL_OPERATION
        .send_task_message(message.into())
        .await?)
}
