use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use anyhow::Result;
use rustc_hash::FxHashMap;
use tokio::sync::oneshot;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, duration_span};
use turbo_tasks_fs::FileSystemPath;

use crate::{
    AssetsForSourceMapping,
    evaluate::{EvaluateOperation, EvaluatePool, Operation},
    worker_pool::{
        operation::{
            PoolOptions, PoolState, WORKER_POOL_OPERATION, WorkerOperation, get_pool_state,
        },
        worker_thread::{NapiPoolOptions, create_worker},
    },
};

mod operation;
mod worker_thread;

static OPERATION_TASK_ID: AtomicU32 = AtomicU32::new(1);

#[turbo_tasks::value(cell = "new", serialization = "none", eq = "manual", shared)]
pub(crate) struct WorkerThreadPool {
    cwd: PathBuf,
    entrypoint: PathBuf,
    env: Arc<FxHashMap<RcStr, RcStr>>,
    concurrency: usize,
    pub(crate) assets_for_source_mapping: ResolvedVc<AssetsForSourceMapping>,
    pub(crate) assets_root: FileSystemPath,
    pub(crate) project_dir: FileSystemPath,
    #[turbo_tasks(trace_ignore, debug_ignore)]
    state: Arc<PoolState>,
}

impl WorkerThreadPool {
    pub(crate) async fn create(
        cwd: PathBuf,
        entrypoint: PathBuf,
        env: FxHashMap<RcStr, RcStr>,
        assets_for_source_mapping: ResolvedVc<AssetsForSourceMapping>,
        assets_root: FileSystemPath,
        project_dir: FileSystemPath,
        concurrency: usize,
        debug: bool,
    ) -> EvaluatePool {
        let pool_id: RcStr = entrypoint.to_string_lossy().to_string().into();
        let state = get_pool_state(&pool_id).await;
        EvaluatePool::new(
            pool_id,
            Box::new(Self {
                cwd,
                entrypoint,
                env: Arc::new(env),
                concurrency: (if debug { 1 } else { concurrency }),
                assets_for_source_mapping,
                assets_root: assets_root.clone(),
                project_dir: project_dir.clone(),
                state,
            }),
            assets_for_source_mapping,
            assets_root,
            project_dir,
        )
    }

    async fn acquire_worker(&self, task_id: u32) -> Result<u32> {
        {
            let mut idle = self.state.idle_workers.lock();
            if let Some(worker_id) = idle.pop() {
                return Ok(worker_id);
            }
        }

        let can_create = {
            let mut stats = self.state.stats.lock();
            if (stats.workers as usize) < self.concurrency {
                stats.add_booting_worker();
                true
            } else {
                false
            }
        };

        if can_create {
            let pool_id: RcStr = self.entrypoint.to_string_lossy().into();
            let worker_id = create_worker(
                NapiPoolOptions {
                    filename: pool_id,
                    cwd: self.cwd.to_string_lossy().into(),
                },
                task_id,
            )
            .await?;

            {
                let mut stats = self.state.stats.lock();
                stats.finished_booting_worker();
            }

            return Ok(worker_id);
        }

        let (tx, rx) = oneshot::channel();
        {
            let mut waiters = self.state.waiters.lock();
            let mut idle = self.state.idle_workers.lock();
            if let Some(worker_id) = idle.pop() {
                return Ok(worker_id);
            }
            waiters.push(tx);
        }

        Ok(rx.await?)
    }
}

impl WorkerThreadPool {
    pub fn scale_down() {
        let _ = WORKER_POOL_OPERATION.scale_down();
    }

    pub fn scale_zero() {
        let _ = WORKER_POOL_OPERATION.scale_zero();
    }
}

#[async_trait::async_trait]
impl EvaluateOperation for WorkerThreadPool {
    async fn operation(&self) -> Result<Box<dyn Operation>> {
        let operation = {
            let _guard = duration_span!("Node.js operation");
            let pool_id: RcStr = self.entrypoint.to_string_lossy().into();

            let task_id = OPERATION_TASK_ID.fetch_add(1, Ordering::Release);

            if task_id == 0 {
                panic!("operation task id overflow")
            }

            let worker_id = self.acquire_worker(task_id).await?;

            let state = self.state.clone();

            WorkerOperation {
                pool_id,
                task_id,
                worker_id,
                state: state.clone(),
                on_drop: Some(Box::new(move |worker_id| {
                    let mut waiters = state.waiters.lock();
                    if let Some(tx) = waiters.pop() {
                        let _ = tx.send(worker_id);
                    } else {
                        state.idle_workers.lock().push(worker_id);
                    }
                })),
            }
        };

        Ok(Box::new(operation))
    }
}
