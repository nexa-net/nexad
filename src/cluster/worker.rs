use std::sync::Arc;

use tracing::{error, info};

use nexa_core::ports::runtime::ContainerRuntime;
use nexa_core::ports::state::StateStore;

use super::heartbeat;
use super::proto;
use super::proto::cluster_service_client::ClusterServiceClient;
use super::server::start_grpc_server;

/// Start the daemon in worker mode.
///
/// 1. Resolve hostname and system resources
/// 2. Connect to master gRPC and register this worker
/// 3. Start local gRPC server (receives pod assignments from master)
/// 4. Start heartbeat loop
/// 5. Wait for either task to finish
pub async fn start_worker(
    master_addr: String,
    token: String,
    listen_addr: String,
    runtime: Arc<dyn ContainerRuntime>,
    state: Arc<dyn StateStore>,
) -> anyhow::Result<()> {
    // 1. Collect hostname and system resources.
    let hostname = hostname::get()
        .map_err(|e| anyhow::anyhow!("failed to get hostname: {e}"))?
        .to_string_lossy()
        .to_string();
    let resources = heartbeat::collect_resources();

    info!(
        hostname = %hostname,
        cpu_cores = resources.cpu_cores,
        memory_bytes = resources.memory_bytes,
        "worker starting"
    );

    // 2. Register with master.
    let endpoint = format!("http://{}", master_addr);
    let mut client = ClusterServiceClient::connect(endpoint).await?;

    let register_req = proto::RegisterRequest {
        node_name: hostname.clone(),
        node_address: listen_addr.clone(),
        token: token.clone(),
        resources: Some(proto::ResourceInfo {
            cpu_cores: resources.cpu_cores,
            memory_bytes: resources.memory_bytes,
            cpu_available: resources.cpu_available,
            memory_available: resources.memory_available,
            running_pods: resources.running_pods,
        }),
    };

    let resp = client.register(register_req).await?.into_inner();
    if !resp.accepted {
        anyhow::bail!("master rejected registration: {}", resp.message);
    }

    let node_id: uuid::Uuid = resp
        .node_id
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid node_id from master: {e}"))?;

    info!(node_id = %node_id, "registered with master");

    // 3. Start local gRPC server for pod assignments.
    // We use the token hash as the shared secret for any callbacks from master.
    let token_hash = super::token::hash_token(&token);
    let grpc_state = Arc::clone(&state);
    let grpc_runtime = Arc::clone(&runtime);
    let grpc_addr = listen_addr.clone();
    let grpc_handle = tokio::spawn(async move {
        if let Err(e) = start_grpc_server(&grpc_addr, grpc_runtime, grpc_state, token_hash).await {
            error!(error = %e, "worker gRPC server failed");
        }
    });

    // 4. Start heartbeat loop.
    let hb_master = master_addr.clone();
    let heartbeat_handle = tokio::spawn(async move {
        loop {
            if let Err(e) = heartbeat::run_heartbeat_sender(hb_master.clone(), node_id).await {
                error!(error = %e, "heartbeat stream disconnected, reconnecting in 5s");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });

    // 5. Wait for either to finish.
    tokio::select! {
        res = grpc_handle => {
            match res {
                Err(e) => error!(error = %e, "gRPC server task panicked"),
                Ok(_) => info!("gRPC server exited"),
            }
        }
        res = heartbeat_handle => {
            match res {
                Err(e) => error!(error = %e, "heartbeat task panicked"),
                Ok(_) => info!("heartbeat task exited"),
            }
        }
    }

    Ok(())
}
