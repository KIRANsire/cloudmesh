use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct SystemMetrics {
    pub timestamp: DateTime<Utc>,

    pub cpu_usage: f32,

    pub memory_used: u64,
    pub memory_total: u64,

    pub disk_used: u64,
    pub disk_total: u64,

    pub network_received: u64,
    pub network_transmitted: u64,
}