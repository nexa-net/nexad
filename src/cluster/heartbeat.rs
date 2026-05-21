use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio_stream::StreamExt;
use tonic::Request;
use tracing::{error, info, warn};
use uuid::Uuid;

use nexa_core::domain::models::*;
use nexa_core::ports::state::StateStore;

use super::proto;
use super::proto::cluster_service_client::ClusterServiceClient;

/// Callback invoked when a dead node's pods need rescheduling.
pub type RescheduleFn = Arc<dyn Fn(Uuid, Vec<Pod>) + Send + Sync>;

/// How often the monitor checks all nodes.
const MONITOR_INTERVAL: Duration = Duration::from_secs(10);

/// A node with no heartbeat for this duration is marked NotReady.
const NOT_READY_THRESHOLD: Duration = Duration::from_secs(30);

/// A node with no heartbeat for this duration is considered dead and its pods
/// are rescheduled.
const DEAD_THRESHOLD: Duration = Duration::from_secs(60);

/// How often a worker sends a heartbeat to the master.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

// ───────────────────────── master-side monitor ─────────────────────────

/// Runs on the master node. Periodically scans all worker nodes and marks
/// stale ones as NotReady or triggers rescheduling when they are dead.
pub async fn run_monitor(state: Arc<dyn StateStore>, reschedule: RescheduleFn) {
    info!("heartbeat monitor started");
    let mut tick = tokio::time::interval(MONITOR_INTERVAL);

    loop {
        tick.tick().await;
        if let Err(e) = check_nodes(&state, &reschedule).await {
            error!(error = %e, "heartbeat monitor check failed");
        }
    }
}

/// Single pass: inspect every node and act on stale heartbeats.
pub async fn check_nodes(
    state: &Arc<dyn StateStore>,
    reschedule: &RescheduleFn,
) -> anyhow::Result<()> {
    let nodes = state.list_nodes().await?;
    let now = Utc::now();

    for mut node in nodes {
        // Skip master nodes — they are not monitored via heartbeat.
        if node.role == NodeRole::Master {
            continue;
        }

        let elapsed = now
            .signed_duration_since(node.last_heartbeat)
            .to_std()
            .unwrap_or(Duration::ZERO);

        if elapsed >= DEAD_THRESHOLD {
            if node.status != NodeStatus::NotReady {
                warn!(
                    node_id = %node.id,
                    name = %node.name,
                    elapsed_secs = elapsed.as_secs(),
                    "node dead — triggering reschedule"
                );
                node.status = NodeStatus::NotReady;
                let _ = state.update_node(&node).await;
            }

            // Collect pods running on this dead node and reschedule.
            let all_pods = state.list_pods(None).await?;
            let node_pods: Vec<Pod> = all_pods
                .into_iter()
                .filter(|p| p.node_id == Some(node.id))
                .collect();

            if !node_pods.is_empty() {
                reschedule(node.id, node_pods);
            }
        } else if elapsed >= NOT_READY_THRESHOLD {
            if node.status != NodeStatus::NotReady {
                warn!(
                    node_id = %node.id,
                    name = %node.name,
                    elapsed_secs = elapsed.as_secs(),
                    "node stale — marking NotReady"
                );
                node.status = NodeStatus::NotReady;
                let _ = state.update_node(&node).await;
            }
        }
    }

    Ok(())
}

// ───────────────────── worker-side heartbeat sender ────────────────────

/// Collects current system resources using `sysinfo`.
pub fn collect_resources() -> NodeResources {
    use sysinfo::System;

    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_cores = sys.cpus().len() as f64;
    let memory_bytes = sys.total_memory();
    let memory_available = sys.available_memory();

    // Average CPU usage across all cores, as a fraction of total cores.
    let cpu_used: f64 = sys
        .cpus()
        .iter()
        .map(|c| f64::from(c.cpu_usage()) / 100.0)
        .sum();
    let cpu_available = (cpu_cores - cpu_used).max(0.0);

    NodeResources {
        cpu_cores,
        memory_bytes,
        cpu_available,
        memory_available,
        running_pods: 0, // filled in by the caller if needed
    }
}

