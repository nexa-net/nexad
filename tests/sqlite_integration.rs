use std::collections::HashMap;

use nexa_core::domain::models::*;
use nexa_core::ports::state::StateStore;

#[tokio::test]
async fn full_lifecycle_with_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite:{}?mode=rwc", db_path.display());

    let store = nexad::adapters::state::SqliteStore::connect(&url)
        .await
        .expect("failed to connect to SQLite");

    let project = Project::new("integration");
    store.insert_project(&project).await.unwrap();

    let spec = DeploymentSpec {
        project: "integration".into(),
        deployment: DeploymentMeta { name: "api".into() },
        replicas: 2,
        image: "nginx:latest".into(),
        ports: vec![8080],
        env: HashMap::from([("ENV".into(), "test".into())]),
        volumes: vec![],
        network: None,
        healthcheck: None,
        restart: RestartPolicy::default(),
        secrets: vec![],
        resources: None,
    };
    let deployment = Deployment::from_spec(spec);
    store.insert_deployment(&deployment).await.unwrap();

    let pod0 = Pod::new(deployment.id, "integration", "api", 0, "nginx:latest");
    let pod1 = Pod::new(deployment.id, "integration", "api", 1, "nginx:latest");
    store.insert_pod(&pod0).await.unwrap();
    store.insert_pod(&pod1).await.unwrap();

    let projects = store.list_projects().await.unwrap();
    assert_eq!(projects.len(), 1);

    let deployments = store.list_deployments(Some("integration")).await.unwrap();
    assert_eq!(deployments.len(), 1);
    assert_eq!(deployments[0].spec.replicas, 2);
    assert_eq!(deployments[0].spec.env.get("ENV").unwrap(), "test");

    let pods = store.pods_by_deployment(&deployment.id).await.unwrap();
    assert_eq!(pods.len(), 2);

    let mut updated = deployments[0].clone();
    updated.status = DeploymentStatus::Running;
    store.update_deployment(&updated).await.unwrap();

    let refetched = store
        .get_deployment("integration", "api")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(refetched.status, DeploymentStatus::Running);

    store.delete_deployment(&deployment.id).await.unwrap();
    let remaining_pods = store.list_pods(None).await.unwrap();
    assert_eq!(remaining_pods.len(), 0, "cascade delete should remove pods");

    drop(store);
    let store2 = nexad::adapters::state::SqliteStore::connect(&url)
        .await
        .expect("reconnect failed");

    let projects2 = store2.list_projects().await.unwrap();
    assert_eq!(projects2.len(), 1, "project should survive reconnect");
    assert_eq!(projects2[0].name, "integration");
}

#[tokio::test]
async fn cascade_delete_project_removes_deployments_and_pods() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cascade_test.db");
    let url = format!("sqlite:{}?mode=rwc", db_path.display());

    let store = nexad::adapters::state::SqliteStore::connect(&url)
        .await
        .expect("failed to connect to SQLite");

    // Create project
    let project = Project::new("cascade-proj");
    store.insert_project(&project).await.unwrap();

    // Insert a deployment under the project
    let spec = DeploymentSpec {
        project: "cascade-proj".into(),
        deployment: DeploymentMeta { name: "web".into() },
        replicas: 1,
        image: "alpine:latest".into(),
        ports: vec![],
        env: HashMap::new(),
        volumes: vec![],
        network: None,
        healthcheck: None,
        restart: RestartPolicy::default(),
        secrets: vec![],
        resources: None,
    };
    let deployment = Deployment::from_spec(spec);
    store.insert_deployment(&deployment).await.unwrap();

    // Insert a pod under that deployment
    let pod = Pod::new(deployment.id, "cascade-proj", "web", 0, "alpine:latest");
    store.insert_pod(&pod).await.unwrap();

    // Verify they exist
    let deployments = store.list_deployments(Some("cascade-proj")).await.unwrap();
    assert_eq!(deployments.len(), 1);
    let pods = store.list_pods(Some("cascade-proj")).await.unwrap();
    assert_eq!(pods.len(), 1);

    // Delete the project — should cascade to deployments (and pods via deployment cascade)
    store.delete_project("cascade-proj").await.unwrap();

    // Verify deployments and pods are gone
    let deployments_after = store.list_deployments(Some("cascade-proj")).await.unwrap();
    assert_eq!(
        deployments_after.len(),
        0,
        "deployments should be removed when project is deleted"
    );
    let pods_after = store.list_pods(Some("cascade-proj")).await.unwrap();
    assert_eq!(
        pods_after.len(),
        0,
        "pods should be removed when project is deleted"
    );
}

