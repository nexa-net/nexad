use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::Json;
use futures::StreamExt;
use serde::Deserialize;

use nexa_core::config::parse_deployment;
use nexa_core::error::NexaError;

use super::AppState as SharedState;

type AppStateExtractor = State<SharedState>;

pub async fn health() -> &'static str {
    "ok"
}

pub async fn list_projects(State(state): AppStateExtractor) -> impl IntoResponse {
    Json(state.handle.list_projects().await)
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    name: String,
}

pub async fn create_project(
    State(state): AppStateExtractor,
    Json(req): Json<CreateProjectRequest>,
) -> impl IntoResponse {
    match state.handle.create_project(req.name).await {
        Ok(project) => (StatusCode::CREATED, Json(serde_json::json!(project))).into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct DeploymentFilter {
    project: Option<String>,
}

pub async fn list_deployments(
    State(state): AppStateExtractor,
    Query(filter): Query<DeploymentFilter>,
) -> impl IntoResponse {
    Json(state.handle.list_deployments(filter.project).await)
}

pub async fn deploy(State(state): AppStateExtractor, body: String) -> impl IntoResponse {
    let spec = match serde_json::from_str(&body) {
        Ok(spec) => spec,
        Err(_) => match parse_deployment(&body) {
            Ok(spec) => spec,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
        },
    };

    match state.handle.deploy(spec).await {
        Ok(deployment) => (StatusCode::CREATED, Json(serde_json::json!(deployment))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn stop_deployment(
    State(state): AppStateExtractor,
    Path((project, name)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.handle.stop(project, name).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn remove_deployment(
    State(state): AppStateExtractor,
    Path((project, name)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.handle.remove_deployment(project, name).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct ScaleRequest {
    replicas: u32,
}

pub async fn scale_deployment(
    State(state): AppStateExtractor,
    Path((project, name)): Path<(String, String)>,
    Json(req): Json<ScaleRequest>,
) -> impl IntoResponse {
    match state.handle.scale(project, name, req.replicas).await {
        Ok(deployment) => Json(serde_json::json!(deployment)).into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn list_pods(
    State(state): AppStateExtractor,
    Query(filter): Query<DeploymentFilter>,
) -> impl IntoResponse {
    Json(state.handle.list_pods(filter.project).await)
}

#[derive(Deserialize)]
pub struct LogsQuery {
    tail: Option<u64>,
}

pub async fn logs(
    State(state): AppStateExtractor,
    Path((project, name)): Path<(String, String)>,
    Query(query): Query<LogsQuery>,
) -> impl IntoResponse {
    match state.handle.pod_logs(project, name, query.tail).await {
        Ok(stream) => {
            let event_stream =
                stream.map(|result| -> std::result::Result<Event, std::convert::Infallible> {
                    match result {
                        Ok(line) => Ok(Event::default().data(line)),
                        Err(e) => Ok(Event::default().data(format!("error: {e}"))),
                    }
                });
            Sse::new(event_stream).into_response()
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ---- Project lifecycle handlers ----

pub async fn suspend_project(
    State(state): AppStateExtractor,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.handle.suspend_project(name).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn resume_project(
    State(state): AppStateExtractor,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.handle.resume_project(name).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn delete_project(
    State(state): AppStateExtractor,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.handle.delete_project(name).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => {
            let status = match &e {
                NexaError::ProjectNotEmpty(_) => StatusCode::CONFLICT,
                _ => StatusCode::NOT_FOUND,
            };
            (status, Json(serde_json::json!({ "error": e.to_string() }))).into_response()
        }
    }
}

// ---- Secrets handlers ----

pub async fn list_secrets(
    State(state): AppStateExtractor,
    Path(project): Path<String>,
) -> impl IntoResponse {
    match state.handle.list_secrets(project).await {
        Ok(names) => Json(serde_json::json!(names)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct SetSecretRequest {
    pub value: String,
}

pub async fn set_secret(
    State(state): AppStateExtractor,
    Path((project, name)): Path<(String, String)>,
    Json(req): Json<SetSecretRequest>,
) -> impl IntoResponse {
    match state.handle.set_secret(project, name, req.value.into_bytes()).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn delete_secret(
    State(state): AppStateExtractor,
    Path((project, name)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.handle.delete_secret(project, name).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ---- Cluster management handlers ----

pub async fn cluster_init(State(state): AppStateExtractor) -> impl IntoResponse {
    let token = nexad::cluster::token::generate_token();
    let hash = nexad::cluster::token::hash_token(&token);
    match state.store.set_cluster_config("join_token_hash", &hash).await {
        Ok(()) => {
            let _ = state.store.set_cluster_config("join_token", &token).await;
            Json(serde_json::json!({ "token": token })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn cluster_token_show(State(state): AppStateExtractor) -> impl IntoResponse {
    match state.store.get_cluster_config("join_token").await {
        Ok(Some(token)) => Json(serde_json::json!({ "token": token })).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "cluster not initialized" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn cluster_token_rotate(State(state): AppStateExtractor) -> impl IntoResponse {
    let token = nexad::cluster::token::generate_token();
    let hash = nexad::cluster::token::hash_token(&token);
    match state.store.set_cluster_config("join_token_hash", &hash).await {
        Ok(()) => {
            let _ = state.store.set_cluster_config("join_token", &token).await;
            Json(serde_json::json!({ "token": token })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ---- Node management handlers ----

pub async fn list_nodes(State(state): AppStateExtractor) -> impl IntoResponse {
    match state.store.list_nodes().await {
        Ok(nodes) => Json(nodes).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn drain_node(
    State(state): AppStateExtractor,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.store.get_node_by_name(&name).await {
        Ok(Some(mut node)) => {
            node.status = nexa_core::domain::models::NodeStatus::Draining;
            match state.store.update_node(&node).await {
                Ok(()) => StatusCode::OK.into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response(),
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("node '{}' not found", name) })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn remove_node(
    State(state): AppStateExtractor,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.store.get_node_by_name(&name).await {
        Ok(Some(node)) => {
            if node.status != nexa_core::domain::models::NodeStatus::Draining
                && node.role != nexa_core::domain::models::NodeRole::Master
            {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error": format!(
                            "node '{}' must be drained before removal (status: {})",
                            name, node.status
                        )
                    })),
                )
                    .into_response();
            }
            match state.store.delete_node(&node.id).await {
                Ok(()) => StatusCode::OK.into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response(),
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("node '{}' not found", name) })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
