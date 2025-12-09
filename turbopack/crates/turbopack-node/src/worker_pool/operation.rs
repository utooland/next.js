use std::{
    process::ExitStatus,
    sync::{Arc, LazyLock},
};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rustc_hash::FxHashMap;
use tokio::sync::{
    Mutex as AsyncMutex,
    mpsc::{self, UnboundedReceiver, UnboundedSender},
    oneshot,
};
use turbo_rcstr::RcStr;

use crate::{evaluate::Operation, pool_stats::NodeJsPoolStats, worker_pool::worker_thread};

#[derive(Clone)]
pub(crate) struct MessageChannel<T: Send + Sync + 'static> {
    sender: UnboundedSender<T>,
    receiver: Arc<AsyncMutex<UnboundedReceiver<T>>>,
}

impl<T: Send + Sync + 'static> MessageChannel<T> {
    pub(super) fn unbounded() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            sender,
            receiver: Arc::new(AsyncMutex::new(receiver)),
        }
    }
}

impl<T: Send + Sync + 'static> MessageChannel<T> {
    pub(crate) async fn send(&self, message: T) -> Result<()> {
        self.sender
            .send(message)
            .map_err(|_| anyhow::anyhow!("failed to send message"))
    }

    pub(crate) async fn recv(&self) -> Result<T> {
        let mut rx = self.receiver.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("failed to recv message"))
    }
}

pub(crate) struct PoolState {
    pub(crate) idle_workers: Mutex<Vec<u32>>,
    pub(crate) stats: Arc<Mutex<NodeJsPoolStats>>,
    pub(crate) waiters: Mutex<Vec<oneshot::Sender<u32>>>,
}

impl Default for PoolState {
    fn default() -> Self {
        Self {
            idle_workers: Mutex::new(Vec::new()),
            stats: Arc::new(Mutex::new(NodeJsPoolStats::default())),
            waiters: Mutex::new(Vec::new()),
        }
    }
}

#[turbo_tasks::value(cell = "new", serialization = "none", eq = "manual", shared)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct WorkerOptions {
    pub(super) filename: RcStr,
    pub(super) cwd: RcStr,
}

#[derive(Default)]
pub(crate) struct WorkerPoolOperation {
    #[allow(clippy::type_complexity)]
    worker_routed_channel: Mutex<FxHashMap<u32, Arc<MessageChannel<(u32, String)>>>>,
    #[allow(clippy::type_complexity)]
    task_routed_channel: Mutex<FxHashMap<u32, Arc<MessageChannel<String>>>>,
    pools: Mutex<FxHashMap<WorkerOptions, Arc<PoolState>>>,
}

impl WorkerPoolOperation {
    pub(crate) async fn get_pool_state(&self, worker_options: &WorkerOptions) -> Arc<PoolState> {
        self.pools
            .lock()
            .entry(worker_options.clone())
            .or_default()
            .clone()
    }

    pub(crate) fn scale_down(&self) -> Result<()> {
        let mut to_terminate = Vec::new();

        {
            let pools = self.pools.lock();
            for (worker_options, state) in pools.iter() {
                let mut idle = state.idle_workers.lock();
                if idle.len() > 1 {
                    let workers = idle.split_off(1);
                    let mut stats = state.stats.lock();
                    for worker_id in workers {
                        stats.remove_worker();
                        to_terminate.push((worker_options.clone(), worker_id));
                    }
                }
            }
        }

        to_terminate
            .into_iter()
            .map(|(worker_options, worker_id)| self.terminate_worker(worker_options, worker_id))
            .collect::<Result<Vec<_>>>()?;

        Ok(())
    }

    pub(crate) fn scale_zero(&self) -> Result<()> {
        let mut to_terminate = Vec::new();

        {
            let pools = self.pools.lock();
            for (worker_options, state) in pools.iter() {
                let mut idle = state.idle_workers.lock();
                let workers = std::mem::take(&mut *idle);
                let mut stats = state.stats.lock();
                for worker_id in workers {
                    stats.remove_worker();
                    to_terminate.push((worker_options.clone(), worker_id));
                }
            }
        }

        to_terminate
            .into_iter()
            .map(|(worker_options, worker_id)| self.terminate_worker(worker_options, worker_id))
            .collect::<Result<Vec<_>>>()?;

        Ok(())
    }

