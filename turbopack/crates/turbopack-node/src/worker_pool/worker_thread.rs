use napi_derive::napi;

use crate::worker_pool::operation::WORKER_POOL_OPERATION;

#[napi(object)]
#[allow(unused)]
pub struct PoolOptions {
    pub filename: String,
    pub concurrency: u32,
}

#[napi]
#[allow(unused)]
pub fn recv_pool_creation() -> Option<PoolOptions> {
    WORKER_POOL_OPERATION
        .try_recv_pool_creation()
        .map(|(filename, concurrency)| PoolOptions {
            filename,
            concurrency: concurrency as u32,
        })
}

#[napi]
#[allow(unused)]
pub async fn recv_worker_request(pool_id: String) -> napi::Result<String> {
    WORKER_POOL_OPERATION
        .recv_worker_request(pool_id)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
// TODO: use zero-copy externaled type array
#[allow(unused)]
pub async fn recv_message_in_worker(worker_id: u32) -> napi::Result<String> {
    Ok(WORKER_POOL_OPERATION
        .recv_message_in_worker(worker_id)
        .await?)
}

#[napi]
#[allow(unused)]
pub async fn notify_one_worker_created(filename: String) -> napi::Result<()> {
    WORKER_POOL_OPERATION
        .notify_one_worker_created(filename)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
#[allow(unused)]
pub async fn notify_worker_ack(task_id: String, worker_id: u32) -> napi::Result<()> {
    WORKER_POOL_OPERATION
        .notify_worker_ack(task_id, worker_id)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
#[allow(unused)]
pub async fn send_task_message(task_id: String, message: String) -> napi::Result<()> {
    WORKER_POOL_OPERATION
        .send_task_message(task_id, message)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}
