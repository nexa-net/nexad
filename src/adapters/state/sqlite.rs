use async_trait::async_trait;
use sqlx::{Row, SqlitePool, sqlite::SqliteRow};
use uuid::Uuid;

use nexa_core::domain::models::{
    Deployment, DeploymentSpec, DeploymentStatus, Node, NodeResources, NodeRole, NodeStatus, Pod,
    PodStatus, Project, ProjectStatus,
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

    fn row_to_node(row: &SqliteRow) -> Result<Node> {
        let id_str: String = row.get("id");
        let id = id_str
            .parse::<Uuid>()
            .map_err(|e| NexaError::Runtime(format!("invalid node id: {e}")))?;

        let role_str: String = row.get("role");
        let role = role_str
            .parse::<NodeRole>()
            .map_err(|e| NexaError::Runtime(format!("invalid node role: {e}")))?;

        let status_str: String = row.get("status");
        let status = status_str
            .parse::<NodeStatus>()
            .map_err(|e| NexaError::Runtime(format!("invalid node status: {e}")))?;

        let cpu_cores: f64 = row.get("cpu_cores");
        let memory_bytes: i64 = row.get("memory_bytes");
        let cpu_available: f64 = row.get("cpu_available");
        let memory_available: i64 = row.get("memory_available");
        let running_pods: i32 = row.get("running_pods");

        let last_heartbeat_str: String = row.get("last_heartbeat");
        let last_heartbeat = last_heartbeat_str
            .parse()
            .map_err(|e: chrono::ParseError| NexaError::Runtime(format!("invalid last_heartbeat: {e}")))?;

        let joined_at_str: String = row.get("joined_at");
        let joined_at = joined_at_str
            .parse()
            .map_err(|e: chrono::ParseError| NexaError::Runtime(format!("invalid joined_at: {e}")))?;

        Ok(Node {
            id,
            name: row.get("name"),
            address: row.get("address"),
            role,
            status,
            resources: NodeResources {
                cpu_cores,
                memory_bytes: memory_bytes as u64,
                cpu_available,
                memory_available: memory_available as u64,
                running_pods: running_pods as u32,
            },
            last_heartbeat,
            joined_at,
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

        let node_id_str: Option<String> = row.get("node_id");
        let node_id = node_id_str
            .map(|s| s.parse::<Uuid>())
            .transpose()
            .map_err(|e| NexaError::Runtime(format!("invalid node_id: {e}")))?;

        Ok(Pod {
            id,
            deployment_id,
            project: row.get("project"),
            deployment_name: row.get("deployment_name"),
            replica_index: replica_index as u32,
            node_id,
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
             (id, deployment_id, project, deployment_name, replica_index, node_id, container_id, container_ip, status, image, restart_count, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(pod.id.to_string())
        .bind(pod.deployment_id.to_string())
        .bind(&pod.project)
        .bind(&pod.deployment_name)
        .bind(pod.replica_index as i64)
        .bind(pod.node_id.map(|id| id.to_string()))
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
                    "SELECT id, deployment_id, project, deployment_name, replica_index, node_id, \
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
                    "SELECT id, deployment_id, project, deployment_name, replica_index, node_id, \
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
            "UPDATE pods SET node_id = ?, container_id = ?, container_ip = ?, status = ?, restart_count = ? WHERE id = ?",
        )
        .bind(pod.node_id.map(|id| id.to_string()))
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
            "SELECT id, deployment_id, project, deployment_name, replica_index, node_id, \
             container_id, container_ip, status, image, restart_count, created_at \
             FROM pods WHERE deployment_id = ? ORDER BY replica_index",
        )
        .bind(deployment_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| NexaError::Runtime(e.to_string()))?;

        rows.iter().map(|r| Self::row_to_pod(r)).collect()
    }

    async fn insert_node(&self, node: &Node) -> Result<()> {
        let result = sqlx::query(
            "INSERT INTO nodes \
             (id, name, address, role, status, cpu_cores, memory_bytes, cpu_available, memory_available, running_pods, last_heartbeat, joined_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(node.id.to_string())
        .bind(&node.name)
        .bind(&node.address)
        .bind(node.role.to_string())
        .bind(node.status.to_string())
        .bind(node.resources.cpu_cores)
        .bind(node.resources.memory_bytes as i64)
        .bind(node.resources.cpu_available)
        .bind(node.resources.memory_available as i64)
        .bind(node.resources.running_pods as i32)
        .bind(node.last_heartbeat.to_rfc3339())
        .bind(node.joined_at.to_rfc3339())
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                Err(NexaError::InvalidSpec(format!(
                    "node '{}' already exists",
                    node.name
                )))
            }
            Err(e) => Err(NexaError::Runtime(e.to_string())),
        }
    }

    async fn get_node(&self, id: &Uuid) -> Result<Option<Node>> {
        let row = sqlx::query(
            "SELECT id, name, address, role, status, cpu_cores, memory_bytes, \
             cpu_available, memory_available, running_pods, last_heartbeat, joined_at \
             FROM nodes WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| NexaError::Runtime(e.to_string()))?;

        match row {
            None => Ok(None),
            Some(r) => Ok(Some(Self::row_to_node(&r)?)),
        }
    }

    async fn get_node_by_name(&self, name: &str) -> Result<Option<Node>> {
        let row = sqlx::query(
            "SELECT id, name, address, role, status, cpu_cores, memory_bytes, \
             cpu_available, memory_available, running_pods, last_heartbeat, joined_at \
             FROM nodes WHERE name = ?",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| NexaError::Runtime(e.to_string()))?;

        match row {
            None => Ok(None),
            Some(r) => Ok(Some(Self::row_to_node(&r)?)),
        }
    }

    async fn list_nodes(&self) -> Result<Vec<Node>> {
        let rows = sqlx::query(
            "SELECT id, name, address, role, status, cpu_cores, memory_bytes, \
             cpu_available, memory_available, running_pods, last_heartbeat, joined_at \
             FROM nodes ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| NexaError::Runtime(e.to_string()))?;

        rows.iter().map(|r| Self::row_to_node(r)).collect()
    }

    async fn update_node(&self, node: &Node) -> Result<()> {
        let rows_affected = sqlx::query(
            "UPDATE nodes SET name = ?, address = ?, role = ?, status = ?, \
             cpu_cores = ?, memory_bytes = ?, cpu_available = ?, memory_available = ?, \
             running_pods = ?, last_heartbeat = ? WHERE id = ?",
        )
        .bind(&node.name)
        .bind(&node.address)
        .bind(node.role.to_string())
        .bind(node.status.to_string())
        .bind(node.resources.cpu_cores)
        .bind(node.resources.memory_bytes as i64)
        .bind(node.resources.cpu_available)
        .bind(node.resources.memory_available as i64)
        .bind(node.resources.running_pods as i32)
        .bind(node.last_heartbeat.to_rfc3339())
        .bind(node.id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| NexaError::Runtime(e.to_string()))?
        .rows_affected();

        if rows_affected == 0 {
            Err(NexaError::NodeNotFound(node.id.to_string()))
        } else {
            Ok(())
        }
    }

    async fn delete_node(&self, id: &Uuid) -> Result<()> {
        sqlx::query("DELETE FROM nodes WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| NexaError::Runtime(e.to_string()))?;
        Ok(())
    }

    async fn get_cluster_config(&self, key: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT value FROM cluster_config WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| NexaError::Runtime(e.to_string()))?;

        Ok(row.map(|r| r.get("value")))
    }

    async fn set_cluster_config(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO cluster_config (key, value) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(|e| NexaError::Runtime(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use nexa_core::domain::models::{
        Deployment, DeploymentMeta, DeploymentSpec, DeploymentStatus, Node, NodeResources,
        NodeRole, NodeStatus, Pod, PodStatus, Project, ProjectStatus, RestartPolicy,
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

    // ---- Node tests ----

    fn sample_resources() -> NodeResources {
        NodeResources {
            cpu_cores: 4.0,
            memory_bytes: 8_589_934_592,
            cpu_available: 3.5,
            memory_available: 7_000_000_000,
            running_pods: 2,
        }
    }

    #[tokio::test]
    async fn node_insert_and_get() {
        let store = setup_store().await;
        let node = Node::new(
            "worker-1".into(),
            "192.168.1.1:9000".into(),
            NodeRole::Worker,
            sample_resources(),
        );
        let node_id = node.id;

        store.insert_node(&node).await.unwrap();
        let fetched = store.get_node(&node_id).await.unwrap();

        assert!(fetched.is_some());
        let n = fetched.unwrap();
        assert_eq!(n.name, "worker-1");
        assert_eq!(n.address, "192.168.1.1:9000");
        assert_eq!(n.role, NodeRole::Worker);
        assert_eq!(n.status, NodeStatus::Ready);
        assert_eq!(n.resources.cpu_cores, 4.0);
        assert_eq!(n.resources.memory_bytes, 8_589_934_592);
        assert_eq!(n.resources.running_pods, 2);
    }

    #[tokio::test]
    async fn node_get_by_name() {
        let store = setup_store().await;
        let node = Node::new(
            "master-1".into(),
            "10.0.0.1:9000".into(),
            NodeRole::Master,
            sample_resources(),
        );
        store.insert_node(&node).await.unwrap();

        let fetched = store.get_node_by_name("master-1").await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, node.id);

        let missing = store.get_node_by_name("nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn node_list() {
        let store = setup_store().await;
        let n1 = Node::new("w1".into(), "10.0.0.1:9000".into(), NodeRole::Worker, sample_resources());
        let n2 = Node::new("w2".into(), "10.0.0.2:9000".into(), NodeRole::Worker, sample_resources());

        store.insert_node(&n1).await.unwrap();
        store.insert_node(&n2).await.unwrap();

        let all = store.list_nodes().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn node_update() {
        let store = setup_store().await;
        let mut node = Node::new(
            "worker-1".into(),
            "10.0.0.1:9000".into(),
            NodeRole::Worker,
            sample_resources(),
        );
        store.insert_node(&node).await.unwrap();

        node.status = NodeStatus::Draining;
        node.resources.running_pods = 0;
        store.update_node(&node).await.unwrap();

        let fetched = store.get_node(&node.id).await.unwrap().unwrap();
        assert_eq!(fetched.status, NodeStatus::Draining);
        assert_eq!(fetched.resources.running_pods, 0);
    }

    #[tokio::test]
    async fn node_update_not_found() {
        let store = setup_store().await;
        let phantom = Node::new("ghost".into(), "0.0.0.0:0".into(), NodeRole::Worker, sample_resources());
        let result = store.update_node(&phantom).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn node_delete() {
        let store = setup_store().await;
        let node = Node::new("w1".into(), "10.0.0.1:9000".into(), NodeRole::Worker, sample_resources());
        let node_id = node.id;

        store.insert_node(&node).await.unwrap();
        store.delete_node(&node_id).await.unwrap();

        let fetched = store.get_node(&node_id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn node_duplicate_name_fails() {
        let store = setup_store().await;
        let n1 = Node::new("worker-1".into(), "10.0.0.1:9000".into(), NodeRole::Worker, sample_resources());
        let n2 = Node::new("worker-1".into(), "10.0.0.2:9000".into(), NodeRole::Worker, sample_resources());

        store.insert_node(&n1).await.unwrap();
        let result = store.insert_node(&n2).await;
        assert!(result.is_err());
    }

    // ---- Cluster config tests ----

    #[tokio::test]
    async fn cluster_config_roundtrip() {
        let store = setup_store().await;

        // Initially empty
        let val = store.get_cluster_config("leader_id").await.unwrap();
        assert!(val.is_none());

        // Set and get
        store.set_cluster_config("leader_id", "node-abc").await.unwrap();
        let val = store.get_cluster_config("leader_id").await.unwrap();
        assert_eq!(val.as_deref(), Some("node-abc"));

        // Overwrite
        store.set_cluster_config("leader_id", "node-xyz").await.unwrap();
        let val = store.get_cluster_config("leader_id").await.unwrap();
        assert_eq!(val.as_deref(), Some("node-xyz"));
    }
}
