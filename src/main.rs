mod db;
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

use db::create_pool;
use models::SystemMetrics;
use telemetry::TelemetryCollector;

type SharedMetrics = Arc<RwLock<SystemMetrics>>;

#[derive(Clone)]
struct AppState {
    metrics: SharedMetrics,
    db_pool: sqlx::PgPool,
}

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
    State(state): State<AppState>,
) -> Json<SystemMetrics> {
    let metrics = state.metrics.read().await;

    Json(metrics.clone())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting CloudMesh...");

    // Load environment variables from .env if present.
    dotenvy::dotenv().ok();

    // Read PostgreSQL connection string.
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    // Create PostgreSQL connection pool.
    let db_pool = create_pool(&database_url).await?;

    println!("Connected to PostgreSQL");

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

    // PgPool is cheap to clone because it is internally shared.
    let db_pool_for_worker = db_pool.clone();

    // Background telemetry + persistence worker.
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(5)).await;

            let metrics = collector.collect();

            // Update current in-memory telemetry.
            {
                let mut shared = collector_metrics.write().await;
                *shared = metrics.clone();
            }

            // Persist telemetry to PostgreSQL.
            if let Err(error) = db::insert_metrics(
                &db_pool_for_worker,
                &metrics,
            )
            .await
            {
                eprintln!(
                    "Failed to persist telemetry: {}",
                    error
                );
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

    // Create Axum application.
    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/metrics", get(get_metrics))
        .with_state(AppState {
            metrics: shared_metrics,
            db_pool,
        });

    // Start HTTP server.
    let listener = tokio::net::TcpListener::bind(
        "127.0.0.1:3000"
    )
    .await?;

    println!(
        "CloudMesh running on http://127.0.0.1:3000"
    );

    axum::serve(listener, app).await?;

    Ok(())
}