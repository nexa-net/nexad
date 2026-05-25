mod handlers;
pub mod routes;

use std::sync::Arc;

use nexa_core::domain::orchestrator::OrchestratorHandle;
use nexa_core::ports::metrics::MetricsPort;
use nexa_core::ports::state::StateStore;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub handle: OrchestratorHandle,
    pub store: Arc<dyn StateStore>,
    pub metrics: Arc<dyn MetricsPort>,
    pub event_tx: broadcast::Sender<ClusterEvent>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ClusterEvent {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub kind: String,
    pub name: String,
    pub action: String,
    pub message: String,
}

pub async fn serve(
    handle: OrchestratorHandle,
    store: Arc<dyn StateStore>,
    metrics: Arc<dyn MetricsPort>,
    event_tx: broadcast::Sender<ClusterEvent>,
    addr: &str,
) -> anyhow::Result<()> {
    let state = AppState {
        handle,
        store,
        metrics,
        event_tx,
    };
    let app = routes::build(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("nexad API listening on {addr}");

    axum::serve(listener, app).await?;
    Ok(())
}
