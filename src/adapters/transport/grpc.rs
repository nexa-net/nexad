use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tonic::transport::Channel;
use tracing::{debug, info};
use uuid::Uuid;

use nexa_core::domain::models::*;
use nexa_core::error::{NexaError, Result};
use nexa_core::ports::cluster::ClusterTransport;
use nexa_core::ports::runtime::LogStream;

use crate::cluster::proto;
use crate::cluster::proto::cluster_service_client::ClusterServiceClient;

pub struct GrpcTransport {
    clients: Arc<RwLock<std::collections::HashMap<Uuid, ClusterServiceClient<Channel>>>>,
    master_addr: String,
}

impl GrpcTransport {
    pub fn new(master_addr: String) -> Self {
        Self {
            clients: Arc::new(RwLock::new(std::collections::HashMap::new())),
            master_addr,
        }
    }

    pub async fn add_client(&self, node_id: Uuid, address: &str) -> Result<()> {
        let endpoint = format!("http://{address}");
        let channel = Channel::from_shared(endpoint)
            .map_err(|e| NexaError::Runtime(format!("invalid endpoint: {e}")))?
            .connect()
            .await
            .map_err(|e| NexaError::Runtime(format!("gRPC connect failed: {e}")))?;
        let client = ClusterServiceClient::new(channel);
        self.clients.write().await.insert(node_id, client);
        info!(node_id = %node_id, "gRPC client connected");
        Ok(())
    }

    pub async fn remove_client(&self, node_id: &Uuid) {
        self.clients.write().await.remove(node_id);
    }

    async fn get_client(&self, node_id: &Uuid) -> Result<ClusterServiceClient<Channel>> {
        self.clients
            .read()
            .await
            .get(node_id)
            .cloned()
            .ok_or_else(|| NexaError::Runtime(format!("no gRPC client for node {node_id}")))
    }
}

#[async_trait]
impl ClusterTransport for GrpcTransport {
    async fn register_node(&self, node: &Node) -> Result<()> {
        let endpoint = format!("http://{}", self.master_addr);
        let channel = Channel::from_shared(endpoint)
            .map_err(|e| NexaError::Runtime(format!("invalid master endpoint: {e}")))?
            .connect()
            .await
            .map_err(|e| NexaError::Runtime(format!("cannot reach master: {e}")))?;
        let mut client = ClusterServiceClient::new(channel);
        let request = tonic::Request::new(proto::RegisterRequest {
            node_name: node.name.clone(),
            node_address: node.address.clone(),
            token: String::new(),
            resources: Some(proto::ResourceInfo {
                cpu_cores: node.resources.cpu_cores,
                memory_bytes: node.resources.memory_bytes,
                cpu_available: node.resources.cpu_available,
                memory_available: node.resources.memory_available,
                running_pods: node.resources.running_pods,
            }),
        });
        let response = client
            .register(request)
            .await
            .map_err(|e| NexaError::Runtime(format!("register RPC failed: {e}")))?;
        let resp = response.into_inner();
        if !resp.accepted {
            return Err(NexaError::Runtime(format!(
                "registration rejected: {}",
                resp.message
            )));
        }
        info!(node_id = resp.node_id, "registered with master");
        Ok(())
    }

    async fn heartbeat(
        &self,
        node_id: &Uuid,
        _status: &NodeStatus,
        _resources: &NodeResources,
    ) -> Result<()> {
        debug!(node_id = %node_id, "gRPC heartbeat (managed by stream)");
        Ok(())
    }

    async fn assign_pod(&self, node_id: &Uuid, pod: &Pod, spec: &DeploymentSpec) -> Result<()> {
        let mut client = self.get_client(node_id).await?;
        let pod_data = serde_json::to_vec(pod)
            .map_err(|e| NexaError::Runtime(format!("serialize pod: {e}")))?;
        let spec_data = serde_json::to_vec(spec)
            .map_err(|e| NexaError::Runtime(format!("serialize spec: {e}")))?;
        let request = tonic::Request::new(proto::AssignPodRequest {
            node_id: node_id.to_string(),
            pod_id: pod.id.to_string(),
            pod_data,
            deployment_spec: spec_data,
        });
        let response = client
            .assign_pod(request)
            .await
            .map_err(|e| NexaError::Runtime(format!("assign_pod RPC failed: {e}")))?;
        let resp = response.into_inner();
        if !resp.success {
            return Err(NexaError::Runtime(format!(
                "assign_pod rejected: {}",
                resp.message
            )));
        }
        info!(node_id = %node_id, pod_id = %pod.id, "pod assigned via gRPC");
        Ok(())
    }

    async fn stop_pod(&self, node_id: &Uuid, pod_id: &Uuid) -> Result<()> {
        let mut client = self.get_client(node_id).await?;
        let request = tonic::Request::new(proto::StopPodRequest {
            node_id: node_id.to_string(),
            pod_id: pod_id.to_string(),
        });
        let response = client
            .stop_pod(request)
            .await
            .map_err(|e| NexaError::Runtime(format!("stop_pod RPC failed: {e}")))?;
        if !response.into_inner().success {
            return Err(NexaError::Runtime("stop_pod rejected by worker".into()));
        }
        Ok(())
    }

    async fn remove_pod(&self, node_id: &Uuid, pod_id: &Uuid) -> Result<()> {
        let mut client = self.get_client(node_id).await?;
        let request = tonic::Request::new(proto::RemovePodRequest {
            node_id: node_id.to_string(),
            pod_id: pod_id.to_string(),
        });
        let response = client
            .remove_pod(request)
            .await
            .map_err(|e| NexaError::Runtime(format!("remove_pod RPC failed: {e}")))?;
        if !response.into_inner().success {
            return Err(NexaError::Runtime("remove_pod rejected by worker".into()));
        }
        Ok(())
    }

    async fn stream_logs(
        &self,
        node_id: &Uuid,
        pod_id: &Uuid,
        tail: Option<u64>,
    ) -> Result<LogStream> {
        let mut client = self.get_client(node_id).await?;
        let request = tonic::Request::new(proto::LogsRequest {
            node_id: node_id.to_string(),
            pod_id: pod_id.to_string(),
            tail: tail.unwrap_or(0),
        });
        let response = client
            .stream_logs(request)
            .await
            .map_err(|e| NexaError::Runtime(format!("stream_logs RPC failed: {e}")))?;
        let stream = response.into_inner();
        use futures::StreamExt;
        let mapped = stream.map(|result| match result {
            Ok(chunk) => Ok(chunk.line),
            Err(e) => Err(NexaError::Runtime(format!("log stream error: {e}"))),
        });
        Ok(Box::pin(mapped))
    }
}
