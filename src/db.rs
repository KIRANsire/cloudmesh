use sqlx::{
    postgres::PgPoolOptions,
    PgPool,
};

use crate::models::SystemMetrics;

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