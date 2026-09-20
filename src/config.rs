use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: Option<String>,
    pub cloudmesh_server_url: Option<String>,
    pub node_id: String,
    pub telemetry_interval_secs: u64,
    pub offline_threshold_secs: i64,
}

impl AppConfig {
    pub fn load() -> Self {
        let database_url = env::var("DATABASE_URL").ok();
        
        // If DATABASE_URL is set, we default the server URL to localhost for the local agent.
        // Otherwise, the agent MUST have a CLOUDMESH_SERVER_URL to know where to send data.
        let cloudmesh_server_url = env::var("CLOUDMESH_SERVER_URL").ok().or_else(|| {
            if database_url.is_some() {
                Some("http://127.0.0.1:3000".to_string())
            } else {
                None
            }
        });

        let node_id = env::var("NODE_ID").unwrap_or_else(|_| {
            hostname::get()
                .map(|h| h.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "unknown-node".to_string())
        });

        let telemetry_interval_secs = env::var("TELEMETRY_INTERVAL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        let offline_threshold_secs = env::var("OFFLINE_THRESHOLD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);

        Self {
            database_url,
            cloudmesh_server_url,
            node_id,
            telemetry_interval_secs,
            offline_threshold_secs,
        }
    }

    pub fn is_server(&self) -> bool {
        self.database_url.is_some()
    }
}
