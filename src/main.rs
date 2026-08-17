use axum::{routing::get, Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
    version: &'static str,
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "cloudmesh",
        status: "healthy",
        version: "0.1.0",
    })
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/api/health", get(health_check));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

