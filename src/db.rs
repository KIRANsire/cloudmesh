use chrono::{DateTime, Utc};
use sqlx::{
    postgres::PgPoolOptions,
    PgPool,
    Row,
};

use crate::models::SystemMetrics;

/// Maximum number of records returned by a single historical telemetry query
/// to prevent unbounded memory usage and maintain database/API responsiveness.
/// At 5-second collection intervals, 10,000 records covers ~13.8 hours of continuous telemetry.
pub const MAX_HISTORICAL_METRICS_LIMIT: i64 = 10_000;

/// Creates a PostgreSQL connection pool for CloudMesh.
pub async fn create_pool(
    database_url: &str,
) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

/// Persists one telemetry snapshot into PostgreSQL.
pub async fn insert_metrics(
    pool: &PgPool,
    metrics: &SystemMetrics,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO telemetry (
            timestamp,
            cpu_usage,
            memory_used,
            memory_total,
            disk_used,
            disk_total,
            network_rx,
            network_tx
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(&metrics.timestamp)
    .bind(metrics.cpu_usage)
    .bind(metrics.memory_used as i64)
    .bind(metrics.memory_total as i64)
    .bind(metrics.disk_used as i64)
    .bind(metrics.disk_total as i64)
    .bind(metrics.network_received as i64)
    .bind(metrics.network_transmitted as i64)
    .execute(pool)
    .await?;

    Ok(())
}

/// Queries historical telemetry records from PostgreSQL starting from `start_time`
/// up to `limit` records, ordered chronologically (oldest to newest).
pub async fn get_metrics_history(
    pool: &PgPool,
    start_time: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<SystemMetrics>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            timestamp,
            cpu_usage,
            memory_used,
            memory_total,
            disk_used,
            disk_total,
            network_rx,
            network_tx
        FROM telemetry
        WHERE timestamp >= $1
        ORDER BY timestamp ASC
        LIMIT $2;
        "#,
    )
    .bind(start_time)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let metrics = rows
        .into_iter()
        .map(|row| SystemMetrics {
            timestamp: row.get("timestamp"),
            cpu_usage: row.get("cpu_usage"),
            memory_used: row.get::<i64, _>("memory_used") as u64,
            memory_total: row.get::<i64, _>("memory_total") as u64,
            disk_used: row.get::<i64, _>("disk_used") as u64,
            disk_total: row.get::<i64, _>("disk_total") as u64,
            network_received: row.get::<i64, _>("network_rx") as u64,
            network_transmitted: row.get::<i64, _>("network_tx") as u64,
        })
        .collect();

    Ok(metrics)
}