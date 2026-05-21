use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, info};
use uuid::Uuid;

use nexa_core::domain::models::*;
use nexa_core::error::{NexaError, Result};
use nexa_core::ports::cluster::ClusterTransport;
use nexa_core::ports::runtime::*;

pub struct LocalTransport {
    runtime: Arc<dyn ContainerRuntime>,
}

impl LocalTransport {
    pub fn new(runtime: Arc<dyn ContainerRuntime>) -> Self {
        Self { runtime }
    }
}

#[async_trait]
impl ClusterTransport for LocalTransport {
    async fn register_node(&self, node: &Node) -> Result<()> {
        debug!(name = node.name, "local node registered (no-op)");
        Ok(())
    }

    async fn heartbeat(
        &self,
        _node_id: &Uuid,
        _status: &NodeStatus,
        _resources: &NodeResources,
    ) -> Result<()> {
        Ok(())
    }

    async fn assign_pod(
        &self,
        _node_id: &Uuid,
        pod: &Pod,
        spec: &DeploymentSpec,
    ) -> Result<()> {
        let container_name = pod.container_name();
        let network_name = format!("nexa-{}", spec.project);

        info!(name = container_name, image = spec.image, "local: creating pod");

        if let Err(e) = self.runtime.pull_image(&spec.image).await {
            tracing::warn!(image = spec.image, error = %e, "image pull failed, trying local");
        }

        if self.runtime.container_exists(&container_name).await? {
            let _ = self.runtime.stop_container(&container_name, 5).await;
            let _ = self.runtime.remove_container(&container_name, true).await;
        }

        if !self
            .runtime
            .container_exists(&network_name)
            .await
            .unwrap_or(false)
        {
            let _ = self.runtime.create_network(&network_name).await;
        }

        let ports: Vec<PortBinding> = spec
            .ports
            .iter()
            .map(|&p| PortBinding {
                container_port: p,
                host_port: if spec.replicas == 1 { Some(p) } else { None },
            })
            .collect();

        let mut labels = HashMap::new();
        labels.insert("managed-by".to_string(), "nexanet".to_string());
        labels.insert("nexa.project".to_string(), spec.project.clone());
        labels.insert(
            "nexa.deployment".to_string(),
            spec.deployment.name.clone(),
        );
        labels.insert("nexa.pod-id".to_string(), pod.id.to_string());

        // VolumeSpec uses source_name() and mount_point() methods
        let volumes: Vec<VolumeBinding> = spec
            .volumes
            .iter()
            .map(|v| VolumeBinding {
                source: v.source_name().to_string(),
                target: v.mount_point().to_string(),
                read_only: v.is_read_only(),
            })
            .collect();

        let config = ContainerConfig {
            name: container_name.clone(),
            image: spec.image.clone(),
            env: spec.env.clone(),
            ports,
            volumes,
            labels,
            network: Some(network_name),
        };

        let container_id = self.runtime.create_container(&config).await?;
        self.runtime.start_container(&container_id).await?;

        info!(name = container_name, container_id, "local: pod running");
        Ok(())
    }

    async fn stop_pod(&self, _node_id: &Uuid, pod_id: &Uuid) -> Result<()> {
        debug!(pod_id = %pod_id, "local: stop_pod requested");
        Ok(())
    }

    async fn remove_pod(&self, _node_id: &Uuid, pod_id: &Uuid) -> Result<()> {
        debug!(pod_id = %pod_id, "local: remove_pod requested");
        Ok(())
    }

    async fn stream_logs(
        &self,
        _node_id: &Uuid,
        _pod_id: &Uuid,
        _tail: Option<u64>,
    ) -> Result<LogStream> {
        Err(NexaError::Runtime(
            "local transport: use runtime.logs() directly".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRuntime {
        containers_created: std::sync::Mutex<Vec<String>>,
    }

    impl MockRuntime {
        fn new() -> Self {
            Self {
                containers_created: std::sync::Mutex::new(Vec::new()),
            }
        }
        fn created_count(&self) -> usize {
            self.containers_created.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl ContainerRuntime for MockRuntime {
        async fn pull_image(&self, _image: &str) -> Result<()> {
            Ok(())
        }
        async fn create_container(&self, config: &ContainerConfig) -> Result<String> {
            let id = format!("mock-{}", config.name);
            self.containers_created.lock().unwrap().push(id.clone());
            Ok(id)
        }
        async fn start_container(&self, _id: &str) -> Result<()> {
            Ok(())
        }
        async fn stop_container(&self, _id: &str, _timeout_secs: u64) -> Result<()> {
            Ok(())
        }
        async fn remove_container(&self, _id: &str, _force: bool) -> Result<()> {
            Ok(())
        }
        async fn inspect_container(&self, _id: &str) -> Result<ContainerInfo> {
            Ok(ContainerInfo {
                id: "mock".into(),
                name: "mock".into(),
                image: "mock".into(),
                state: ContainerState::Running,
            })
        }
        async fn logs(&self, _id: &str, _tail: Option<u64>) -> Result<LogStream> {
            Ok(Box::pin(futures::stream::empty()))
        }
        async fn container_exists(&self, _name: &str) -> Result<bool> {
            Ok(false)
        }
        async fn create_network(&self, _name: &str) -> Result<String> {
            Ok("mock-net".into())
        }
        async fn remove_network(&self, _name: &str) -> Result<()> {
            Ok(())
        }
        async fn connect_to_network(&self, _c: &str, _n: &str) -> Result<()> {
            Ok(())
        }
        async fn container_ip(&self, _c: &str, _n: &str) -> Result<String> {
            Ok("172.17.0.2".into())
        }
        async fn events(&self) -> Result<EventStream> {
            Ok(Box::pin(futures::stream::empty()))
        }
    }

    #[tokio::test]
    async fn local_register_node_is_noop() {
        let runtime = Arc::new(MockRuntime::new());
        let transport = LocalTransport::new(runtime);
        let node = Node::new(
            "local".into(),
            "127.0.0.1:6444".into(),
            NodeRole::Master,
            NodeResources::zero(),
        );
        assert!(transport.register_node(&node).await.is_ok());
    }

    #[tokio::test]
    async fn local_heartbeat_is_noop() {
        let runtime = Arc::new(MockRuntime::new());
        let transport = LocalTransport::new(runtime);
        assert!(transport
            .heartbeat(&Uuid::new_v4(), &NodeStatus::Ready, &NodeResources::zero())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn local_assign_pod_creates_container() {
        let runtime = Arc::new(MockRuntime::new());
        let transport = LocalTransport::new(runtime.clone());

        let spec = DeploymentSpec {
            project: "test".into(),
            deployment: DeploymentMeta {
                name: "api".into(),
            },
            replicas: 1,
            image: "nginx:latest".into(),
            ports: vec![8080],
            env: HashMap::new(),
            volumes: vec![],
            secrets: vec![],
            network: None,
            healthcheck: None,
            restart: RestartPolicy::default(),
            resources: None,
        };

        let pod = Pod::new(Uuid::new_v4(), "test", "api", 0, "nginx:latest");
        transport
            .assign_pod(&Uuid::new_v4(), &pod, &spec)
            .await
            .unwrap();
        assert_eq!(runtime.created_count(), 1);
    }
}
