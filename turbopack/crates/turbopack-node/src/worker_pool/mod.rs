use std::{
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
};

use anyhow::Result;
use rustc_hash::FxHashMap;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, duration_span};
use turbo_tasks_fs::FileSystemPath;

use crate::{
    AssetsForSourceMapping,
    evaluate::{EvaluateOperation, EvaluatePool, Operation},
    worker_pool::operation::{WorkerOperation, connect_to_worker, create_or_scale_pool},
};

mod operation;
pub use operation::shutdown;
mod worker_thread;

static OPERATION_TASK_ID: AtomicU32 = AtomicU32::new(1);

#[turbo_tasks::value]
pub(crate) struct WorkerThreadPool {
    cwd: PathBuf,
    entrypoint: PathBuf,
    env: FxHashMap<RcStr, RcStr>,
    concurrency: usize,
    pub(crate) assets_for_source_mapping: ResolvedVc<AssetsForSourceMapping>,
    pub(crate) assets_root: FileSystemPath,
    pub(crate) project_dir: FileSystemPath,
}

impl WorkerThreadPool {
    pub(crate) fn create(
        cwd: PathBuf,
        entrypoint: PathBuf,
        env: FxHashMap<RcStr, RcStr>,
        assets_for_source_mapping: ResolvedVc<AssetsForSourceMapping>,
        assets_root: FileSystemPath,
        project_dir: FileSystemPath,
        concurrency: usize,
        debug: bool,
    ) -> EvaluatePool {
        EvaluatePool::new(
            entrypoint.to_string_lossy().to_string().into(),
            Box::new(Self {
                cwd,
                entrypoint,
                env,
                concurrency: (if debug { 1 } else { concurrency }),
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
            let pool_id: RcStr = self.entrypoint.to_string_lossy().into();

            create_or_scale_pool(pool_id.clone(), self.concurrency, self.env.clone()).await?;

            let task_id = OPERATION_TASK_ID.fetch_add(1, Ordering::Release);

            if task_id == 0 {
                panic!("operation task id overflow")
            }

            let worker_id = connect_to_worker(pool_id.clone(), task_id).await?;

            WorkerOperation {
                pool_id,
                task_id,
                worker_id,
            }
        };

        Ok(Box::new(operation))
    }
}
