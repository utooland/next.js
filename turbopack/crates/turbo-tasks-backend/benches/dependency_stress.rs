use anyhow::Result;
use criterion::{BenchmarkId, Criterion};
use turbo_tasks::{TryJoinIterExt, TurboTasks, Vc};
use turbo_tasks_backend::{BackendOptions, TurboTasksBackend, noop_backing_storage};

pub fn dependency_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("turbo_tasks_backend_dependency_stress");
    group.sample_size(20);

    for size in [100, 1000, 5000] {
        group.throughput(criterion::Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("many_readers", size), &size, |b, size| {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            let size = *size;

            b.to_async(rt).iter_with_large_drop(move || {
                let tt = TurboTasks::new(TurboTasksBackend::new(
                    BackendOptions {
                        storage_mode: None,
                        ..Default::default()
                    },
                    noop_backing_storage(),
                ));
                async move {
                    tt.run(async move {
                        let root = root_task();
                        (0..size).map(|_| dependent_task(root)).try_join().await?;
                        Ok(())
                    })
                    .await
                    .unwrap();
                }
            });
        });
    }
}

#[turbo_tasks::value(transparent)]
struct Empty(());

#[turbo_tasks::function]
fn root_task() -> Vc<Empty> {
    Empty(()).cell()
}

#[turbo_tasks::function]
async fn dependent_task(root: Vc<Empty>) -> Result<Vc<Empty>> {
    root.await?;
    Ok(Empty(()).cell())
}
