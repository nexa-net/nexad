use async_trait::async_trait;
use sqlx::{Row, SqlitePool, sqlite::SqliteRow};
use uuid::Uuid;

use nexa_core::domain::models::{
    Deployment, DeploymentSpec, DeploymentStatus, Pod, PodStatus, Project, ProjectStatus,
};
use nexa_core::error::{NexaError, Result};
use nexa_core::ports::state::StateStore;

pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;

        // Enable WAL and foreign key support
        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&pool)
            .await?;
        sqlx::query("PRAGMA foreign_keys=ON")
            .execute(&pool)
            .await?;

        // Run migrations
        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    fn row_to_deployment(row: &SqliteRow) -> Result<Deployment> {
        let id_str: String = row.get("id");
        let id = id_str
            .parse::<Uuid>()
            .map_err(|e| NexaError::Runtime(format!("invalid deployment id: {e}")))?;

        let spec_json: String = row.get("spec_json");
        let spec: DeploymentSpec = serde_json::from_str(&spec_json)
            .map_err(NexaError::Serialization)?;

        let status_str: String = row.get("status");
        let status = status_str
            .parse::<DeploymentStatus>()
            .map_err(|e| NexaError::Runtime(format!("invalid deployment status: {e}")))?;

        let created_at_str: String = row.get("created_at");
        let created_at = created_at_str
            .parse()
            .map_err(|e: chrono::ParseError| NexaError::Runtime(format!("invalid created_at: {e}")))?;

        let updated_at_str: String = row.get("updated_at");
        let updated_at = updated_at_str
            .parse()
            .map_err(|e: chrono::ParseError| NexaError::Runtime(format!("invalid updated_at: {e}")))?;

        Ok(Deployment {
            id,
            spec,
            status,
            created_at,
            updated_at,
        })
    }

    fn row_to_pod(row: &SqliteRow) -> Result<Pod> {
        let id_str: String = row.get("id");
        let id = id_str
            .parse::<Uuid>()
            .map_err(|e| NexaError::Runtime(format!("invalid pod id: {e}")))?;

        let deployment_id_str: String = row.get("deployment_id");
        let deployment_id = deployment_id_str
            .parse::<Uuid>()
            .map_err(|e| NexaError::Runtime(format!("invalid deployment_id: {e}")))?;

        let status_str: String = row.get("status");
        let status = status_str
            .parse::<PodStatus>()
            .map_err(|e| NexaError::Runtime(format!("invalid pod status: {e}")))?;

        let created_at_str: String = row.get("created_at");
        let created_at = created_at_str
            .parse()
            .map_err(|e: chrono::ParseError| NexaError::Runtime(format!("invalid created_at: {e}")))?;

        let replica_index: i64 = row.get("replica_index");
        let restart_count: i64 = row.get("restart_count");

        Ok(Pod {
            id,
            deployment_id,
            project: row.get("project"),
            deployment_name: row.get("deployment_name"),
            replica_index: replica_index as u32,
            container_id: row.get("container_id"),
            container_ip: row.get("container_ip"),
            status,
            image: row.get("image"),
            restart_count: restart_count as u32,
            created_at,
        })
    }
}

