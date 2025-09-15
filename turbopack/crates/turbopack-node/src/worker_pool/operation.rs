use std::{process::ExitStatus, sync::LazyLock};

use anyhow::{Context, Result};
use async_channel::{Receiver, Sender, unbounded};
use dashmap::DashMap;

use crate::evaluate::Operation;

#[derive(Clone)]
pub(crate) struct MessageChannel<T: Send + Sync + 'static> {
    sender: Sender<T>,
    receiver: Receiver<T>,
}

impl<T: Send + Sync + 'static> MessageChannel<T> {
    pub(super) fn unbounded() -> Self {
        let (sender, receiver) = unbounded::<T>();
        Self { sender, receiver }
    }
}

impl<T: Send + Sync + 'static> MessageChannel<T> {
    pub(crate) async fn send(&self, data: T) -> Result<()> {
        Ok(self.sender.send(data).await?)
    }

    pub(crate) async fn recv(&self) -> Result<T> {
        Ok(self.receiver.recv().await?)
    }

    pub(crate) fn close(&self) {
        self.sender.close();
        self.receiver.close();
    }
}

pub(crate) struct WorkerPoolOperation {
    pool_request_channel: MessageChannel<(String, usize)>,
    worker_termination_channel: MessageChannel<(String, u32)>,
    worker_request_channel: DashMap<String, MessageChannel<u32>>,
    worker_ack_channel: DashMap<u32, MessageChannel<u32>>,
    worker_routed_channel: DashMap<u32, MessageChannel<String>>,
    task_routed_channel: DashMap<u32, MessageChannel<String>>,
}

impl Default for WorkerPoolOperation {
    fn default() -> Self {
        Self {
            pool_request_channel: MessageChannel::unbounded(),
            worker_termination_channel: MessageChannel::unbounded(),
            worker_request_channel: DashMap::new(),
            worker_ack_channel: DashMap::new(),
            worker_routed_channel: DashMap::new(),
            task_routed_channel: DashMap::new(),
        }
    }
}

impl WorkerPoolOperation {
    pub(crate) async fn create_or_scale_pool(
        &self,
        filename: String,
        max_concurrency: usize,
    ) -> Result<()> {
        self.pool_request_channel
            .send((filename.clone(), max_concurrency))
            .await
            .context("failed to send pool request")?;

        Ok(())
    }

    pub(crate) async fn connect_to_worker(&self, pool_id: String, task_id: u32) -> Result<u32> {
        let channel = self
            .worker_request_channel
            .entry(pool_id.clone())
            .or_insert_with(MessageChannel::unbounded)
            .clone();
        channel
            .send(task_id)
            .await
            .context("failed to send worker request")?;
        let worker_id = async move {
            let channel = self
                .worker_ack_channel
                .entry(task_id)
                .or_insert_with(MessageChannel::unbounded)
                .clone();
            channel.recv().await.context("failed to recv worker ack")
        }
        .await?;
        Ok(worker_id)
    }

    pub(crate) async fn send_worker_termination(
        &self,
        pool_id: String,
        worker_id: u32,
    ) -> Result<()> {
        self.worker_termination_channel
            .send((pool_id, worker_id))
            .await
            .context("failed to send worker termination")
    }

    pub(crate) async fn recv_worker_termination(&self) -> Result<(String, u32)> {
        self.worker_termination_channel
            .recv()
            .await
            .context("failed to recv worker termination")
    }

    pub(crate) async fn send_message_to_worker(&self, worker_id: u32, data: String) -> Result<()> {
        let channel = self
            .worker_routed_channel
            .entry(worker_id)
            .or_insert_with(MessageChannel::unbounded)
            .clone();
        channel
            .send(data)
            .await
            .context("failed to send message to worker")?;
        Ok(())
    }

    pub async fn recv_task_response(&self, task_id: u32) -> Result<String> {
        let channel = self
            .task_routed_channel
            .entry(task_id)
            .or_insert_with(MessageChannel::unbounded)
            .clone();
        let data = channel
            .recv()
            .await
            .context("failed to recv task message")?;
        Ok(data)
    }

