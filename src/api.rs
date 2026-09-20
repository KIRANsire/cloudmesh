use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use crate::{
    config::AppConfig,
    db,
    models::{
        ErrorResponse, HistoricalMetricsResponse, HistoryQueryParams, MetricsRange, Node,
        RegisterNodeRequest, TelemetryPayload,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
    pub config: Arc<AppConfig>,
}

#[derive(Debug, serde::Serialize)]
pub struct HealthResponse {
    pub service: String,
    pub status: String,
    pub version: String,
}

#[derive(Debug)]
pub enum ApiError {
    InvalidRange(String),
    DatabaseError(String),
    NotFound(String),
    BadRequest(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match self {
            ApiError::InvalidRange(msg) => (StatusCode::BAD_REQUEST, "invalid_range", msg),
            ApiError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "database_error", msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
        };

        let body = Json(ErrorResponse {
            error: error_code.to_string(),
            message,
        });

        (status, body).into_response()
    }
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "cloudmesh".to_string(),
        status: "healthy".to_string(),
        version: "0.1.0".to_string(),
    })
}

async fn register_node(
    State(state): State<AppState>,
    Json(req): Json<RegisterNodeRequest>,
) -> Result<Json<Node>, ApiError> {
    let node = db::register_node(&state.db_pool, &req).await.map_err(|err| {
        eprintln!("Failed to register node: {}", err);
        ApiError::DatabaseError("Unable to register node".to_string())
    })?;
    Ok(Json(node))
}

async fn list_nodes(State(state): State<AppState>) -> Result<Json<Vec<Node>>, ApiError> {
    let mut nodes = db::list_nodes(&state.db_pool).await.map_err(|err| {
        eprintln!("Failed to list nodes: {}", err);
        ApiError::DatabaseError("Unable to retrieve nodes".to_string())
    })?;

    let now = chrono::Utc::now();
    for node in &mut nodes {
        let elapsed = now.signed_duration_since(node.last_seen).num_seconds();
        if elapsed > state.config.offline_threshold_secs {
            node.status = "offline".to_string();
        } else {
            node.status = "online".to_string();
        }
    }

    Ok(Json(nodes))
}

async fn get_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Result<Json<Node>, ApiError> {
    let mut node = db::get_node(&state.db_pool, &node_id)
        .await
        .map_err(|err| {
            eprintln!("Failed to get node {}: {}", node_id, err);
            ApiError::DatabaseError("Unable to retrieve node".to_string())
        })?
        .ok_or_else(|| ApiError::NotFound("Node not found".to_string()))?;

    let now = chrono::Utc::now();
    let elapsed = now.signed_duration_since(node.last_seen).num_seconds();
    if elapsed > state.config.offline_threshold_secs {
        node.status = "offline".to_string();
    } else {
        node.status = "online".to_string();
    }

    Ok(Json(node))
}

async fn heartbeat(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let updated = db::update_heartbeat(&state.db_pool, &node_id)
        .await
        .map_err(|err| {
            eprintln!("Failed to update heartbeat for {}: {}", node_id, err);
            ApiError::DatabaseError("Unable to process heartbeat".to_string())
        })?;

    if !updated {
        return Err(ApiError::NotFound("Node not found".to_string()));
    }

    Ok(StatusCode::OK)
}

async fn ingest_telemetry(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
    Json(payload): Json<TelemetryPayload>,
) -> Result<StatusCode, ApiError> {
    if node_id != payload.node_id {
        return Err(ApiError::BadRequest("Node ID mismatch".to_string()));
    }
    
    // First verify node exists by getting it or letting the FK constraint handle it
    db::insert_metrics(&state.db_pool, &node_id, &payload.metrics)
        .await
        .map_err(|err| {
            eprintln!("Failed to insert telemetry for {}: {}", node_id, err);
            // In a real app we'd match the PgError code for FK violation, but for now:
            if err.to_string().contains("fk_node") || err.to_string().contains("foreign key") {
                ApiError::NotFound("Node not found".to_string())
            } else {
                ApiError::DatabaseError("Unable to insert telemetry".to_string())
            }
        })?;

    Ok(StatusCode::CREATED)
}

async fn get_node_metrics_history(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
    Query(params): Query<HistoryQueryParams>,
) -> Result<Json<HistoricalMetricsResponse>, ApiError> {
    let range_raw = params.range.as_deref().unwrap_or("");
    let range = MetricsRange::parse(range_raw).map_err(|_| {
        ApiError::InvalidRange(MetricsRange::INVALID_RANGE_MESSAGE.to_string())
    })?;

    let end_time = chrono::Utc::now();
    let start_time = end_time - range.duration();

    // Verify node exists
    let node = db::get_node(&state.db_pool, &node_id).await.map_err(|_| {
        ApiError::DatabaseError("Unable to retrieve node".to_string())
    })?;
    if node.is_none() {
        return Err(ApiError::NotFound("Node not found".to_string()));
    }

    let metrics = db::get_metrics_history(
        &state.db_pool,
        &node_id,
        start_time,
        db::MAX_HISTORICAL_METRICS_LIMIT,
    )
    .await
    .map_err(|err| {
        eprintln!("Database error while querying historical metrics: {err}");
        ApiError::DatabaseError("Unable to retrieve historical metrics".to_string())
    })?;

    let count = metrics.len();

    Ok(Json(HistoricalMetricsResponse {
        range: range.as_str().to_string(),
        start_time,
        end_time,
        count,
        metrics,
    }))
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/nodes", post(register_node).get(list_nodes))
        .route("/api/nodes/{node_id}", get(get_node))
        .route("/api/nodes/{node_id}/heartbeat", post(heartbeat))
        .route("/api/nodes/{node_id}/telemetry", post(ingest_telemetry))
        .route(
            "/api/nodes/{node_id}/metrics/history",
            get(get_node_metrics_history),
        )
        .with_state(state)
}
