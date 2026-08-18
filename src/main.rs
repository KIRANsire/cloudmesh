mod models;
mod telemetry;

use std::sync::Arc;

use axum::{
    extract::State,
    routing::get,
    Json,
    Router,
};

use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use models::SystemMetrics;
use telemetry::TelemetryCollector;

type SharedMetrics = Arc<RwLock<SystemMetrics>>;

#[derive(Debug, serde::Serialize)]
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

async fn get_metrics(
    State(metrics): State<SharedMetrics>,
) -> Json<SystemMetrics> {
    let metrics = metrics.read().await;

    Json(metrics.clone())
}

#[tokio::main]
async fn main() {
    println!("Starting CloudMesh...");

    // Create ONE telemetry collector.
    let mut collector = TelemetryCollector::new();

    // Take the first telemetry snapshot.
    let initial_metrics = collector.collect();

    println!("CPU Usage: {:.2}%", initial_metrics.cpu_usage);
    println!("Memory Used: {} bytes", initial_metrics.memory_used);
    println!("Memory Total: {} bytes", initial_metrics.memory_total);
    println!("Disk Used: {} bytes", initial_metrics.disk_used);
    println!("Disk Total: {} bytes", initial_metrics.disk_total);
    println!(
        "Network Received: {} bytes/sec",
        initial_metrics.network_received
    );
    println!(
        "Network Transmitted: {} bytes/sec",
        initial_metrics.network_transmitted
    );

    // Shared state containing the latest telemetry snapshot.
    let shared_metrics = Arc::new(
        RwLock::new(initial_metrics)
    );

    let collector_metrics = Arc::clone(&shared_metrics);

    // Move the SAME collector into the background task.
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(5)).await;

            let metrics = collector.collect();

            {
                let mut shared = collector_metrics.write().await;
                *shared = metrics.clone();
            }

            println!(
                "Telemetry updated | CPU: {:.2}% | Memory: {} MB | Network RX: {} B/s | Network TX: {} B/s",
                metrics.cpu_usage,
                metrics.memory_used / 1024 / 1024,
                metrics.network_received,
                metrics.network_transmitted
            );
        }
    });

    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/metrics", get(get_metrics))
        .with_state(shared_metrics);

    let listener = tokio::net::TcpListener::bind(
        "127.0.0.1:3000"
    )
    .await
    .unwrap();

    println!(
        "CloudMesh running on http://127.0.0.1:3000"
    );

    axum::serve(listener, app)
        .await
        .unwrap();
}