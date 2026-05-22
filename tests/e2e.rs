use std::sync::Arc;
use std::time::Duration;

use nexa_core::domain::orchestrator::Orchestrator;
use nexa_core::ports::runtime::ContainerRuntime;
use rusqlite::Connection;
use uuid::Uuid;

use nexad::adapters::runtime::DockerRuntime;
use nexad::adapters::secrets::EncryptedSqliteSecretStore;
use nexad::adapters::state::{InMemoryRouteStore, SqliteStore};
use nexad::adapters::transport::LocalTransport;
use nexad::api::{AppState, routes};

// ---------------------------------------------------------------------------
// E2eServer
// ---------------------------------------------------------------------------

struct E2eServer {
    base_url: String,
    _dir: tempfile::TempDir,
}

impl E2eServer {
    async fn new() -> Self {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let db_path = dir.path().join("nexad.db");
        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

        // State store
        let store = SqliteStore::connect(&db_url)
            .await
            .expect("failed to connect SqliteStore");
        let store: Arc<dyn nexa_core::ports::state::StateStore> = Arc::new(store);

        // Secret store
        let secret_conn = Connection::open_in_memory().expect("failed to open secret db");
        let secret_store = EncryptedSqliteSecretStore::new(secret_conn, &[0u8; 32])
            .expect("failed to create secret store");
        let secret_store: Arc<dyn nexa_core::ports::secrets::SecretStore> = Arc::new(secret_store);

        // Real Docker runtime
        let docker_runtime =
            DockerRuntime::new().expect("failed to create DockerRuntime (is Docker running?)");
        let runtime: Arc<dyn ContainerRuntime> = Arc::new(docker_runtime);

        // Transport
        let transport = LocalTransport::new(runtime.clone());
        let transport: Arc<dyn nexa_core::ports::cluster::ClusterTransport> = Arc::new(transport);

        // Route store
        let route_store: Arc<dyn nexa_core::ports::route_store::RouteStore> =
            Arc::new(InMemoryRouteStore::new());

        // Orchestrator
        let handle = Orchestrator::spawn(
            runtime,
            Some(store.clone()),
            Some(secret_store),
            Some(transport),
            None,
            None,
            None,
            Some(route_store),
        );

        // Build axum app
        let state = AppState {
            handle,
            store: store.clone(),
        };
        let app = routes::build(state);

        // Bind to a random port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind listener");
        let addr = listener.local_addr().expect("failed to get local addr");
        let base_url = format!("http://127.0.0.1:{}", addr.port());

        let base_url_clone = base_url.clone();
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server error");
        });

        // Wait for /health to respond (up to 5 seconds)
        let client = reqwest::Client::new();
        let health_url = format!("{}/health", base_url_clone);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if tokio::time::Instant::now() >= deadline {
                panic!("E2eServer did not become healthy within 5 seconds");
            }
            if let Ok(resp) = client.get(&health_url).send().await {
                if resp.status() == 200 {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Self {
            base_url: base_url_clone,
            _dir: dir,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

/// Build a deploy JSON body for busybox with a sleep command to keep it alive.
/// Note: DeploymentSpec has no `command` field, so the container will exit immediately —
/// the tests still verify the API lifecycle correctly.
fn deploy_body_busybox(project: &str, name: &str, replicas: u32) -> serde_json::Value {
    serde_json::json!({
        "project": project,
        "deployment": { "name": name },
        "replicas": replicas,
        "image": "busybox:latest",
        "ports": []
    })
}

async fn cleanup_containers() {
    let output = tokio::process::Command::new("docker")
        .args([
            "ps",
            "-a",
            "--filter",
            "name=nexa-e2e-",
            "--format",
            "{{.Names}}",
        ])
        .output()
        .await;
    if let Ok(output) = output {
        let names = String::from_utf8_lossy(&output.stdout);
        for name in names.lines() {
            let _ = tokio::process::Command::new("docker")
                .args(["rm", "-f", name])
                .output()
                .await;
        }
    }
}

// ---------------------------------------------------------------------------
// E2E Tests (ignored by default — run with:
//   cargo test --test e2e -- --ignored --test-threads=1
// Requires a running Docker daemon.)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn e2e_deploy_lifecycle() {
    let project = format!("e2e-{}", Uuid::new_v4().simple());
    let server = E2eServer::new().await;
    let c = client();

    // Create project
    let resp = c
        .post(server.url("/api/v1/projects"))
        .json(&serde_json::json!({ "name": &project }))
        .send()
        .await
        .expect("create project request failed");
    assert_eq!(
        resp.status(),
        201,
        "create project failed: {}",
        resp.status()
    );

    // Deploy busybox with 1 replica
    let body = serde_json::to_string(&deploy_body_busybox(&project, "sleeper", 1)).unwrap();
    let resp = c
        .post(server.url("/api/v1/deploy"))
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .expect("deploy request failed");
    assert_eq!(
        resp.status(),
        201,
        "deploy failed: {} — {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    // Wait for orchestrator to process
    tokio::time::sleep(Duration::from_secs(3)).await;

    // List pods — expect 1
    let resp = c
        .get(server.url(&format!("/api/v1/pods?project={project}")))
        .send()
        .await
        .expect("list pods request failed");
    assert_eq!(resp.status(), 200);
    let pods: serde_json::Value = resp.json().await.expect("invalid JSON");
    let pod_arr = pods.as_array().expect("expected array");
    assert_eq!(
        pod_arr.len(),
        1,
        "expected 1 pod, got {}: {:?}",
        pod_arr.len(),
        pods
    );

    // Stop deployment
    let resp = c
        .post(server.url(&format!(
            "/api/v1/projects/{project}/deployments/sleeper/stop"
        )))
        .send()
        .await
        .expect("stop request failed");
    assert_eq!(resp.status(), 200, "stop failed: {}", resp.status());

    // Remove deployment
    let resp = c
        .delete(server.url(&format!("/api/v1/projects/{project}/deployments/sleeper")))
        .send()
        .await
        .expect("remove deployment request failed");
    assert_eq!(
        resp.status(),
        200,
        "remove deployment failed: {}",
        resp.status()
    );

    // Delete project
    let resp = c
        .delete(server.url(&format!("/api/v1/projects/{project}")))
        .send()
        .await
        .expect("delete project request failed");
    assert_eq!(
        resp.status(),
        200,
        "delete project failed: {}",
        resp.status()
    );

    cleanup_containers().await;
}

#[tokio::test]
#[ignore]
async fn e2e_scale_up_down() {
    let project = format!("e2e-{}", Uuid::new_v4().simple());
    let server = E2eServer::new().await;
    let c = client();

    // Create project
    c.post(server.url("/api/v1/projects"))
        .json(&serde_json::json!({ "name": &project }))
        .send()
        .await
        .expect("create project request failed");

    // Deploy with 1 replica
    let body = serde_json::to_string(&deploy_body_busybox(&project, "scaler", 1)).unwrap();
    let resp = c
        .post(server.url("/api/v1/deploy"))
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .expect("deploy request failed");
    assert_eq!(resp.status(), 201, "deploy failed: {}", resp.status());

    tokio::time::sleep(Duration::from_secs(3)).await;

    // Scale up to 3
    let resp = c
        .post(server.url(&format!(
            "/api/v1/projects/{project}/deployments/scaler/scale"
        )))
        .json(&serde_json::json!({ "replicas": 3 }))
        .send()
        .await
        .expect("scale up request failed");
    assert_eq!(
        resp.status(),
        200,
        "scale up failed: {} — {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    tokio::time::sleep(Duration::from_secs(3)).await;

    // Verify 3 pods
    let resp = c
        .get(server.url(&format!("/api/v1/pods?project={project}")))
        .send()
        .await
        .expect("list pods request failed");
    let pods: serde_json::Value = resp.json().await.expect("invalid JSON");
    let pod_arr = pods.as_array().expect("expected array");
    assert_eq!(
        pod_arr.len(),
        3,
        "expected 3 pods after scale up, got {}: {:?}",
        pod_arr.len(),
        pods
    );

    // Scale down to 1
    let resp = c
        .post(server.url(&format!(
            "/api/v1/projects/{project}/deployments/scaler/scale"
        )))
        .json(&serde_json::json!({ "replicas": 1 }))
        .send()
        .await
        .expect("scale down request failed");
    assert_eq!(
        resp.status(),
        200,
        "scale down failed: {} — {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    tokio::time::sleep(Duration::from_secs(3)).await;

    // Verify 1 pod
    let resp = c
        .get(server.url(&format!("/api/v1/pods?project={project}")))
        .send()
        .await
        .expect("list pods request failed");
    let pods: serde_json::Value = resp.json().await.expect("invalid JSON");
    let pod_arr = pods.as_array().expect("expected array");
    assert_eq!(
        pod_arr.len(),
        1,
        "expected 1 pod after scale down, got {}: {:?}",
        pod_arr.len(),
        pods
    );

    // Cleanup
    let _ = c
        .post(server.url(&format!(
            "/api/v1/projects/{project}/deployments/scaler/stop"
        )))
        .send()
        .await;
    let _ = c
        .delete(server.url(&format!("/api/v1/projects/{project}/deployments/scaler")))
        .send()
        .await;
    let _ = c
        .delete(server.url(&format!("/api/v1/projects/{project}")))
        .send()
        .await;
    cleanup_containers().await;
}

#[tokio::test]
#[ignore]
async fn e2e_route_management() {
    let project = format!("e2e-{}", Uuid::new_v4().simple());
    let server = E2eServer::new().await;
    let c = client();

    // Create project
    c.post(server.url("/api/v1/projects"))
        .json(&serde_json::json!({ "name": &project }))
        .send()
        .await
        .expect("create project request failed");

    // Deploy
    let body = serde_json::to_string(&deploy_body_busybox(&project, "web", 1)).unwrap();
    let resp = c
        .post(server.url("/api/v1/deploy"))
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .expect("deploy request failed");
    assert_eq!(resp.status(), 201, "deploy failed: {}", resp.status());

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Add route for e2e-test.example.com
    let resp = c
        .post(server.url("/api/v1/routes"))
        .json(&serde_json::json!({
            "domain": "e2e-test.example.com",
            "project": &project,
            "deployment": "web",
            "tls_mode": "none"
        }))
        .send()
        .await
        .expect("add route request failed");
    assert_eq!(resp.status(), 201, "add route failed: {}", resp.status());

    // List routes — verify e2e-test.example.com appears
    let resp = c
        .get(server.url("/api/v1/routes"))
        .send()
        .await
        .expect("list routes request failed");
    assert_eq!(resp.status(), 200);
    let routes_val: serde_json::Value = resp.json().await.expect("invalid JSON");
    let route_arr = routes_val.as_array().expect("expected array");
    let domains: Vec<&str> = route_arr
        .iter()
        .filter_map(|r| r.get("domain").and_then(|d| d.as_str()))
        .collect();
    assert!(
        domains.contains(&"e2e-test.example.com"),
        "e2e-test.example.com not found in routes list: {domains:?}"
    );

    // Delete route
    let resp = c
        .delete(server.url("/api/v1/routes/e2e-test.example.com"))
        .send()
        .await
        .expect("delete route request failed");
    assert_eq!(resp.status(), 200, "delete route failed: {}", resp.status());

    // Verify route is gone
    let resp = c
        .get(server.url("/api/v1/routes"))
        .send()
        .await
        .expect("list routes request failed");
    let routes_val: serde_json::Value = resp.json().await.expect("invalid JSON");
    let route_arr = routes_val.as_array().expect("expected array");
    let domains: Vec<&str> = route_arr
        .iter()
        .filter_map(|r| r.get("domain").and_then(|d| d.as_str()))
        .collect();
    assert!(
        !domains.contains(&"e2e-test.example.com"),
        "e2e-test.example.com should be gone but still in routes: {domains:?}"
    );

    // Cleanup
    let _ = c
        .post(server.url(&format!("/api/v1/projects/{project}/deployments/web/stop")))
        .send()
        .await;
    let _ = c
        .delete(server.url(&format!("/api/v1/projects/{project}/deployments/web")))
        .send()
        .await;
    let _ = c
        .delete(server.url(&format!("/api/v1/projects/{project}")))
        .send()
        .await;
    cleanup_containers().await;
}
