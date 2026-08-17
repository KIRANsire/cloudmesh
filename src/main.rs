mod telemetry;

use axum::{
    routing::get,
    Json,
    Router,
};
use serde::Serialize;

#[derive(Serialize)]
struct HealthResponse {
    service: String,
    status: String,
    version: String,
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "cloudmesh".to_string(),
        status: "healthy".to_string(),
        version: "0.1.0".to_string(),
    })
}

#[tokio::main]
async fn main() {
    let cpu_usage = telemetry::collect_cpu_usage();

    println!("CPU Usage: {:.2}%", cpu_usage);

    let app = Router::new()
        .route("/api/health", get(health_check));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app)
        .await
        .unwrap();
}