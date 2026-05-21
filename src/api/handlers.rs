use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::Json;
use futures::StreamExt;
use serde::Deserialize;

use nexa_core::config::parse_deployment;
use nexa_core::domain::orchestrator::OrchestratorHandle;
use nexa_core::error::NexaError;

type AppState = State<OrchestratorHandle>;

pub async fn health() -> &'static str {
    "ok"
}

pub async fn list_projects(State(handle): AppState) -> impl IntoResponse {
    Json(handle.list_projects().await)
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    name: String,
}

pub async fn create_project(
    State(handle): AppState,
    Json(req): Json<CreateProjectRequest>,
) -> impl IntoResponse {
    match handle.create_project(req.name).await {
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
    State(handle): AppState,
    Query(filter): Query<DeploymentFilter>,
) -> impl IntoResponse {
    Json(handle.list_deployments(filter.project).await)
}

pub async fn deploy(State(handle): AppState, body: String) -> impl IntoResponse {
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

    match handle.deploy(spec).await {
        Ok(deployment) => (StatusCode::CREATED, Json(serde_json::json!(deployment))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn stop_deployment(
    State(handle): AppState,
    Path((project, name)): Path<(String, String)>,
) -> impl IntoResponse {
    match handle.stop(project, name).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn remove_deployment(
    State(handle): AppState,
    Path((project, name)): Path<(String, String)>,
) -> impl IntoResponse {
    match handle.remove_deployment(project, name).await {
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
    State(handle): AppState,
    Path((project, name)): Path<(String, String)>,
    Json(req): Json<ScaleRequest>,
) -> impl IntoResponse {
    match handle.scale(project, name, req.replicas).await {
        Ok(deployment) => Json(serde_json::json!(deployment)).into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn list_pods(
    State(handle): AppState,
    Query(filter): Query<DeploymentFilter>,
) -> impl IntoResponse {
    Json(handle.list_pods(filter.project).await)
}

#[derive(Deserialize)]
pub struct LogsQuery {
    tail: Option<u64>,
}

pub async fn logs(
    State(handle): AppState,
    Path((project, name)): Path<(String, String)>,
    Query(query): Query<LogsQuery>,
) -> impl IntoResponse {
    match handle.pod_logs(project, name, query.tail).await {
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
    State(handle): AppState,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match handle.suspend_project(name).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn resume_project(
    State(handle): AppState,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match handle.resume_project(name).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn delete_project(
    State(handle): AppState,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match handle.delete_project(name).await {
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
    State(handle): AppState,
    Path(project): Path<String>,
) -> impl IntoResponse {
    match handle.list_secrets(project).await {
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
    State(handle): AppState,
    Path((project, name)): Path<(String, String)>,
    Json(req): Json<SetSecretRequest>,
) -> impl IntoResponse {
    match handle.set_secret(project, name, req.value.into_bytes()).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn delete_secret(
    State(handle): AppState,
    Path((project, name)): Path<(String, String)>,
) -> impl IntoResponse {
    match handle.delete_secret(project, name).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
