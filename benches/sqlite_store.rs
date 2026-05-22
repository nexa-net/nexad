use std::collections::HashMap;
use std::time::Instant;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use nexa_core::domain::models::{
    Deployment, DeploymentMeta, DeploymentSpec, Pod, Project, RestartPolicy,
};
use nexa_core::ports::state::StateStore;
use tempfile::TempDir;
use tokio::runtime::Runtime;

fn make_rt() -> Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

/// Set up a store with a project and deployment, returning both plus the tempdir
/// (caller must keep tempdir alive to prevent deletion).
async fn setup_store() -> (nexad::adapters::state::SqliteStore, Deployment, TempDir) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("bench.db");
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let store = nexad::adapters::state::SqliteStore::connect(&url)
        .await
        .expect("connect");

    let project = Project::new("bench-project");
    store.insert_project(&project).await.unwrap();

    let spec = DeploymentSpec {
        project: "bench-project".into(),
        deployment: DeploymentMeta {
            name: "bench-deploy".into(),
        },
        replicas: 1,
        image: "nginx:latest".into(),
        ports: vec![],
        env: HashMap::new(),
        volumes: vec![],
        secrets: vec![],
        network: None,
        healthcheck: None,
        restart: RestartPolicy::default(),
        resources: None,
    };
    let deployment = Deployment::from_spec(spec);
    store.insert_deployment(&deployment).await.unwrap();

    (store, deployment, dir)
}

fn bench_insert_pod(c: &mut Criterion) {
    let rt = make_rt();

    c.bench_function("insert_pod", |b| {
        b.iter_custom(|iters| {
            rt.block_on(async {
                let (store, deployment, _dir) = setup_store().await;
                let mut pods: Vec<Pod> = (0..iters)
                    .map(|i| {
                        Pod::new(
                            deployment.id,
                            "bench-project",
                            "bench-deploy",
                            i as u32,
                            "nginx:latest",
                        )
                    })
                    .collect();

                let start = Instant::now();
                for pod in &pods {
                    store.insert_pod(pod).await.unwrap();
                }
                let elapsed = start.elapsed();

                // cleanup: delete all pods so the DB doesn't grow across calls
                for pod in &mut pods {
                    let _ = store.delete_pod(&pod.id).await;
                }

                elapsed
            })
        });
    });
}

fn bench_list_pods(c: &mut Criterion) {
    let rt = make_rt();
    let mut group = c.benchmark_group("list_pods");

    for &n in &[100usize, 1000usize] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &count| {
            b.iter_custom(|iters| {
                rt.block_on(async {
                    let (store, deployment, _dir) = setup_store().await;

                    // Pre-populate N pods
                    for i in 0..count {
                        let pod = Pod::new(
                            deployment.id,
                            "bench-project",
                            "bench-deploy",
                            i as u32,
                            "nginx:latest",
                        );
                        store.insert_pod(&pod).await.unwrap();
                    }

                    let start = Instant::now();
                    for _ in 0..iters {
                        let pods = store.list_pods(Some("bench-project")).await.unwrap();
                        assert_eq!(pods.len(), count);
                    }
                    start.elapsed()
                })
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_insert_pod, bench_list_pods);
criterion_main!(benches);
