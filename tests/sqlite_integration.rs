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