/// Runs on a worker. Opens a bidirectional heartbeat stream to the master and
/// sends periodic pings. Returns only on connection failure.
pub async fn run_heartbeat_sender(
    master_addr: String,
    node_id: Uuid,
) -> anyhow::Result<()> {
    let endpoint = format!("http://{}", master_addr);
    let mut client = ClusterServiceClient::connect(endpoint).await?;

    let (tx, rx) = tokio::sync::mpsc::channel::<proto::HeartbeatPing>(32);

    // Spawn sender task.
    let sender = tokio::spawn(async move {
        let mut tick = tokio::time::interval(HEARTBEAT_INTERVAL);
        loop {
            tick.tick().await;

            let resources = collect_resources();
            let ping = proto::HeartbeatPing {
                node_id: node_id.to_string(),
                status: "ready".into(),
                resources: Some(proto::ResourceInfo {
                    cpu_cores: resources.cpu_cores,
                    memory_bytes: resources.memory_bytes,
                    cpu_available: resources.cpu_available,
                    memory_available: resources.memory_available,
                    running_pods: resources.running_pods,
                }),
                pod_statuses: vec![],
            };

            if tx.send(ping).await.is_err() {
                break;
            }
        }
    });

    let in_stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    let response = client.heartbeat(Request::new(in_stream)).await?;
    let mut pong_stream = response.into_inner();

    while let Some(result) = pong_stream.next().await {
        match result {
            Ok(_pong) => { /* acknowledged */ }
            Err(e) => {
                error!(error = %e, "heartbeat stream error");
                break;
            }
        }
    }

    sender.abort();
    Ok(())
}

// ───────────────────────────── tests ───────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    use nexa_core::ports::state_memory::InMemoryStore;

    #[tokio::test]
    async fn monitor_marks_stale_node_not_ready() {
        let store: Arc<dyn StateStore> = Arc::new(InMemoryStore::new());

        // Create a worker node with a heartbeat 35 seconds ago.
        let mut node = Node::new(
            "stale-worker".into(),
            "10.0.0.2:6444".into(),
            NodeRole::Worker,
            NodeResources::zero(),
        );
        node.last_heartbeat = Utc::now() - chrono::Duration::seconds(35);
        store.insert_node(&node).await.unwrap();

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let reschedule: RescheduleFn = Arc::new(move |_node_id, _pods| {
            called_clone.store(true, Ordering::SeqCst);
        });

        check_nodes(&store, &reschedule).await.unwrap();

        let updated = store.get_node(&node.id).await.unwrap().unwrap();
        assert_eq!(
            updated.status,
            NodeStatus::NotReady,
            "node should be marked NotReady after 35s without heartbeat"
        );
        // 35s is past NOT_READY but before DEAD, so reschedule should NOT fire.
        assert!(
            !called.load(Ordering::SeqCst),
            "reschedule should not be called for merely stale node"
        );
    }

    #[tokio::test]
    async fn monitor_ignores_master_node() {
        let store: Arc<dyn StateStore> = Arc::new(InMemoryStore::new());

        // Create a master node with a heartbeat 120 seconds ago — well past
        // both thresholds.
        let mut node = Node::new(
            "master-1".into(),
            "10.0.0.1:6444".into(),
            NodeRole::Master,
            NodeResources::zero(),
        );
        node.last_heartbeat = Utc::now() - chrono::Duration::seconds(120);
        store.insert_node(&node).await.unwrap();

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let reschedule: RescheduleFn = Arc::new(move |_node_id, _pods| {
            called_clone.store(true, Ordering::SeqCst);
        });

        check_nodes(&store, &reschedule).await.unwrap();

        // Master node should still be Ready — the monitor skips it.
        let updated = store.get_node(&node.id).await.unwrap().unwrap();
        assert_eq!(
            updated.status,
            NodeStatus::Ready,
            "master node status should not be changed by the heartbeat monitor"
        );
        assert!(
            !called.load(Ordering::SeqCst),
            "reschedule must never be called for a master node"
        );
    }

    #[test]
    fn collect_resources_returns_valid_data() {
        let res = collect_resources();
        assert!(
            res.cpu_cores > 0.0,
            "cpu_cores must be > 0, got {}",
            res.cpu_cores
        );
        assert!(
            res.memory_bytes > 0,
            "memory_bytes must be > 0, got {}",
            res.memory_bytes
        );
    }
}
