use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{error, info, warn};
use uuid::Uuid;

use nexa_core::domain::models::*;
use nexa_core::ports::runtime::ContainerRuntime;
use nexa_core::ports::state::StateStore;

use super::proto;
use super::proto::cluster_service_server::ClusterService;

pub struct ClusterServer {
    runtime: Arc<dyn ContainerRuntime>,
    state: Arc<dyn StateStore>,
    token_hash: String,
}

impl ClusterServer {
    pub fn new(
        runtime: Arc<dyn ContainerRuntime>,
        state: Arc<dyn StateStore>,
        token_hash: String,
    ) -> Self {
        Self {
            runtime,
            state,
            token_hash,
        }
    }

    fn verify_token(&self, token: &str) -> bool {
        use sha2::{Digest, Sha256};
        let hash = hex::encode(Sha256::digest(token.as_bytes()));
        hash == self.token_hash
    }
}

#[tonic::async_trait]
impl ClusterService for ClusterServer {
    async fn register(
        &self,
        request: Request<proto::RegisterRequest>,
    ) -> std::result::Result<Response<proto::RegisterResponse>, Status> {
        let req = request.into_inner();
        if !self.verify_token(&req.token) {
            return Ok(Response::new(proto::RegisterResponse {
                node_id: String::new(),
                accepted: false,
                message: "invalid join token".into(),
            }));
        }
        if let Ok(Some(_)) = self.state.get_node_by_name(&req.node_name).await {
            return Ok(Response::new(proto::RegisterResponse {
                node_id: String::new(),
                accepted: false,
                message: format!("node '{}' already registered", req.node_name),
            }));
        }
        let resources = req
            .resources
            .map(|r| NodeResources {
                cpu_cores: r.cpu_cores,
                memory_bytes: r.memory_bytes,
                cpu_available: r.cpu_available,
                memory_available: r.memory_available,
                running_pods: r.running_pods,
            })
            .unwrap_or_else(NodeResources::zero);
        let node = Node::new(
            req.node_name.clone(),
            req.node_address.clone(),
            NodeRole::Worker,
            resources,
        );
        let node_id = node.id;
        if let Err(e) = self.state.insert_node(&node).await {
            error!(error = %e, "failed to persist node");
            return Err(Status::internal("failed to register node"));
        }
        info!(
            node_id = %node_id,
            name = req.node_name,
            address = req.node_address,
            "worker registered"
        );
        Ok(Response::new(proto::RegisterResponse {
            node_id: node_id.to_string(),
            accepted: true,
            message: "registered".into(),
        }))
    }

    type HeartbeatStream =
        Pin<Box<dyn Stream<Item = std::result::Result<proto::HeartbeatPong, Status>> + Send>>;