#[async_trait]
impl StateStore for SqliteStore {
    async fn insert_project(&self, project: &Project) -> Result<()> {
        let result = sqlx::query(
            "INSERT INTO projects (name, status, created_at) VALUES (?, ?, ?)",
        )
        .bind(&project.name)
        .bind(project.status.to_string())
        .bind(project.created_at.to_rfc3339())
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                Err(NexaError::InvalidSpec(format!(
                    "project '{}' already exists",
                    project.name
                )))
            }
            Err(e) => Err(NexaError::Runtime(e.to_string())),
        }
    }

    async fn get_project(&self, name: &str) -> Result<Option<Project>> {
        let row = sqlx::query("SELECT name, status, created_at FROM projects WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| NexaError::Runtime(e.to_string()))?;

        match row {
            None => Ok(None),
            Some(r) => {
                let status_str: String = r.get("status");
                let status = status_str
                    .parse::<ProjectStatus>()
                    .map_err(|e| NexaError::Runtime(format!("invalid project status: {e}")))?;
                let created_at_str: String = r.get("created_at");
                let created_at = created_at_str
                    .parse()
                    .map_err(|e: chrono::ParseError| NexaError::Runtime(format!("invalid created_at: {e}")))?;
                Ok(Some(Project {
                    name: r.get("name"),
                    status,
                    created_at,
                }))
            }
        }
    }

    async fn list_projects(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query("SELECT name, status, created_at FROM projects ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| NexaError::Runtime(e.to_string()))?;

        rows.iter()
            .map(|r| {
                let status_str: String = r.get("status");
                let status = status_str
                    .parse::<ProjectStatus>()
                    .map_err(|e| NexaError::Runtime(format!("invalid project status: {e}")))?;
                let created_at_str: String = r.get("created_at");
                let created_at = created_at_str
                    .parse()
                    .map_err(|e: chrono::ParseError| NexaError::Runtime(format!("invalid created_at: {e}")))?;
                Ok(Project {
                    name: r.get("name"),
                    status,
                    created_at,
                })
            })
            .collect()
    }

    async fn update_project_status(&self, name: &str, status: ProjectStatus) -> Result<()> {
        let rows_affected = sqlx::query("UPDATE projects SET status = ? WHERE name = ?")
            .bind(status.to_string())
            .bind(name)
            .execute(&self.pool)
            .await
            .map_err(|e| NexaError::Runtime(e.to_string()))?
            .rows_affected();

        if rows_affected == 0 {
            Err(NexaError::ProjectNotFound(name.to_string()))
        } else {
            Ok(())
        }
    }

    async fn delete_project(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM projects WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await
            .map_err(|e| NexaError::Runtime(e.to_string()))?;
        Ok(())
    }

    async fn insert_deployment(&self, deployment: &Deployment) -> Result<()> {
        let spec_json =
            serde_json::to_string(&deployment.spec).map_err(NexaError::Serialization)?;

        sqlx::query(
            "INSERT INTO deployments (id, project, name, spec_json, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(deployment.id.to_string())
        .bind(deployment.project())
        .bind(deployment.name())
        .bind(&spec_json)
        .bind(deployment.status.to_string())
        .bind(deployment.created_at.to_rfc3339())
        .bind(deployment.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| NexaError::Runtime(e.to_string()))?;

        Ok(())
    }

    async fn get_deployment(&self, project: &str, name: &str) -> Result<Option<Deployment>> {
        let row = sqlx::query(
            "SELECT id, spec_json, status, created_at, updated_at \
             FROM deployments WHERE project = ? AND name = ?",
        )
        .bind(project)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| NexaError::Runtime(e.to_string()))?;

        match row {
            None => Ok(None),
            Some(r) => Ok(Some(Self::row_to_deployment(&r)?)),
        }
    }

    async fn list_deployments(&self, project: Option<&str>) -> Result<Vec<Deployment>> {
        let rows = match project {
            Some(p) => {
                sqlx::query(
                    "SELECT id, spec_json, status, created_at, updated_at \
                     FROM deployments WHERE project = ? ORDER BY created_at",
                )
                .bind(p)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| NexaError::Runtime(e.to_string()))?
            }
            None => {
                sqlx::query(
                    "SELECT id, spec_json, status, created_at, updated_at \
                     FROM deployments ORDER BY created_at",
                )
                .fetch_all(&self.pool)
                .await
                .map_err(|e| NexaError::Runtime(e.to_string()))?
            }
        };

        rows.iter().map(|r| Self::row_to_deployment(r)).collect()
    }

    async fn update_deployment(&self, deployment: &Deployment) -> Result<()> {
        let spec_json =
            serde_json::to_string(&deployment.spec).map_err(NexaError::Serialization)?;

        let rows_affected = sqlx::query(
            "UPDATE deployments SET spec_json = ?, status = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&spec_json)
        .bind(deployment.status.to_string())
        .bind(deployment.updated_at.to_rfc3339())
        .bind(deployment.id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| NexaError::Runtime(e.to_string()))?
        .rows_affected();

        if rows_affected == 0 {
            Err(NexaError::DeploymentNotFound(deployment.id.to_string()))
        } else {
            Ok(())
        }
    }

    async fn delete_deployment(&self, id: &Uuid) -> Result<()> {
        sqlx::query("DELETE FROM deployments WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| NexaError::Runtime(e.to_string()))?;
        Ok(())
    }

    async fn insert_pod(&self, pod: &Pod) -> Result<()> {
        sqlx::query(
            "INSERT INTO pods \
             (id, deployment_id, project, deployment_name, replica_index, container_id, container_ip, status, image, restart_count, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(pod.id.to_string())
        .bind(pod.deployment_id.to_string())
        .bind(&pod.project)
        .bind(&pod.deployment_name)
        .bind(pod.replica_index as i64)
        .bind(&pod.container_id)
        .bind(&pod.container_ip)
        .bind(pod.status.to_string())
        .bind(&pod.image)
        .bind(pod.restart_count as i64)
        .bind(pod.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| NexaError::Runtime(e.to_string()))?;

        Ok(())
    }

    async fn list_pods(&self, project: Option<&str>) -> Result<Vec<Pod>> {
        let rows = match project {
            Some(p) => {
                sqlx::query(
                    "SELECT id, deployment_id, project, deployment_name, replica_index, \
                     container_id, container_ip, status, image, restart_count, created_at \
                     FROM pods WHERE project = ? ORDER BY created_at",
                )
                .bind(p)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| NexaError::Runtime(e.to_string()))?
            }
            None => {
                sqlx::query(
                    "SELECT id, deployment_id, project, deployment_name, replica_index, \
                     container_id, container_ip, status, image, restart_count, created_at \
                     FROM pods ORDER BY created_at",
                )
                .fetch_all(&self.pool)
                .await
                .map_err(|e| NexaError::Runtime(e.to_string()))?
            }
        };

        rows.iter().map(|r| Self::row_to_pod(r)).collect()
    }

    async fn update_pod(&self, pod: &Pod) -> Result<()> {
        let rows_affected = sqlx::query(
            "UPDATE pods SET container_id = ?, container_ip = ?, status = ?, restart_count = ? WHERE id = ?",
        )
        .bind(&pod.container_id)
        .bind(&pod.container_ip)
        .bind(pod.status.to_string())
        .bind(pod.restart_count as i64)
        .bind(pod.id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| NexaError::Runtime(e.to_string()))?
        .rows_affected();

        if rows_affected == 0 {
            Err(NexaError::PodNotFound(pod.id.to_string()))
        } else {
            Ok(())
        }
    }

    async fn delete_pod(&self, id: &Uuid) -> Result<()> {
        sqlx::query("DELETE FROM pods WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| NexaError::Runtime(e.to_string()))?;
        Ok(())
    }

    async fn pods_by_deployment(&self, deployment_id: &Uuid) -> Result<Vec<Pod>> {
        let rows = sqlx::query(
            "SELECT id, deployment_id, project, deployment_name, replica_index, \
             container_id, container_ip, status, image, restart_count, created_at \
             FROM pods WHERE deployment_id = ? ORDER BY replica_index",
        )
        .bind(deployment_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| NexaError::Runtime(e.to_string()))?;

        rows.iter().map(|r| Self::row_to_pod(r)).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use nexa_core::domain::models::{
        Deployment, DeploymentMeta, DeploymentSpec, DeploymentStatus, Pod, PodStatus, Project,
        ProjectStatus, RestartPolicy,
    };
    use nexa_core::ports::state::StateStore;

    use super::SqliteStore;

    async fn setup_store() -> SqliteStore {
        SqliteStore::connect("sqlite::memory:")
            .await
            .expect("failed to connect to in-memory SQLite")
    }

    fn make_spec(project: &str, name: &str) -> DeploymentSpec {
        DeploymentSpec {
            project: project.to_string(),
            deployment: DeploymentMeta { name: name.to_string() },
            replicas: 1,
            image: "nginx:latest".to_string(),
            ports: vec![80, 443],
            env: {
                let mut m = HashMap::new();
                m.insert("ENV".to_string(), "production".to_string());
                m
            },
            volumes: vec![],
            secrets: vec![],
            network: None,
            healthcheck: None,
            restart: RestartPolicy::default(),
            resources: None,
        }
    }

    // ---- Project tests ----

    #[tokio::test]
    async fn project_roundtrip() {
        let store = setup_store().await;
        let project = Project::new("myapp");

        store.insert_project(&project).await.unwrap();
        let fetched = store.get_project("myapp").await.unwrap();

        assert!(fetched.is_some());
        let p = fetched.unwrap();
        assert_eq!(p.name, "myapp");
        assert_eq!(p.status, ProjectStatus::Active);
    }

    #[tokio::test]
    async fn duplicate_project_fails() {
        let store = setup_store().await;
        let project = Project::new("dup");

        store.insert_project(&project).await.unwrap();
        let result = store.insert_project(&project).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn project_status_update() {
        let store = setup_store().await;
        let project = Project::new("myapp");
        store.insert_project(&project).await.unwrap();

        store
            .update_project_status("myapp", ProjectStatus::Suspended)
            .await
            .unwrap();

        let fetched = store.get_project("myapp").await.unwrap().unwrap();
        assert_eq!(fetched.status, ProjectStatus::Suspended);
    }

    // ---- Deployment tests ----

    #[tokio::test]
    async fn deployment_roundtrip() {
        let store = setup_store().await;
        // project must exist due to FK constraint
        store.insert_project(&Project::new("myapp")).await.unwrap();

        let spec = make_spec("myapp", "api");
        let deployment = Deployment::from_spec(spec);

        store.insert_deployment(&deployment).await.unwrap();

        let fetched = store.get_deployment("myapp", "api").await.unwrap();
        assert!(fetched.is_some());
        let d = fetched.unwrap();
        assert_eq!(d.id, deployment.id);
        assert_eq!(d.spec.image, "nginx:latest");
        assert_eq!(d.spec.ports, vec![80, 443]);
        assert_eq!(
            d.spec.env.get("ENV").map(String::as_str),
            Some("production")
        );
        assert_eq!(d.status, DeploymentStatus::Pending);
    }

    #[tokio::test]
    async fn deployment_update() {
        let store = setup_store().await;
        store.insert_project(&Project::new("myapp")).await.unwrap();

        let spec = make_spec("myapp", "api");
        let mut deployment = Deployment::from_spec(spec);
        store.insert_deployment(&deployment).await.unwrap();

        deployment.status = DeploymentStatus::Running;
        store.update_deployment(&deployment).await.unwrap();

        let fetched = store
            .get_deployment("myapp", "api")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.status, DeploymentStatus::Running);
    }

    // ---- Pod tests ----

    #[tokio::test]
    async fn pod_roundtrip() {
        let store = setup_store().await;
        store.insert_project(&Project::new("myapp")).await.unwrap();

        let spec = make_spec("myapp", "api");
        let deployment = Deployment::from_spec(spec);
        store.insert_deployment(&deployment).await.unwrap();

        let mut pod = Pod::new(deployment.id, "myapp", "api", 0, "nginx:latest");
        pod.container_id = Some("abc123".to_string());
        pod.restart_count = 3;

        store.insert_pod(&pod).await.unwrap();

        let pods = store.pods_by_deployment(&deployment.id).await.unwrap();
        assert_eq!(pods.len(), 1);
        let p = &pods[0];
        assert_eq!(p.id, pod.id);
        assert_eq!(p.container_id, Some("abc123".to_string()));
        assert_eq!(p.restart_count, 3);
        assert_eq!(p.status, PodStatus::Pending);
    }

    #[tokio::test]
    async fn delete_deployment_cascades_pods() {
        let store = setup_store().await;
        store.insert_project(&Project::new("myapp")).await.unwrap();

        let spec = make_spec("myapp", "api");
        let deployment = Deployment::from_spec(spec);
        store.insert_deployment(&deployment).await.unwrap();

        let pod = Pod::new(deployment.id, "myapp", "api", 0, "nginx:latest");
        store.insert_pod(&pod).await.unwrap();

        // Verify pod exists
        let pods_before = store.pods_by_deployment(&deployment.id).await.unwrap();
        assert_eq!(pods_before.len(), 1);

        // Delete deployment — cascade should remove pods
        store.delete_deployment(&deployment.id).await.unwrap();

        let pods_after = store.pods_by_deployment(&deployment.id).await.unwrap();
        assert_eq!(pods_after.len(), 0);
    }

    #[tokio::test]
    async fn list_pods_filters_by_project() {
        let store = setup_store().await;
        store.insert_project(&Project::new("alpha")).await.unwrap();
        store.insert_project(&Project::new("beta")).await.unwrap();

        let d_alpha = Deployment::from_spec(make_spec("alpha", "svc"));
        let d_beta = Deployment::from_spec(make_spec("beta", "svc"));
        store.insert_deployment(&d_alpha).await.unwrap();
        store.insert_deployment(&d_beta).await.unwrap();

        store
            .insert_pod(&Pod::new(d_alpha.id, "alpha", "svc", 0, "img:1"))
            .await
            .unwrap();
        store
            .insert_pod(&Pod::new(d_alpha.id, "alpha", "svc", 1, "img:1"))
            .await
            .unwrap();
        store
            .insert_pod(&Pod::new(d_beta.id, "beta", "svc", 0, "img:2"))
            .await
            .unwrap();

        let alpha_pods = store.list_pods(Some("alpha")).await.unwrap();
        assert_eq!(alpha_pods.len(), 2);

        let beta_pods = store.list_pods(Some("beta")).await.unwrap();
        assert_eq!(beta_pods.len(), 1);

        let all_pods = store.list_pods(None).await.unwrap();
        assert_eq!(all_pods.len(), 3);
    }
}
