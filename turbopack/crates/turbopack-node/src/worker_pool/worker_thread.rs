use std::sync::Arc;

use napi_derive::napi;
use rustc_hash::FxHashMap;
use turbo_rcstr::RcStr;

use crate::worker_pool::{PoolOptions, operation::WORKER_POOL_OPERATION};

#[napi(object)]
#[allow(unused)]
pub struct NapiPoolOptions {
    pub filename: RcStr,
    pub concurrency: u32,
    pub env: Arc<FxHashMap<RcStr, RcStr>>,
    pub cwd: RcStr,
}

impl From<PoolOptions> for NapiPoolOptions {
    fn from(pool_options: PoolOptions) -> Self {
        let PoolOptions {
            filename,
            concurrency,
            env,
            cwd,
        } = pool_options;
        NapiPoolOptions {
            filename,
            concurrency,
            env,
            cwd,
        }
    }
}

#[napi(object)]
#[allow(unused)]
pub struct WorkerTermination {
    pub filename: RcStr,
    pub worker_id: u32,
}

#[napi]
#[allow(unused)]
pub async fn recv_pool_request() -> napi::Result<NapiPoolOptions> {
    let pool_options = WORKER_POOL_OPERATION.recv_pool_request().await?;

    Ok(pool_options.into())
}

#[napi]
#[allow(unused)]
pub async fn recv_worker_termination() -> napi::Result<WorkerTermination> {
    let (filename, worker_id) = WORKER_POOL_OPERATION.recv_worker_termination().await?;
    Ok(WorkerTermination {
        filename,
        worker_id,
    })
}

#[napi]
#[allow(unused)]
pub async fn recv_worker_request(filename: RcStr) -> napi::Result<u32> {
    Ok(WORKER_POOL_OPERATION.recv_worker_request(filename).await?)
}

#[napi]
#[allow(unused)]
// TODO: use zero-copy externaled type array
pub async fn recv_message_in_worker(worker_id: u32) -> napi::Result<String> {
    Ok(WORKER_POOL_OPERATION
        .recv_message_in_worker(worker_id)
        .await?)
}

#[napi]
#[allow(unused)]
pub async fn notify_worker_ack(task_id: u32, worker_id: u32) -> napi::Result<()> {
    Ok(WORKER_POOL_OPERATION
        .notify_worker_ack(task_id, worker_id)
        .await?)
}

#[napi]
#[allow(unused)]
pub async fn send_task_message(task_id: u32, message: String) -> napi::Result<()> {
    Ok(WORKER_POOL_OPERATION
        .send_task_message(task_id, message)
        .await?)
}
