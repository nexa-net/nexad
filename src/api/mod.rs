mod handlers;
mod routes;

use nexa_core::domain::orchestrator::OrchestratorHandle;

pub async fn serve(handle: OrchestratorHandle, addr: &str) -> anyhow::Result<()> {
    let app = routes::build(handle);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("nexad API listening on {addr}");

    axum::serve(listener, app).await?;
    Ok(())
}
