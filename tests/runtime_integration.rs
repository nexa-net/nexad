//! Integration tests for ContainerRuntime implementations.
//!
//! Requires a real container runtime. Tests are #[ignore] by default.
//! Run with: NEXA_TEST_RUNTIME=docker cargo test --test runtime_integration -- --ignored

use std::collections::HashMap;
use std::sync::Arc;

use nexa_core::ports::runtime::*;

const TEST_IMAGE: &str = "busybox:latest";

async fn get_runtime() -> Option<Arc<dyn ContainerRuntime>> {
    let runtime_name = std::env::var("NEXA_TEST_RUNTIME").unwrap_or("docker".into());
    match runtime_name.as_str() {
        "docker" => {
            use nexad::adapters::runtime::DockerRuntime;
            match DockerRuntime::new() {
                Ok(rt) => {
                    if rt.ping().await.is_ok() {
                        Some(Arc::new(rt))
                    } else {
                        eprintln!("Docker not reachable, skipping");
                        None
                    }
                }
                Err(_) => {
                    eprintln!("Failed to create DockerRuntime");
                    None
                }
            }
        }
        "containerd" => {
            use nexad::adapters::runtime::ContainerdRuntime;
            match ContainerdRuntime::new("/tmp/nexa-test") {
                Ok(rt) => {
                    if rt.ping().await.is_ok() {
                        Some(Arc::new(rt))
                    } else {
                        eprintln!("containerd not reachable, skipping");
                        None
                    }
                }
                Err(_) => {
                    eprintln!("Failed to create ContainerdRuntime");
                    None
                }
            }
        }
        other => panic!("Unknown NEXA_TEST_RUNTIME: {other}"),
    }
}

fn unique_name(prefix: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    format!("nexa-test-{prefix}-{id}")
}

#[tokio::test]
#[ignore]
async fn test_pull_image() {
    let rt = match get_runtime().await {
        Some(rt) => rt,
        None => return,
    };

    rt.pull_image(TEST_IMAGE)
        .await
        .expect("pull_image should succeed");
}

#[tokio::test]
#[ignore]
async fn test_create_start_stop_remove() {
    let rt = match get_runtime().await {
        Some(rt) => rt,
        None => return,
    };

    rt.pull_image(TEST_IMAGE).await.expect("pull_image failed");

    let name = unique_name("lifecycle");
    let config = ContainerConfig {
        name: name.clone(),
        image: TEST_IMAGE.to_string(),
        command: vec!["sleep".into(), "60".into()],
        env: HashMap::new(),
        ports: vec![],
        volumes: vec![],
        labels: HashMap::new(),
        network: None,
        dns: vec![],
        dns_search: vec![],
    };

    let id = rt
        .create_container(&config)
        .await
        .expect("create_container failed");

    rt.start_container(&id).await.expect("start failed");

    let info = rt.inspect_container(&id).await.expect("inspect failed");
    assert_eq!(
        info.state,
        ContainerState::Running,
        "container should be running after start"
    );

    rt.stop_container(&id, 5).await.expect("stop failed");

    let info = rt.inspect_container(&id).await.expect("inspect failed");
    assert!(
        info.state == ContainerState::Exited || info.state == ContainerState::Created,
        "container should be exited or created after stop, got {:?}",
        info.state
    );

    rt.remove_container(&id, false)
        .await
        .expect("remove failed");

    let exists = rt
        .container_exists(&name)
        .await
        .expect("container_exists failed");
    assert!(!exists, "container should not exist after removal");
}

#[tokio::test]
#[ignore]
async fn test_container_exists() {
    let rt = match get_runtime().await {
        Some(rt) => rt,
        None => return,
    };

    rt.pull_image(TEST_IMAGE).await.expect("pull_image failed");

    let name = unique_name("exists");

    let exists = rt
        .container_exists(&name)
        .await
        .expect("container_exists failed");
    assert!(!exists, "container should not exist before creation");

    let config = ContainerConfig {
        name: name.clone(),
        image: TEST_IMAGE.to_string(),
        command: vec![],
        env: HashMap::new(),
        ports: vec![],
        volumes: vec![],
        labels: HashMap::new(),
        network: None,
        dns: vec![],
        dns_search: vec![],
    };

    let id = rt
        .create_container(&config)
        .await
        .expect("create_container failed");

    let exists = rt
        .container_exists(&name)
        .await
        .expect("container_exists failed");
    assert!(exists, "container should exist after creation");

    rt.remove_container(&id, true).await.expect("remove failed");

    let exists = rt
        .container_exists(&name)
        .await
        .expect("container_exists failed");
    assert!(!exists, "container should not exist after removal");
}

#[tokio::test]
#[ignore]
async fn test_inspect_container() {
    let rt = match get_runtime().await {
        Some(rt) => rt,
        None => return,
    };

    rt.pull_image(TEST_IMAGE).await.expect("pull_image failed");

    let name = unique_name("inspect");
    let config = ContainerConfig {
        name: name.clone(),
        image: TEST_IMAGE.to_string(),
        command: vec![],
        env: HashMap::new(),
        ports: vec![],
        volumes: vec![],
        labels: HashMap::new(),
        network: None,
        dns: vec![],
        dns_search: vec![],
    };

    let id = rt
        .create_container(&config)
        .await
        .expect("create_container failed");

    let info = rt.inspect_container(&id).await.expect("inspect failed");
    assert!(!info.id.is_empty(), "container id should not be empty");
    assert!(
        info.image.contains("busybox"),
        "image should contain 'busybox', got: {}",
        info.image
    );

    rt.remove_container(&id, true).await.expect("remove failed");
}

#[tokio::test]
#[ignore]
async fn test_network_lifecycle() {
    let rt = match get_runtime().await {
        Some(rt) => rt,
        None => return,
    };

    let net_name = unique_name("net");

    let net_id = rt
        .create_network(&net_name)
        .await
        .expect("create_network failed");
    assert!(!net_id.is_empty(), "network id should not be empty");

    rt.remove_network(&net_name)
        .await
        .expect("remove_network failed");
}

#[tokio::test]
#[ignore]
async fn test_runtime_name() {
    let rt = match get_runtime().await {
        Some(rt) => rt,
        None => return,
    };

    let name = rt.runtime_name();
    assert!(
        name == "docker" || name == "containerd",
        "runtime_name should be 'docker' or 'containerd', got: {name}"
    );
}
