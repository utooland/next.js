use std::sync::LazyLock;

use anyhow::{Context, Result};
use async_channel::{Receiver, Sender, bounded, unbounded};
use dashmap::DashMap;

use crate::evaluate::Operation;

pub(crate) struct MessageChannel<T: Send + Sync + 'static> {
    sender: Sender<T>,
    receiver: Receiver<T>,
}

impl<T: Send + Sync + 'static> MessageChannel<T> {
    pub(super) fn unbounded() -> Self {
        let (sender, receiver) = unbounded::<T>();
        Self { sender, receiver }
    }

    pub(super) fn bounded(cap: usize) -> Self {
        let (sender, receiver) = bounded::<T>(cap);
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
}

pub(crate) static POOL_REQUEST_CHANNEL: LazyLock<MessageChannel<String>> =
    LazyLock::new(MessageChannel::unbounded);

pub(crate) static POOL_CREATION_CHANNEL: LazyLock<DashMap<String, MessageChannel<()>>> =
    LazyLock::new(DashMap::new);

pub(crate) async fn create_pool(filename: String, concurrency: usize) -> anyhow::Result<()> {
    POOL_REQUEST_CHANNEL
        .send(filename.clone())
        .await
        .context("failed to send pool request")?;

    let mut created_worker_count = 0;

    {
        let channel = POOL_CREATION_CHANNEL
            .entry(filename.clone())
            .or_insert_with(|| MessageChannel::bounded(concurrency));

        while created_worker_count < concurrency {
            channel
                .recv()
                .await
                .context("failed to recv worker creation")?;
            created_worker_count += 1;
        }
    };

    POOL_CREATION_CHANNEL.remove(&filename);

    Ok(())
}

pub(crate) static EVALUATION_REQUEST_CHANNAL: LazyLock<DashMap<String, MessageChannel<Vec<u8>>>> =
    LazyLock::new(DashMap::new);

pub(crate) static WORKER_REQUEST_CHANNAL: LazyLock<DashMap<String, MessageChannel<()>>> =
    LazyLock::new(DashMap::new);

pub(crate) static WORKER_ACK_CHANNAL: LazyLock<DashMap<String, MessageChannel<u32>>> =
    LazyLock::new(DashMap::new);

pub(crate) async fn connect_to_worker(pool_id: String, task_id: String) -> Result<u32> {
    let channel = WORKER_REQUEST_CHANNAL
        .entry(pool_id.clone())
        .or_insert_with(MessageChannel::unbounded);
    channel
        .send(())
        .await
        .context("failed to send evaluation request")?;
    let worker_id = async move {
        let channel = WORKER_ACK_CHANNAL
            .entry(task_id.clone())
            .or_insert_with(MessageChannel::unbounded);
        channel
            .recv()
            .await
            .context("failed to recv evaluation ack")
    }
    .await?;
    Ok(worker_id)
}

pub(crate) static WORKER_ROUTED_CHANNEL: LazyLock<DashMap<u32, MessageChannel<Vec<u8>>>> =
    LazyLock::new(DashMap::new);

pub(crate) async fn send_message_to_worker(worker_id: u32, data: Vec<u8>) -> Result<()> {
    let entry = WORKER_ROUTED_CHANNEL
        .entry(worker_id)
        .or_insert_with(MessageChannel::unbounded);
    entry
        .send(data)
        .await
        .with_context(|| format!("failed to send message to worker {worker_id}"))?;
    Ok(())
}

pub(crate) static TASK_ROUTERD_CHANNEL: LazyLock<DashMap<String, MessageChannel<Vec<u8>>>> =
    LazyLock::new(DashMap::new);

pub async fn recv_task_response(task_id: String) -> Result<Vec<u8>> {
    let channel = TASK_ROUTERD_CHANNEL
        .entry(task_id.clone())
        .or_insert_with(MessageChannel::unbounded);
    let data = channel
        .recv()
        .await
        .with_context(|| format!("failed to send message to worker {task_id}"))?;
    Ok(data)
}

pub(crate) struct WorkerOperation {
    pub(crate) task_id: String,
    pub(crate) worker_id: u32,
}

#[async_trait::async_trait]
impl Operation for WorkerOperation {
    async fn recv(&mut self) -> Result<Vec<u8>> {
        recv_task_response(self.task_id.clone()).await
    }

    async fn send(&mut self, data: Vec<u8>) -> Result<()> {
        send_message_to_worker(self.worker_id, data).await
    }
}
