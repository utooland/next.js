use std::sync::{Arc, LazyLock};

use bytes::Bytes;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::info;
use wasm_bindgen::{JsCast, prelude::*};

use crate::worker_pool::{
    WorkerOptions,
    operation::{TaskMessage, WORKER_POOL_OPERATION},
};

pub enum WorkerOperation {
    Create(WebWorkerCreation, oneshot::Sender<u32>),
    Terminate(WebWorkerTermination),
}

static WORKER_CHANNEL: LazyLock<(
    mpsc::UnboundedSender<WorkerOperation>,
    std::sync::Mutex<Option<mpsc::UnboundedReceiver<WorkerOperation>>>,
)> = LazyLock::new(|| {
    let (tx, rx) = mpsc::unbounded_channel();
    (tx, std::sync::Mutex::new(Some(rx)))
});

static PENDING_CREATIONS: LazyLock<
    std::sync::Mutex<std::collections::VecDeque<oneshot::Sender<u32>>>,
> = LazyLock::new(|| std::sync::Mutex::new(std::collections::VecDeque::new()));

pub fn worker_created(worker_id: u32) {
    if let Some(tx) = PENDING_CREATIONS.lock().unwrap().pop_front() {
        let _ = tx.send(worker_id);
    }
}

pub async fn register_worker_scheduler(creator: js_sys::Function, terminator: js_sys::Function) {
    let mut rx_opt = WORKER_CHANNEL.1.lock().unwrap();
    let mut rx = if let Some(rx) = rx_opt.take() {
        rx
    } else {
        return;
    };

    while let Some(op) = rx.recv().await {
        match op {
            WorkerOperation::Create(creation, tx) => {
                let (created_tx, created_rx) = oneshot::channel();
                PENDING_CREATIONS.lock().unwrap().push_back(created_tx);

                if creator
                    .call1(&JsValue::NULL, &JsValue::from(creation))
                    .is_err()
                {
                    let _ = PENDING_CREATIONS.lock().unwrap().pop_back();
                    continue;
                }

                // Web workers can finish booting out of creation order. Wait for
                // workerCreated before accepting another Create, so the returned
                // worker id is paired with the WorkerOptions that requested it.
                if let Ok(worker_id) = created_rx.await {
                    let _ = tx.send(worker_id);
                }
            }
            WorkerOperation::Terminate(termination) => {
                let _ = terminator.call1(&JsValue::NULL, &JsValue::from(termination));
            }
        }
    }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct WebWorkerOptions {
    #[wasm_bindgen(getter_with_clone)]
    pub filename: String,
    #[wasm_bindgen(getter_with_clone)]
    pub cwd: String,
}

impl From<&WorkerOptions> for WebWorkerOptions {
    fn from(options: &WorkerOptions) -> Self {
        Self {
            filename: options.filename.to_string(),
            cwd: options.cwd.to_string(),
        }
    }
}

#[wasm_bindgen]
pub struct WebWorkerCreation {
    #[wasm_bindgen(getter_with_clone)]
    pub options: WebWorkerOptions,
}

#[wasm_bindgen]
pub struct WebWorkerTermination {
    #[wasm_bindgen(getter_with_clone)]
    pub options: WebWorkerOptions,
    #[wasm_bindgen(js_name = "workerId")]
    pub worker_id: u32,
}

pub async fn create_worker(options: Arc<WorkerOptions>) -> anyhow::Result<u32> {
    let (tx, rx) = oneshot::channel();

    let options_js = options.as_ref().into();

    let creation = WebWorkerCreation {
        options: options_js,
    };

    let sender = &WORKER_CHANNEL.0;

    sender
        .send(WorkerOperation::Create(creation, tx))
        .map_err(|_| anyhow::anyhow!("Worker scheduler closed"))?;

    let worker_id = rx.await?;

    Ok(worker_id)
}

pub fn terminate_worker(options: Arc<WorkerOptions>, worker_id: u32) {
    let options_js = options.as_ref().into();

    let termination = WebWorkerTermination {
        options: options_js,
        worker_id,
    };

    let sender = &WORKER_CHANNEL.0;
    let _ = sender.send(WorkerOperation::Terminate(termination));
}

#[wasm_bindgen]
pub struct WasmTaskMessage {
    #[wasm_bindgen(js_name = "taskId")]
    pub task_id: u32,
    data: js_sys::Uint8Array,
}

#[wasm_bindgen]
impl WasmTaskMessage {
    #[wasm_bindgen(getter)]
    pub fn data(&self) -> js_sys::Uint8Array {
        self.data.clone()
    }
}

impl From<TaskMessage> for WasmTaskMessage {
    fn from(msg: TaskMessage) -> Self {
        Self {
            task_id: msg.task_id,
            data: js_sys::Uint8Array::from(&msg.data[..]),
        }
    }
}

#[wasm_bindgen(js_name = "recvTaskMessageInWorker")]
pub async fn recv_task_message_in_worker(worker_id: u32) -> Result<WasmTaskMessage, JsError> {
    let (task_id, data) = WORKER_POOL_OPERATION
        .recv_task_message_in_worker(worker_id)
        .await
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(TaskMessage { task_id, data }.into())
}

#[wasm_bindgen(js_name = "sendTaskMessage")]
pub async fn send_task_message(message: JsValue) -> Result<(), JsError> {
    let task_id = js_sys::Reflect::get(&message, &"taskId".into())
        .map_err(|_| JsError::new("Failed to get taskId"))?
        .as_f64()
        .ok_or_else(|| JsError::new("taskId must be a number"))? as u32;

    let data_js = js_sys::Reflect::get(&message, &"data".into())
        .map_err(|_| JsError::new("Failed to get data"))?;

    let data = Bytes::from(js_sys::Uint8Array::new(&data_js).to_vec());

    WORKER_POOL_OPERATION
        .send_task_message(TaskMessage { task_id, data })
        .await
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(())
}
