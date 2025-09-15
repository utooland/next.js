use wasm_bindgen::prelude::*;

use crate::worker_pool::operation::WORKER_POOL_OPERATION;

#[wasm_bindgen]
pub struct PoolOptions {
    #[wasm_bindgen(getter_with_clone)]
    pub filename: String,
    #[wasm_bindgen(js_name = "maxConcurrency")]
    pub max_concurrency: u32,
}

#[wasm_bindgen]
pub struct WorkerTermination {
    #[wasm_bindgen(getter_with_clone)]
    pub filename: String,
    #[wasm_bindgen]
    pub worker_id: u32,
}

#[wasm_bindgen(js_name = "recvPoolRequest")]
pub async fn recv_pool_request() -> Result<PoolOptions, JsError> {
    let (filename, concurrency) = WORKER_POOL_OPERATION
        .recv_pool_request()
        .await
        .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(PoolOptions {
        filename,
        max_concurrency: concurrency as u32,
    })
}

#[wasm_bindgen(js_name = "recvWorkerTermination")]
pub async fn recv_worker_termination() -> Result<WorkerTermination, JsError> {
    let (filename, worker_id) = WORKER_POOL_OPERATION
        .recv_worker_termination()
        .await
        .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(WorkerTermination {
        filename,
        worker_id,
    })
}

#[wasm_bindgen(js_name = "recvWorkerRequest")]
pub async fn recv_worker_request(pool_id: String) -> Result<u32, JsError> {
    WORKER_POOL_OPERATION
        .recv_worker_request(pool_id)
        .await
        .map_err(|e| JsError::new(&e.to_string()))
}

// TODO: use zero-copy externaled type array
#[wasm_bindgen(js_name = "recvMessageInWorker")]
pub async fn recv_message_in_worker(worker_id: u32) -> Result<String, JsError> {
    WORKER_POOL_OPERATION
        .recv_message_in_worker(worker_id)
        .await
        .map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen(js_name = "notifyWorkerAck")]
pub async fn notify_worker_ack(task_id: u32, worker_id: u32) -> Result<(), JsError> {
    WORKER_POOL_OPERATION
        .notify_worker_ack(task_id, worker_id)
        .await
        .map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen(js_name = "sendTaskMessage")]
pub async fn send_task_message(task_id: u32, message: String) -> Result<(), JsError> {
    WORKER_POOL_OPERATION
        .send_task_message(task_id, message)
        .await
        .map_err(|e| JsError::new(&e.to_string()))
}
