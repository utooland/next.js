use std::path::PathBuf;

use anyhow::Result;
use rustc_hash::FxHashMap;
use tokio::sync::OnceCell;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, duration_span};
use turbo_tasks_fs::FileSystemPath;

use crate::{
    AssetsForSourceMapping,
    evaluate::{EvaluateOperation, EvaluatePool, Operation},
    worker_pool::operation::{WorkerOperation, connect_to_worker, create_pool},
};

mod operation;
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
mod worker_thread;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
mod web_worker;

#[turbo_tasks::value(cell = "new", serialization = "none", eq = "manual", shared)]
pub struct WorkerThreadPool {
    cwd: PathBuf,
    entrypoint: PathBuf,
    env: FxHashMap<RcStr, RcStr>,
    concurrency: usize,
    #[turbo_tasks(trace_ignore, debug_ignore)]
    ready: OnceCell<()>,
    pub assets_for_source_mapping: ResolvedVc<AssetsForSourceMapping>,
    pub assets_root: FileSystemPath,
    pub project_dir: FileSystemPath,
}

impl WorkerThreadPool {
    pub fn create(
        cwd: PathBuf,
        entrypoint: PathBuf,
        env: FxHashMap<RcStr, RcStr>,
        assets_for_source_mapping: ResolvedVc<AssetsForSourceMapping>,
        assets_root: FileSystemPath,
        project_dir: FileSystemPath,
        concurrency: usize,
        _debug: bool,
    ) -> EvaluatePool {
        EvaluatePool::new(
            entrypoint.to_string_lossy().to_string().into(),
            Box::new(Self {
                cwd,
                entrypoint,
                env,
                concurrency,
                ready: OnceCell::new(),
                assets_for_source_mapping,
                assets_root: assets_root.clone(),
                project_dir: project_dir.clone(),
            }),
            assets_for_source_mapping,
            assets_root,
            project_dir,
        )
    }
}

#[async_trait::async_trait]
impl EvaluateOperation for WorkerThreadPool {
    async fn operation(&self) -> Result<Box<dyn Operation>> {
        let operation = {
            let _guard = duration_span!("Node.js operation");
            let entrypoint = self.entrypoint.to_string_lossy().to_string();
            self.ready
                .get_or_init(async || {
                    create_pool(
                        self.entrypoint.to_string_lossy().to_string(),
                        self.concurrency,
                    )
                    .await
                    .unwrap_or_else(|e| {
                        panic!("failed to create worker pool for {entrypoint} for reason: {e}",)
                    })
                })
                .await;

            let task_id = uuid::Uuid::new_v4().to_string();

            let worker_id = connect_to_worker(
                self.entrypoint.to_string_lossy().to_string(),
                task_id.clone(),
            )
            .await?;

            WorkerOperation { task_id, worker_id }
        };

        Ok(Box::new(operation))
    }
}
