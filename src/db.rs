use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::{
    postgres::PgPoolOptions,
    PgPool,
    Row,
};

use crate::models::{
    Node,
    RegisterNodeRequest,
    SystemMetrics,
};

/// Maximum number of records returned by a single historical telemetry query.
///
/// At 5-second collection intervals:
/// 10,000 records ≈ 13.8 hours of telemetry.
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

/// Hashes an API key using SHA-256.
///
/// CloudMesh never needs to store the raw API key in PostgreSQL.
/// Only the SHA-256 hash is stored.
fn hash_api_key(api_key: &str) -> String {
    let mut hasher = Sha256::new();

    hasher.update(api_key.as_bytes());

    let result = hasher.finalize();

    hex::encode(result)
}

/// Authenticates an API key and returns the node that owns it.
///
/// Returns:
/// - Ok(Some(node_id)) if the key is valid
/// - Ok(None) if the key is invalid/revoked/not found
/// - Err(...) if PostgreSQL fails
pub async fn authenticate_api_key(
    pool: &PgPool,
    api_key: &str,
) -> Result<Option<String>, sqlx::Error> {
    if api_key.trim().is_empty() {
        return Ok(None);
    }

    let key_hash = hash_api_key(api_key);

    let row = sqlx::query(
        r#"
        SELECT node_id
        FROM node_api_keys
        WHERE key_hash = $1
          AND revoked_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(&key_hash)
    .fetch_optional(pool)
    .await?;

    let node_id = match row {
        Some(row) => {
            let node_id: String = row.get("node_id");

            // Record successful API-key usage.
            sqlx::query(
                r#"
                UPDATE node_api_keys
                SET last_used_at = NOW()
                WHERE key_hash = $1
                  AND revoked_at IS NULL
                "#,
            )
            .bind(&key_hash)
            .execute(pool)
            .await?;

            Some(node_id)
        }

        None => None,
    };

    Ok(node_id)
}

/// Registers or updates a node in the database.
pub async fn register_node(
    pool: &PgPool,
    req: &RegisterNodeRequest,
) -> Result<Node, sqlx::Error> {
    let row = sqlx::query(
        r#"
        INSERT INTO nodes (
            node_id,
            hostname,
            os,
            architecture,
            agent_version,
            status,
            last_seen
        )
        VALUES (
            $1,
            $2,
            $3,
            $4,
            $5,
            'online',
            NOW()
        )

        ON CONFLICT (node_id) DO UPDATE SET
            hostname = EXCLUDED.hostname,
            os = EXCLUDED.os,
            architecture = EXCLUDED.architecture,
            agent_version = EXCLUDED.agent_version,
            status = 'online',
            last_seen = NOW(),
            updated_at = NOW()

        RETURNING
            node_id,
            hostname,
            os,
            architecture,
            agent_version,
            status,
            last_seen,
            created_at,
            updated_at
        "#,
    )
    .bind(&req.node_id)
    .bind(&req.hostname)
    .bind(&req.os)
    .bind(&req.architecture)
    .bind(&req.agent_version)
    .fetch_one(pool)
    .await?;

    Ok(Node {
        node_id: row.get("node_id"),
        hostname: row.get("hostname"),
        os: row.get("os"),
        architecture: row.get("architecture"),
        agent_version: row.get("agent_version"),
        status: row.get("status"),
        last_seen: row.get("last_seen"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

/// Lists all known nodes.
pub async fn list_nodes(
    pool: &PgPool,
) -> Result<Vec<Node>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            node_id,
            hostname,
            os,
            architecture,
            agent_version,
            status,
            last_seen,
            created_at,
            updated_at
        FROM nodes
        ORDER BY node_id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let nodes = rows
        .into_iter()
        .map(|row| Node {
            node_id: row.get("node_id"),
            hostname: row.get("hostname"),
            os: row.get("os"),
            architecture: row.get("architecture"),
            agent_version: row.get("agent_version"),
            status: row.get("status"),
            last_seen: row.get("last_seen"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect();

    Ok(nodes)
}

/// Gets a specific node by ID.
pub async fn get_node(
    pool: &PgPool,
    node_id: &str,
) -> Result<Option<Node>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            node_id,
            hostname,
            os,
            architecture,
            agent_version,
            status,
            last_seen,
            created_at,
            updated_at
        FROM nodes
        WHERE node_id = $1
        "#,
    )
    .bind(node_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| Node {
        node_id: row.get("node_id"),
        hostname: row.get("hostname"),
        os: row.get("os"),
        architecture: row.get("architecture"),
        agent_version: row.get("agent_version"),
        status: row.get("status"),
        last_seen: row.get("last_seen"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }))
}

/// Updates the last_seen timestamp for a node.
///
/// This is called by the heartbeat endpoint.
pub async fn update_heartbeat(
    pool: &PgPool,
    node_id: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE nodes
        SET
            last_seen = NOW(),
            status = 'online',
            updated_at = NOW()
        WHERE node_id = $1
        "#,
    )
    .bind(node_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Persists one telemetry snapshot into PostgreSQL.
pub async fn insert_metrics(
    pool: &PgPool,
    node_id: &str,
    metrics: &SystemMetrics,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO telemetry (
            node_id,
            timestamp,
            cpu_usage,
            memory_used,
            memory_total,
            disk_used,
            disk_total,
            network_rx,
            network_tx
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(node_id)
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

/// Queries historical telemetry records for a node.
pub async fn get_metrics_history(
    pool: &PgPool,
    node_id: &str,
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
        WHERE node_id = $1
          AND timestamp >= $2
        ORDER BY timestamp ASC
        LIMIT $3
        "#,
    )
    .bind(node_id)
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