    pub(crate) async fn recv_pool_request(&self) -> Result<(String, usize)> {
        self.pool_request_channel
            .recv()
            .await
            .context("failed to recv pool request")
    }

    pub(crate) fn shutdown(&self) {
        // We need to close channels connected to schedule thread,
        // or else, it will be forever waiting in schedule thread
        self.pool_request_channel.close();
        self.worker_termination_channel.close();
    }

    pub(crate) async fn recv_worker_request(&self, pool_id: String) -> Result<u32> {
        let channel = self
            .worker_request_channel
            .entry(pool_id.clone())
            .or_insert_with(MessageChannel::unbounded)
            .clone();
        channel
            .recv()
            .await
            .context("failed to recv worker request")
    }

    pub(crate) async fn notify_worker_ack(&self, task_id: u32, worker_id: u32) -> Result<()> {
        let channel = self
            .worker_ack_channel
            .get(&task_id)
            .with_context(|| format!("worker ack channel for {task_id} not found"))?;
        channel
            .send(worker_id)
            .await
            .context("failed to notify worker ack")
    }

    pub(crate) async fn recv_message_in_worker(&self, worker_id: u32) -> Result<String> {
        let channel = self
            .worker_routed_channel
            .entry(worker_id)
            .or_insert_with(MessageChannel::unbounded)
            .clone();
        channel
            .recv()
            .await
            .with_context(|| format!("failed to recv message in worker {worker_id}"))
    }

    pub(crate) async fn send_task_message(&self, task_id: u32, data: String) -> Result<()> {
        let channel = self
            .task_routed_channel
            .entry(task_id)
            .or_insert_with(MessageChannel::unbounded)
            .clone();
        channel
            .send(data)
            .await
            .with_context(|| format!("failed to send  response for task {task_id}"))
    }
}

pub(crate) static WORKER_POOL_OPERATION: LazyLock<WorkerPoolOperation> =
    LazyLock::new(WorkerPoolOperation::default);

pub(crate) async fn create_or_scale_pool(filename: String, max_concurrency: usize) -> Result<()> {
    WORKER_POOL_OPERATION
        .create_or_scale_pool(filename, max_concurrency)
        .await
}

pub(crate) async fn connect_to_worker(pool_id: String, task_id: u32) -> Result<u32> {
    WORKER_POOL_OPERATION
        .connect_to_worker(pool_id, task_id)
        .await
}

pub(crate) async fn send_message_to_worker(worker_id: u32, data: String) -> Result<()> {
    WORKER_POOL_OPERATION
        .send_message_to_worker(worker_id, data)
        .await
}

pub(crate) async fn send_worker_termination(pool_id: String, worker_id: u32) -> Result<()> {
    WORKER_POOL_OPERATION
        .send_worker_termination(pool_id, worker_id)
        .await
}

pub async fn recv_task_message(task_id: u32) -> Result<String> {
    WORKER_POOL_OPERATION.recv_task_response(task_id).await
}

pub fn shutdown() {
    WORKER_POOL_OPERATION.shutdown();
}

pub(crate) struct WorkerOperation {
    pub(crate) pool_id: String,
    pub(crate) task_id: u32,
    pub(crate) worker_id: u32,
}

#[async_trait::async_trait]
impl Operation for WorkerOperation {
    async fn recv(&mut self) -> Result<String> {
        recv_task_message(self.task_id).await
    }

    async fn send(&mut self, data: String) -> Result<()> {
        send_message_to_worker(self.worker_id, data).await
    }

    async fn wait_or_kill(&mut self) -> Result<ExitStatus> {
        send_worker_termination(self.pool_id.clone(), self.worker_id).await?;
        Ok(ExitStatus::default())
    }

    fn disallow_reuse(&mut self) {
        // do nothing
    }
}
