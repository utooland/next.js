use napi_derive::napi;

use crate::worker_pool::operation::WORKER_POOL_OPERATION;

#[napi(object)]
pub struct PoolOptions {
    pub filename: String,
    pub concurrency: u32,
}

#[napi]
pub async fn recv_pool_creation() -> napi::Result<PoolOptions> {
    let (filename, concurrency) = WORKER_POOL_OPERATION.recv_pool_creation().await?;

    Ok(PoolOptions {
        filename,
        concurrency: concurrency as u32,
    })
}

#[napi]
pub async fn recv_worker_request(pool_id: String) -> napi::Result<u32> {
    Ok(WORKER_POOL_OPERATION.recv_worker_request(pool_id).await?)
}

#[napi]
// TODO: use zero-copy externaled type array
pub async fn recv_message_in_worker(worker_id: u32) -> napi::Result<String> {
    Ok(WORKER_POOL_OPERATION
        .recv_message_in_worker(worker_id)
        .await?)
}

#[napi]
pub async fn notify_one_worker_created(filename: String) -> napi::Result<()> {
    Ok(WORKER_POOL_OPERATION
        .notify_one_worker_created(filename)
        .await?)
}

#[napi]
pub async fn notify_worker_ack(task_id: u32, worker_id: u32) -> napi::Result<()> {
    Ok(WORKER_POOL_OPERATION
        .notify_worker_ack(task_id, worker_id)
        .await?)
}

#[napi]
pub async fn send_task_message(task_id: u32, message: String) -> napi::Result<()> {
    Ok(WORKER_POOL_OPERATION
        .send_task_message(task_id, message)
        .await?)
}