#[tokio::test]
async fn node_crud_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("node_test.db");
    let url = format!("sqlite:{}?mode=rwc", db_path.display());

    let store = nexad::adapters::state::SqliteStore::connect(&url)
        .await
        .expect("failed to connect to SQLite");

    // Insert a node
    let resources = NodeResources {
        cpu_cores: 4.0,
        memory_bytes: 8 * 1024 * 1024 * 1024,
        cpu_available: 3.5,
        memory_available: 6 * 1024 * 1024 * 1024,
        running_pods: 0,
    };
    let node = Node::new(
        "worker-1".to_string(),
        "192.168.1.10".to_string(),
        NodeRole::Worker,
        resources,
    );
    store.insert_node(&node).await.unwrap();

    // List nodes — expect 1
    let nodes = store.list_nodes().await.unwrap();
    assert_eq!(nodes.len(), 1, "should have exactly 1 node after insert");
    assert_eq!(nodes[0].name, "worker-1");
    assert_eq!(nodes[0].status, NodeStatus::Ready);

    // Update node status to NotReady
    let mut updated_node = nodes[0].clone();
    updated_node.status = NodeStatus::NotReady;
    store.update_node(&updated_node).await.unwrap();

    // Verify status update
    let nodes_after_update = store.list_nodes().await.unwrap();
    assert_eq!(nodes_after_update.len(), 1);
    assert_eq!(
        nodes_after_update[0].status,
        NodeStatus::NotReady,
        "node status should be NotReady after update"
    );

    // Delete the node
    store.delete_node(&node.id).await.unwrap();

    // Verify list is empty
    let nodes_after_delete = store.list_nodes().await.unwrap();
    assert_eq!(
        nodes_after_delete.len(),
        0,
        "node list should be empty after deletion"
    );
}

#[tokio::test]
async fn concurrent_pod_inserts() {
    use std::sync::Arc;

    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("concurrent_test.db");
    let url = format!("sqlite:{}?mode=rwc", db_path.display());

    let store = Arc::new(
        nexad::adapters::state::SqliteStore::connect(&url)
            .await
            .expect("failed to connect to SQLite"),
    );

    // Set up project and deployment
    let project = Project::new("concurrent-proj");
    store.insert_project(&project).await.unwrap();

    let spec = DeploymentSpec {
        project: "concurrent-proj".into(),
        deployment: DeploymentMeta {
            name: "worker".into(),
        },
        replicas: 20,
        image: "busybox:latest".into(),
        ports: vec![],
        env: HashMap::new(),
        volumes: vec![],
        network: None,
        healthcheck: None,
        restart: RestartPolicy::default(),
        secrets: vec![],
        resources: None,
    };
    let deployment = Deployment::from_spec(spec);
    let deployment_id = deployment.id;
    store.insert_deployment(&deployment).await.unwrap();

    // Spawn 20 concurrent tasks each inserting one pod
    let mut handles = Vec::with_capacity(20);
    for i in 0..20u32 {
        let store_clone = Arc::clone(&store);
        let handle = tokio::spawn(async move {
            let pod = Pod::new(
                deployment_id,
                "concurrent-proj",
                "worker",
                i,
                "busybox:latest",
            );
            store_clone.insert_pod(&pod).await.unwrap();
        });
        handles.push(handle);
    }

    // Await all tasks
    for handle in handles {
        handle.await.expect("pod insert task panicked");
    }

    // Verify all 20 pods are in the database
    let pods = store.pods_by_deployment(&deployment_id).await.unwrap();
    assert_eq!(
        pods.len(),
        20,
        "all 20 concurrently inserted pods should be present"
    );
}
