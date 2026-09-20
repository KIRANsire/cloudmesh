use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
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
    Unauthorized(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match self {
            ApiError::InvalidRange(msg) => {
                (StatusCode::BAD_REQUEST, "invalid_range", msg)
            }

            ApiError::DatabaseError(msg) => {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    msg,
                )
            }

            ApiError::NotFound(msg) => {
                (StatusCode::NOT_FOUND, "not_found", msg)
            }

            ApiError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, "bad_request", msg)
            }

            ApiError::Unauthorized(msg) => {
                (StatusCode::UNAUTHORIZED, "unauthorized", msg)
            }
        };

        let body = Json(ErrorResponse {
            error: error_code.to_string(),
            message,
        });

        (status, body).into_response()
    }
}

/*
|--------------------------------------------------------------------------
| Authentication
|--------------------------------------------------------------------------
|
| Phase 6 introduces API-key authentication.
|
| Agents send:
|
| X-API-Key: <their-secret-key>
|
| The server verifies the key before allowing the agent to:
|
| - register
| - send heartbeat
| - send telemetry
|
*/

fn extract_api_key(headers: &HeaderMap) -> Result<&str, ApiError> {
    headers
        .get("X-API-Key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ApiError::Unauthorized(
                "Missing X-API-Key header".to_string(),
            )
        })
}

async fn authenticate_api_key(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<String, ApiError> {
    let api_key = extract_api_key(headers)?;

    /*
     * Phase 6 stores only hashes in PostgreSQL.
     *
     * The database layer is responsible for verifying
     * the supplied key against the stored credential.
     */
    let node_id = db::authenticate_api_key(
        &state.db_pool,
        api_key,
    )
    .await
    .map_err(|err| {
        eprintln!("Authentication database error: {}", err);

        ApiError::DatabaseError(
            "Unable to authenticate request".to_string(),
        )
    })?;

    node_id.ok_or_else(|| {
        ApiError::Unauthorized(
            "Invalid API key".to_string(),
        )
    })
}

/*
|--------------------------------------------------------------------------
| Health
|--------------------------------------------------------------------------
*/

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "cloudmesh".to_string(),
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/*
|--------------------------------------------------------------------------
| Node Registration
|--------------------------------------------------------------------------
|
| POST /api/nodes
|
| Protected by X-API-Key.
|
| The API key must belong to the node being registered.
|--------------------------------------------------------------------------
*/

async fn register_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterNodeRequest>,
) -> Result<Json<Node>, ApiError> {
    let authenticated_node =
        authenticate_api_key(&state, &headers).await?;

    if authenticated_node != req.node_id {
        return Err(ApiError::Unauthorized(
            "API key does not belong to this node".to_string(),
        ));
    }

    let node = db::register_node(
        &state.db_pool,
        &req,
    )
    .await
    .map_err(|err| {
        eprintln!("Failed to register node: {}", err);

        ApiError::DatabaseError(
            "Unable to register node".to_string(),
        )
    })?;

    Ok(Json(node))
}

/*
|--------------------------------------------------------------------------
| List Nodes
|--------------------------------------------------------------------------
|
| GET /api/nodes
|
| This is a monitoring/read endpoint.
| It remains accessible without an agent API key because this endpoint
| will later be consumed by the CloudMesh dashboard.
|--------------------------------------------------------------------------
*/

async fn list_nodes(
    State(state): State<AppState>,
) -> Result<Json<Vec<Node>>, ApiError> {
    let mut nodes = db::list_nodes(
        &state.db_pool,
    )
    .await
    .map_err(|err| {
        eprintln!("Failed to list nodes: {}", err);

        ApiError::DatabaseError(
            "Unable to retrieve nodes".to_string(),
        )
    })?;

    let now = chrono::Utc::now();

    for node in &mut nodes {
        let elapsed = now
            .signed_duration_since(node.last_seen)
            .num_seconds();

        if elapsed > state.config.offline_threshold_secs {
            node.status = "offline".to_string();
        } else {
            node.status = "online".to_string();
        }
    }

    Ok(Json(nodes))
}

/*
|--------------------------------------------------------------------------
| Get Single Node
|--------------------------------------------------------------------------
|
| GET /api/nodes/{node_id}
|
| Public monitoring endpoint for now.
|--------------------------------------------------------------------------
*/

async fn get_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Result<Json<Node>, ApiError> {
    let mut node = db::get_node(
        &state.db_pool,
        &node_id,
    )
    .await
    .map_err(|err| {
        eprintln!(
            "Failed to get node {}: {}",
            node_id,
            err
        );

        ApiError::DatabaseError(
            "Unable to retrieve node".to_string(),
        )
    })?
    .ok_or_else(|| {
        ApiError::NotFound(
            "Node not found".to_string(),
        )
    })?;

    let now = chrono::Utc::now();

    let elapsed = now
        .signed_duration_since(node.last_seen)
        .num_seconds();

    if elapsed > state.config.offline_threshold_secs {
        node.status = "offline".to_string();
    } else {
        node.status = "online".to_string();
    }

    Ok(Json(node))
}

