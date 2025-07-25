use turbo_tasks::{ResolvedVc, Vc, TaskInput};
use turbo_tasks_env::ProcessEnv;
use turbo_tasks_fs::FileSystemPath;
use turbopack_core::chunk::ChunkingContext;

#[turbo_tasks::value]
#[derive(Clone, Copy, Debug, Hash, TaskInput)]
pub enum NodeExecutionEnvironment {
    Server,
    WebWorker,
    ServiceWorker,
}

impl Default for NodeExecutionEnvironment {
    fn default() -> Self {
        Self::Server
    }
}

#[turbo_tasks::value]
pub struct ExecutionContext {
    pub project_path: FileSystemPath,
    pub chunking_context: ResolvedVc<Box<dyn ChunkingContext>>,
    pub env: ResolvedVc<Box<dyn ProcessEnv>>,
    pub environment: NodeExecutionEnvironment,
}

#[turbo_tasks::value_impl]
impl ExecutionContext {
    #[turbo_tasks::function]
    pub fn new(
        project_path: FileSystemPath,
        chunking_context: ResolvedVc<Box<dyn ChunkingContext>>,
        env: ResolvedVc<Box<dyn ProcessEnv>>,
    ) -> Vc<Self> {
        Self::new_with_environment(
            project_path,
            *chunking_context,
            *env,
            NodeExecutionEnvironment::Server
        )
    }

    #[turbo_tasks::function]
    pub fn new_with_environment(
        project_path: FileSystemPath,
        chunking_context: ResolvedVc<Box<dyn ChunkingContext>>,
        env: ResolvedVc<Box<dyn ProcessEnv>>,
        environment: NodeExecutionEnvironment,
    ) -> Vc<Self> {
        ExecutionContext {
            project_path,
            chunking_context,
            env,
            environment,
        }
        .cell()
    }

    #[turbo_tasks::function]
    pub fn project_path(&self) -> Vc<FileSystemPath> {
        self.project_path.clone().cell()
    }

    #[turbo_tasks::function]
    pub fn chunking_context(&self) -> Vc<Box<dyn ChunkingContext>> {
        *self.chunking_context
    }

    #[turbo_tasks::function]
    pub fn env(&self) -> Vc<Box<dyn ProcessEnv>> {
        *self.env
    }

    #[turbo_tasks::function]
    pub fn environment(&self) -> Vc<NodeExecutionEnvironment> {
        self.environment.cell()
    }

    #[turbo_tasks::function]
    pub fn is_worker_environment(&self) -> Vc<bool> {
        Vc::cell(matches!(
            self.environment,
            NodeExecutionEnvironment::WebWorker | NodeExecutionEnvironment::ServiceWorker
        ))
    }
}
