mod handlers;
pub mod routes;

use std::sync::Arc;

use nexa_core::domain::orchestrator::OrchestratorHandle;
use nexa_core::ports::metrics::MetricsPort;
use nexa_core::ports::state::StateStore;

#[derive(Clone)]
pub struct AppState {
    pub handle: OrchestratorHandle,
    pub store: Arc<dyn StateStore>,
    pub metrics: Arc<dyn MetricsPort>,
}

pub async fn serve(
    handle: OrchestratorHandle,
    store: Arc<dyn StateStore>,
    metrics: Arc<dyn MetricsPort>,
    addr: &str,
) -> anyhow::Result<()> {
    let state = AppState {
        handle,
        store,
        metrics,
    };
    let app = routes::build(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("nexad API listening on {addr}");

    axum::serve(listener, app).await?;
    Ok(())
}