    pub(crate) async fn send_message_to_worker(
        &self,
        worker_id: u32,
        task_id: u32,
        message: String,
    ) -> Result<()> {
        let channel = {
            let mut map = self.worker_routed_channel.lock();
            map.entry(worker_id)
                .or_insert_with(|| Arc::new(MessageChannel::unbounded()))
                .clone()
        };
        channel
            .send((task_id, message))
            .await
            .context("failed to send message to worker")?;

        Ok(())
    }

    pub(crate) fn terminate_worker(
        &self,
        worker_options: WorkerOptions,
        worker_id: u32,
    ) -> Result<()> {
        self.worker_routed_channel.lock().remove(&worker_id);
        worker_thread::terminate_worker(worker_options, worker_id);
        Ok(())
    }

    pub async fn recv_task_message(&self, task_id: u32) -> Result<String> {
        let channel = {
            let mut map = self.task_routed_channel.lock();
            map.entry(task_id)
                .or_insert_with(|| Arc::new(MessageChannel::unbounded()))
                .clone()
        };
        let message = channel
            .recv()
            .await
            .context("failed to recv task message")?;
        Ok(message)
    }

    pub(crate) fn remove_task_channel(&self, task_id: u32) {
        self.task_routed_channel.lock().remove(&task_id);
    }

    pub(crate) async fn recv_message_in_worker(&self, worker_id: u32) -> Result<(u32, String)> {
        let channel = {
            let mut map = self.worker_routed_channel.lock();
            map.entry(worker_id)
                .or_insert_with(|| Arc::new(MessageChannel::unbounded()))
                .clone()
        };
        channel
            .recv()
            .await
            .with_context(|| format!("failed to recv message in worker {worker_id}"))
    }

    pub(crate) async fn send_task_message(&self, task_id: u32, message: String) -> Result<()> {
        let channel = {
            let mut map = self.task_routed_channel.lock();
            map.entry(task_id)
                .or_insert_with(|| Arc::new(MessageChannel::unbounded()))
                .clone()
        };
        channel
            .send(message)
            .await
            .with_context(|| format!("failed to send  response for task {task_id}"))
    }
}

pub(crate) static WORKER_POOL_OPERATION: LazyLock<WorkerPoolOperation> =
    LazyLock::new(WorkerPoolOperation::default);

pub(crate) async fn send_message_to_worker(
    worker_id: u32,
    task_id: u32,
    message: String,
) -> Result<()> {
    WORKER_POOL_OPERATION
        .send_message_to_worker(worker_id, task_id, message)
        .await
}

pub(crate) fn terminate_worker(worker_options: WorkerOptions, worker_id: u32) -> Result<()> {
    WORKER_POOL_OPERATION.terminate_worker(worker_options, worker_id)
}

pub(crate) async fn recv_task_message(task_id: u32) -> Result<String> {
    WORKER_POOL_OPERATION.recv_task_message(task_id).await
}

pub(crate) fn remove_task_channel(task_id: u32) {
    WORKER_POOL_OPERATION.remove_task_channel(task_id)
}

pub(crate) async fn get_pool_state(worker_options: &WorkerOptions) -> Arc<PoolState> {
    WORKER_POOL_OPERATION.get_pool_state(worker_options).await
}

pub(crate) struct WorkerOperation {
    pub(crate) worker_options: WorkerOptions,
    pub(crate) task_id: u32,
    pub(crate) worker_id: u32,
    pub(crate) state: Arc<PoolState>,
    pub(crate) on_drop: Option<Box<dyn FnOnce(u32) + Send + Sync>>,
}

impl Drop for WorkerOperation {
    fn drop(&mut self) {
        if let Some(on_drop) = self.on_drop.take() {
            on_drop(self.worker_id);
        }
        remove_task_channel(self.task_id);
    }
}

#[async_trait::async_trait]
impl Operation for WorkerOperation {
    async fn recv(&mut self) -> Result<String> {
        recv_task_message(self.task_id).await
    }

    async fn send(&mut self, message: String) -> Result<()> {
        send_message_to_worker(self.worker_id, self.task_id, message).await
    }

    async fn wait_or_kill(&mut self) -> Result<ExitStatus> {
        if self.on_drop.is_some() {
            self.state.stats.lock().remove_worker();
            self.on_drop = None;
        }
        terminate_worker(self.worker_options.clone(), self.worker_id)?;
        Ok(ExitStatus::default())
    }

    fn disallow_reuse(&mut self) {
        if self.on_drop.is_some() {
            self.state.stats.lock().remove_worker();
            self.on_drop = None;
        }
    }
}
