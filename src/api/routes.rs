use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::trace::TraceLayer;

use super::handlers;
use super::AppState;

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/api/v1/projects", get(handlers::list_projects))
        .route("/api/v1/projects", post(handlers::create_project))
        .route(
            "/api/v1/projects/{name}/suspend",
            post(handlers::suspend_project),
        )
        .route(
            "/api/v1/projects/{name}/resume",
            post(handlers::resume_project),
        )
        .route(
            "/api/v1/projects/{name}",
            delete(handlers::delete_project),
        )
        .route("/api/v1/deployments", get(handlers::list_deployments))
        .route("/api/v1/deploy", post(handlers::deploy))
        .route(
            "/api/v1/projects/{project}/deployments/{name}",
            delete(handlers::remove_deployment),
        )
        .route(
            "/api/v1/projects/{project}/deployments/{name}/stop",
            post(handlers::stop_deployment),
        )
        .route(
            "/api/v1/projects/{project}/deployments/{name}/scale",
            post(handlers::scale_deployment),
        )
        .route("/api/v1/pods", get(handlers::list_pods))
        .route(
            "/api/v1/projects/{project}/deployments/{name}/logs",
            get(handlers::logs),
        )
        .route(
            "/api/v1/projects/{project}/secrets",
            get(handlers::list_secrets),
        )
        .route(
            "/api/v1/projects/{project}/secrets/{name}",
            post(handlers::set_secret),
        )
        .route(
            "/api/v1/projects/{project}/secrets/{name}",
            delete(handlers::delete_secret),
        )
        // Cluster management routes
        .route("/api/v1/cluster/init", post(handlers::cluster_init))
        .route("/api/v1/cluster/token", get(handlers::cluster_token_show))
        .route(
            "/api/v1/cluster/token/rotate",
            post(handlers::cluster_token_rotate),
        )
        // Node management routes
        .route("/api/v1/nodes", get(handlers::list_nodes))
        .route(
            "/api/v1/nodes/{name}/drain",
            post(handlers::drain_node),
        )
        .route("/api/v1/nodes/{name}", delete(handlers::remove_node))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
