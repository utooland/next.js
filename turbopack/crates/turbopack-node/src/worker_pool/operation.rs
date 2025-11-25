use std::sync::LazyLock;

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

    pub(crate) fn try_recv(&self) -> Result<T> {
        Ok(self.receiver.try_recv()?)
    }
}

pub(crate) struct WorkerThreadOperation {
    pool_request_channel: MessageChannel<(String, usize)>,
    pool_ack_channel: DashMap<String, MessageChannel<()>>,
    worker_request_channel: DashMap<String, MessageChannel<String>>,
    worker_ack_channel: DashMap<String, MessageChannel<u32>>,
    worker_routed_channel: DashMap<u32, MessageChannel<String>>,
    task_routed_channel: DashMap<String, MessageChannel<String>>,
}

impl Default for WorkerThreadOperation {
    fn default() -> Self {
        Self {
            pool_request_channel: MessageChannel::unbounded(),
            pool_ack_channel: DashMap::new(),
            worker_request_channel: DashMap::new(),
            worker_ack_channel: DashMap::new(),
            worker_routed_channel: DashMap::new(),
            task_routed_channel: DashMap::new(),
        }
    }
}

impl WorkerThreadOperation {
    pub(crate) async fn create_pool(
        &self,
        filename: String,
        concurrency: usize,
    ) -> anyhow::Result<()> {
        self.pool_request_channel
            .send((filename.clone(), concurrency))
            .await
            .context("failed to send pool request")?;

        let mut created_worker_count = 0;

        {
            let channel = self
                .pool_ack_channel
                .entry(filename.clone())
                .or_insert_with(MessageChannel::unbounded)
                .clone();

            while created_worker_count < concurrency {
                channel
                    .recv()
                    .await
                    .context("failed to recv worker creation")?;
                created_worker_count += 1;
            }
        };

        self.pool_ack_channel.remove(&filename);

        Ok(())
    }

    pub(crate) async fn connect_to_worker(&self, pool_id: String, task_id: String) -> Result<u32> {
        let channel = self
            .worker_request_channel
            .entry(pool_id.clone())
            .or_insert_with(MessageChannel::unbounded)
            .clone();
        channel
            .send(task_id.clone())
            .await
            .context("failed to send worker request")?;
        let worker_id = async move {
            let channel = self
                .worker_ack_channel
                .entry(task_id.clone())
                .or_insert_with(MessageChannel::unbounded)
                .clone();
            channel.recv().await.context("failed to recv worker ack")
        }
        .await?;
        Ok(worker_id)
    }

    pub(crate) async fn send_message_to_worker(&self, worker_id: u32, data: String) -> Result<()> {
        let entry = self
            .worker_routed_channel
            .entry(worker_id)
            .or_insert_with(MessageChannel::unbounded)
            .clone();
        entry
            .send(data)
            .await
            .with_context(|| format!("failed to send message to worker {worker_id}"))?;
        Ok(())
    }

    pub async fn recv_task_response(&self, task_id: String) -> Result<String> {
        let channel = self
            .task_routed_channel
            .entry(task_id.clone())
            .or_insert_with(MessageChannel::unbounded)
            .clone();
        let data = channel
            .recv()
            .await
            .with_context(|| format!("failed to recv message  for task {task_id}"))?;
        Ok(data)
    }

    pub(crate) fn try_recv_pool_creation(&self) -> Option<(String, usize)> {
        self.pool_request_channel.try_recv().ok()
    }

    pub(crate) async fn notify_one_worker_created(&self, filename: String) -> Result<()> {
        let channel = self
            .pool_ack_channel
            .entry(filename.clone())
            .or_insert_with(MessageChannel::unbounded)
            .clone();
        channel
            .send(())
            .await
            .context("failed to notify worker created")
    }

    pub(crate) async fn recv_worker_request(&self, pool_id: String) -> Result<String> {
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

    pub(crate) async fn notify_worker_ack(&self, task_id: String, worker_id: u32) -> Result<()> {
        let channel = self
            .worker_ack_channel
            .get(&task_id)
            .context(format!("worker ack channel for {task_id} not found"))?;
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

    pub(crate) async fn send_task_message(&self, task_id: String, data: String) -> Result<()> {
        let channel = self
            .task_routed_channel
            .entry(task_id.clone())
            .or_insert_with(MessageChannel::unbounded)
            .clone();
        channel
            .send(data)
            .await
            .with_context(|| format!("failed to send  response for task {task_id}"))
    }
}

pub(crate) static WORKER_POOL_OPERATION: LazyLock<WorkerThreadOperation> =
    LazyLock::new(WorkerThreadOperation::default);

pub(crate) async fn create_pool(filename: String, concurrency: usize) -> anyhow::Result<()> {
    WORKER_POOL_OPERATION
        .create_pool(filename, concurrency)
        .await
}

pub(crate) async fn connect_to_worker(pool_id: String, task_id: String) -> Result<u32> {
    WORKER_POOL_OPERATION
        .connect_to_worker(pool_id, task_id)
        .await
}

pub(crate) async fn send_message_to_worker(worker_id: u32, data: String) -> Result<()> {
    WORKER_POOL_OPERATION
        .send_message_to_worker(worker_id, data)
        .await
}

pub async fn recv_task_response(task_id: String) -> Result<String> {
    WORKER_POOL_OPERATION.recv_task_response(task_id).await
}

pub(crate) struct WorkerOperation {
    pub(crate) task_id: String,
    pub(crate) worker_id: u32,
}

#[async_trait::async_trait]
impl Operation for WorkerOperation {
    async fn recv(&mut self) -> Result<String> {
        recv_task_response(self.task_id.clone()).await
    }

    async fn send(&mut self, data: String) -> Result<()> {
        send_message_to_worker(self.worker_id, data).await
    }
}