    async fn heartbeat(
        &self,
        request: Request<Streaming<proto::HeartbeatPing>>,
    ) -> std::result::Result<Response<Self::HeartbeatStream>, Status> {
        let mut in_stream = request.into_inner();
        let state = self.state.clone();
        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(async move {
            while let Ok(Some(ping)) = in_stream.message().await {
                let node_id = match Uuid::parse_str(&ping.node_id) {
                    Ok(id) => id,
                    Err(_) => {
                        warn!(raw_id = ping.node_id, "invalid node_id in heartbeat");
                        continue;
                    }
                };
                if let Ok(Some(mut node)) = state.get_node(&node_id).await {
                    node.last_heartbeat = chrono::Utc::now();
                    node.status = match ping.status.as_str() {
                        "ready" => NodeStatus::Ready,
                        "draining" => NodeStatus::Draining,
                        _ => NodeStatus::NotReady,
                    };
                    if let Some(res) = &ping.resources {
                        node.resources = NodeResources {
                            cpu_cores: res.cpu_cores,
                            memory_bytes: res.memory_bytes,
                            cpu_available: res.cpu_available,
                            memory_available: res.memory_available,
                            running_pods: res.running_pods,
                        };
                    }
                    let _ = state.update_node(&node).await;
                }
                let pong = proto::HeartbeatPong {
                    acknowledged: true,
                    pending_actions: vec![],
                };
                if tx.send(Ok(pong)).await.is_err() {
                    break;
                }
            }
        });
        let out_stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(out_stream)))
    }

    async fn assign_pod(
        &self,
        request: Request<proto::AssignPodRequest>,
    ) -> std::result::Result<Response<proto::AssignPodResponse>, Status> {
        let req = request.into_inner();
        let pod: Pod = serde_json::from_slice(&req.pod_data)
            .map_err(|e| Status::invalid_argument(format!("bad pod data: {e}")))?;
        let spec: DeploymentSpec = serde_json::from_slice(&req.deployment_spec)
            .map_err(|e| Status::invalid_argument(format!("bad spec: {e}")))?;
        let container_name = pod.container_name();
        let network_name = format!("nexa-{}", spec.project);

        info!(name = container_name, "worker: creating pod");

        if let Err(e) = self.runtime.pull_image(&spec.image).await {
            warn!(error = %e, "image pull failed, trying local");
        }

        if self
            .runtime
            .container_exists(&container_name)
            .await
            .unwrap_or(false)
        {
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

        use nexa_core::ports::runtime::*;

        let ports: Vec<PortBinding> = spec
            .ports
            .iter()
            .map(|&p| PortBinding {
                container_port: p,
                host_port: if spec.replicas == 1 { Some(p) } else { None },
            })
            .collect();

        let mut labels = std::collections::HashMap::new();
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
            dns: vec![],
            dns_search: vec![],
        };

        match self.runtime.create_container(&config).await {
            Ok(container_id) => {
                if let Err(e) = self.runtime.start_container(&container_id).await {
                    return Ok(Response::new(proto::AssignPodResponse {
                        success: false,
                        message: format!("start failed: {e}"),
                        container_id: String::new(),
                    }));
                }
                info!(name = container_name, "worker: pod running");
                Ok(Response::new(proto::AssignPodResponse {
                    success: true,
                    message: "running".into(),
                    container_id,
                }))
            }
            Err(e) => Ok(Response::new(proto::AssignPodResponse {
                success: false,
                message: format!("create failed: {e}"),
                container_id: String::new(),
            })),
        }
    }

    async fn stop_pod(
        &self,
        request: Request<proto::StopPodRequest>,
    ) -> std::result::Result<Response<proto::StopPodResponse>, Status> {
        let req = request.into_inner();
        info!(pod_id = req.pod_id, "worker: stopping pod");
        Ok(Response::new(proto::StopPodResponse {
            success: true,
            message: "stopped".into(),
        }))
    }

    async fn remove_pod(
        &self,
        request: Request<proto::RemovePodRequest>,
    ) -> std::result::Result<Response<proto::RemovePodResponse>, Status> {
        let req = request.into_inner();
        info!(pod_id = req.pod_id, "worker: removing pod");
        Ok(Response::new(proto::RemovePodResponse {
            success: true,
            message: "removed".into(),
        }))
    }

    async fn report_status(
        &self,
        request: Request<proto::StatusReport>,
    ) -> std::result::Result<Response<proto::Empty>, Status> {
        let report = request.into_inner();
        let node_id = Uuid::parse_str(&report.node_id)
            .map_err(|_| Status::invalid_argument("bad node_id"))?;
        for ps in &report.pod_statuses {
            info!(
                node_id = %node_id,
                pod_id = ps.pod_id,
                status = ps.status,
                "status report from worker"
            );
        }
        Ok(Response::new(proto::Empty {}))
    }

    type StreamLogsStream =
        Pin<Box<dyn Stream<Item = std::result::Result<proto::LogChunk, Status>> + Send>>;

    async fn stream_logs(
        &self,
        request: Request<proto::LogsRequest>,
    ) -> std::result::Result<Response<Self::StreamLogsStream>, Status> {
        let _req = request.into_inner();
        let (tx, rx) = mpsc::channel(64);
        drop(tx); // Empty stream for now
        let out_stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(out_stream)))
    }
}

pub async fn start_grpc_server(
    addr: &str,
    runtime: Arc<dyn ContainerRuntime>,
    state: Arc<dyn StateStore>,
    token_hash: String,
) -> anyhow::Result<()> {
    use proto::cluster_service_server::ClusterServiceServer;

    let service = ClusterServer::new(runtime, state, token_hash);
    let addr = addr.parse().map_err(|e| anyhow::anyhow!("bad addr: {e}"))?;
    info!("gRPC cluster server listening on {addr}");

    tonic::transport::Server::builder()
        .add_service(ClusterServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