/*
|--------------------------------------------------------------------------
| Heartbeat
|--------------------------------------------------------------------------
|
| POST /api/nodes/{node_id}/heartbeat
|
| Protected by X-API-Key.
|--------------------------------------------------------------------------
*/

async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let authenticated_node =
        authenticate_api_key(&state, &headers).await?;

    if authenticated_node != node_id {
        return Err(ApiError::Unauthorized(
            "API key does not belong to this node".to_string(),
        ));
    }

    let updated = db::update_heartbeat(
        &state.db_pool,
        &node_id,
    )
    .await
    .map_err(|err| {
        eprintln!(
            "Failed to update heartbeat for {}: {}",
            node_id,
            err
        );

        ApiError::DatabaseError(
            "Unable to process heartbeat".to_string(),
        )
    })?;

    if !updated {
        return Err(ApiError::NotFound(
            "Node not found".to_string(),
        ));
    }

    Ok(StatusCode::OK)
}

/*
|--------------------------------------------------------------------------
| Telemetry Ingestion
|--------------------------------------------------------------------------
|
| POST /api/nodes/{node_id}/telemetry
|
| Protected by X-API-Key.
|--------------------------------------------------------------------------
*/

async fn ingest_telemetry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<String>,
    Json(payload): Json<TelemetryPayload>,
) -> Result<StatusCode, ApiError> {
    let authenticated_node =
        authenticate_api_key(&state, &headers).await?;

    /*
     * Three identities must agree:
     *
     * 1. API key owner
     * 2. URL node_id
     * 3. JSON payload node_id
     *
     * This prevents Node A from sending telemetry pretending
     * to be Node B.
     */

    if authenticated_node != node_id {
        return Err(ApiError::Unauthorized(
            "API key does not belong to this node".to_string(),
        ));
    }

    if node_id != payload.node_id {
        return Err(ApiError::BadRequest(
            "Node ID mismatch".to_string(),
        ));
    }

    db::insert_metrics(
        &state.db_pool,
        &node_id,
        &payload.metrics,
    )
    .await
    .map_err(|err| {
        eprintln!(
            "Failed to insert telemetry for {}: {}",
            node_id,
            err
        );

        if err
            .to_string()
            .contains("foreign key")
        {
            ApiError::NotFound(
                "Node not found".to_string(),
            )
        } else {
            ApiError::DatabaseError(
                "Unable to insert telemetry".to_string(),
            )
        }
    })?;

    Ok(StatusCode::CREATED)
}

/*
|--------------------------------------------------------------------------
| Historical Metrics
|--------------------------------------------------------------------------
|
| GET /api/nodes/{node_id}/metrics/history?range=5m
|
| This is a monitoring/read endpoint.
|--------------------------------------------------------------------------
*/

async fn get_node_metrics_history(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
    Query(params): Query<HistoryQueryParams>,
) -> Result<Json<HistoricalMetricsResponse>, ApiError> {
    let range_raw = params
        .range
        .as_deref()
        .unwrap_or("");

    let range = MetricsRange::parse(range_raw)
        .map_err(|_| {
            ApiError::InvalidRange(
                MetricsRange::INVALID_RANGE_MESSAGE
                    .to_string(),
            )
        })?;

    let end_time = chrono::Utc::now();

    let start_time =
        end_time - range.duration();

    /*
     * Make sure the requested node actually exists.
     */
    let node = db::get_node(
        &state.db_pool,
        &node_id,
    )
    .await
    .map_err(|err| {
        eprintln!(
            "Failed to retrieve node {}: {}",
            node_id,
            err
        );

        ApiError::DatabaseError(
            "Unable to retrieve node".to_string(),
        )
    })?;

    if node.is_none() {
        return Err(ApiError::NotFound(
            "Node not found".to_string(),
        ));
    }

    let metrics = db::get_metrics_history(
        &state.db_pool,
        &node_id,
        start_time,
        db::MAX_HISTORICAL_METRICS_LIMIT,
    )
    .await
    .map_err(|err| {
        eprintln!(
            "Database error while querying historical metrics: {}",
            err
        );

        ApiError::DatabaseError(
            "Unable to retrieve historical metrics"
                .to_string(),
        )
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

/*
|--------------------------------------------------------------------------
| Router
|--------------------------------------------------------------------------
*/

pub fn create_app(state: AppState) -> Router {
    Router::new()

        // ---------------------------------------------------------
        // Health
        // ---------------------------------------------------------
        .route(
            "/api/health",
            get(health_check),
        )

        // ---------------------------------------------------------
        // Nodes
        // ---------------------------------------------------------
        .route(
            "/api/nodes",
            post(register_node)
                .get(list_nodes),
        )

        .route(
            "/api/nodes/{node_id}",
            get(get_node),
        )

        // ---------------------------------------------------------
        // Agent communication
        // ---------------------------------------------------------
        .route(
            "/api/nodes/{node_id}/heartbeat",
            post(heartbeat),
        )

        .route(
            "/api/nodes/{node_id}/telemetry",
            post(ingest_telemetry),
        )

        // ---------------------------------------------------------
        // Historical telemetry
        // ---------------------------------------------------------
        .route(
            "/api/nodes/{node_id}/metrics/history",
            get(get_node_metrics_history),
        )

        .with_state(state)
}