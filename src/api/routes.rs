use axum::Router;
use axum::middleware;
use axum::routing::{delete, get, post};
use tower_http::trace::TraceLayer;

use super::AppState;
use super::handlers;

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/metrics", get(handlers::metrics_endpoint))
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
        .route("/api/v1/projects/{name}", delete(handlers::delete_project))
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
        .route("/api/v1/nodes/stats", get(handlers::node_stats))
        .route("/api/v1/nodes/{name}/drain", post(handlers::drain_node))
        .route("/api/v1/nodes/{name}", delete(handlers::remove_node))
        .route(
            "/api/v1/cluster/scheduler",
            get(handlers::get_scheduler_config),
        )
        .route(
            "/api/v1/cluster/scheduler",
            post(handlers::set_scheduler_config),
        )
        // Routes
        .route("/api/v1/routes", get(handlers::list_routes))
        .route("/api/v1/routes", post(handlers::add_route))
        .route("/api/v1/routes/{domain}", delete(handlers::remove_route))
        // Certificates
        .route("/api/v1/certs/import", post(handlers::import_cert))
        // Proxy config
        .route(
            "/api/v1/cluster/config/proxy",
            get(handlers::get_proxy_config),
        )
        .route(
            "/api/v1/cluster/config/proxy",
            post(handlers::set_proxy_config),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            handlers::metrics_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
