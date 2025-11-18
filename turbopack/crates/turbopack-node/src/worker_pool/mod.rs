use std::path::PathBuf;

use anyhow::Result;
use rustc_hash::FxHashMap;
use turbo_rcstr::RcStr;
use turbo_tasks::ResolvedVc;
use turbo_tasks_fs::FileSystemPath;

use crate::{
    AssetsForSourceMapping,
    evaluate::{EvaluateOperation, EvaluatePool, Operation},
    worker_pool::operation::{WorkerOperation, connect_to_worker, create_pool},
};

mod operation;
mod worker_thread;

#[turbo_tasks::value(cell = "new", serialization = "none", eq = "manual", shared)]
pub struct WorkerThreadPool {
    cwd: PathBuf,
    entrypoint: PathBuf,
    env: FxHashMap<RcStr, RcStr>,
    concurrency: usize,
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
        create_pool(
            self.entrypoint.to_string_lossy().to_string(),
            self.concurrency,
        )
        .await?;

        let task_id = uuid::Uuid::new_v4().to_string();

        let worker_id = connect_to_worker(
            self.entrypoint.to_string_lossy().to_string(),
            task_id.clone(),
        )
        .await?;

        let worker_operation = WorkerOperation { task_id, worker_id };

        Ok(Box::new(worker_operation))
    }
}
