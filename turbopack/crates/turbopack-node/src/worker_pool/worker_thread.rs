use anyhow::Context;
use napi_derive::napi;

use crate::worker_pool::operation::{
    EVALUATION_REQUEST_CHANNAL, MessageChannel, POOL_CREATION_CHANNEL, POOL_REQUEST_CHANNEL,
    TASK_ROUTERD_CHANNEL, WORKER_REQUEST_CHANNAL, WORKER_ROUTED_CHANNEL,
};

#[napi]
pub async fn recv_pool_request() -> napi::Result<String> {
    Ok(POOL_REQUEST_CHANNEL
        .recv()
        .await
        .context("failed to recv pool request")?)
}

#[napi]
pub async fn notify_pool_created(filename: String) -> napi::Result<()> {
    let channel = if let Some(channel) = POOL_CREATION_CHANNEL.get(&filename) {
        channel
    } else {
        return Err(napi::Error::from_reason(format!(
            "pool creation channel for {filename} not found"
        )));
    };
    Ok(channel
        .send(())
        .await
        .context("failed to notify pool created")?)
}

#[napi]
pub async fn recv_worker_request(pool_id: String) -> napi::Result<()> {
    let channel = if let Some(channel) = WORKER_REQUEST_CHANNAL.get(&pool_id) {
        channel
    } else {
        return Err(napi::Error::from_reason(format!(
            "worker request channel for {pool_id} not found"
        )));
    };
    Ok(channel
        .send(())
        .await
        .context("failed to recv worker request")?)
}

#[napi]
pub async fn notify_worker_ack(pool_id: String) -> napi::Result<()> {
    let channel = if let Some(channel) = POOL_CREATION_CHANNEL.get(&pool_id) {
        channel
    } else {
        return Err(napi::Error::from_reason(format!(
            "evaluation ack channel for {pool_id} not found"
        )));
    };
    Ok(channel
        .send(())
        .await
        .context("failed to notify evaluation ack")?)
}

#[napi]
pub async fn recv_evaluation(pool_id: String) -> napi::Result<Vec<u8>> {
    let channel = if let Some(channel) = EVALUATION_REQUEST_CHANNAL.get(&pool_id) {
        channel
    } else {
        return Err(napi::Error::from_reason(format!(
            "evaluation request channel for {pool_id} not found"
        )));
    };

    Ok(channel
        .recv()
        .await
        .context("failed to recv evaluate request")?)
}

#[napi]
pub async fn recv_message_in_worker(worker_id: u32) -> napi::Result<Vec<u8>> {
    let channel = WORKER_ROUTED_CHANNEL
        .entry(worker_id)
        .or_insert_with(MessageChannel::unbounded);
    let data = channel
        .recv()
        .await
        .with_context(|| format!("failed to recv message in worker {worker_id}"))?;
    Ok(data)
}

#[napi]
pub async fn send_task_response(task_id: String, data: Vec<u8>) -> napi::Result<()> {
    let channel = TASK_ROUTERD_CHANNEL
        .entry(task_id.clone())
        .or_insert_with(MessageChannel::unbounded);
    channel
        .send(data)
        .await
        .with_context(|| format!("failed to recv message in worker {task_id}"))?;
    Ok(())
}
