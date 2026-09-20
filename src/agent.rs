use std::time::Duration;
use tokio::time::sleep;

use crate::{
    config::AppConfig,
    models::{RegisterNodeRequest, TelemetryPayload},
    telemetry::TelemetryCollector,
};

pub struct AgentWorker {
    config: AppConfig,
    client: reqwest::Client,
}

impl AgentWorker {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    pub async fn run(self) {
        let server_url = match &self.config.cloudmesh_server_url {
            Some(url) => url,
            None => {
                eprintln!("AgentWorker: No CLOUDMESH_SERVER_URL configured. Exiting agent loop.");
                return;
            }
        };

        // Register Node
        let mut retry_count = 0;
        loop {
            let req = RegisterNodeRequest {
                node_id: self.config.node_id.clone(),
                hostname: hostname::get()
                    .map(|h| h.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| "unknown".to_string()),
                os: std::env::consts::OS.to_string(),
                architecture: std::env::consts::ARCH.to_string(),
                agent_version: env!("CARGO_PKG_VERSION").to_string(),
            };

            let res = self
                .client
                .post(format!("{}/api/nodes", server_url))
                .json(&req)
                .send()
                .await;

            match res {
                Ok(resp) if resp.status().is_success() => {
                    println!("AgentWorker: Successfully registered node {}", self.config.node_id);
                    break;
                }
                Ok(resp) => {
                    eprintln!(
                        "AgentWorker: Failed to register node. Server returned: {:?}",
                        resp.status()
                    );
                }
                Err(e) => {
                    eprintln!("AgentWorker: Failed to connect to server to register node: {}", e);
                }
            }

            retry_count += 1;
            sleep(Duration::from_secs(5)).await;
            if retry_count % 12 == 0 {
                eprintln!("AgentWorker: Still trying to register... ({} attempts)", retry_count);
            }
        }

        let mut collector = TelemetryCollector::new();
        // Discard first reading
        collector.collect();

        // Main Loop: Heartbeat and Telemetry
        let mut last_heartbeat = tokio::time::Instant::now();
        // Heartbeat interval could be slightly less than offline threshold.
        let heartbeat_interval = Duration::from_secs((self.config.offline_threshold_secs / 2).max(5) as u64);

        loop {
            sleep(Duration::from_secs(self.config.telemetry_interval_secs)).await;

            let metrics = collector.collect();
            let payload = TelemetryPayload {
                node_id: self.config.node_id.clone(),
                metrics,
            };

            // Send Telemetry
            let res = self
                .client
                .post(format!("{}/api/nodes/{}/telemetry", server_url, self.config.node_id))
                .json(&payload)
                .send()
                .await;

            match res {
                Ok(resp) if resp.status().is_success() => {
                    println!(
                        "AgentWorker [{}]: Telemetry sent | CPU: {:.2}% | Mem: {} MB",
                        self.config.node_id,
                        payload.metrics.cpu_usage,
                        payload.metrics.memory_used / 1024 / 1024
                    );
                }
                Ok(resp) => {
                    eprintln!("AgentWorker: Server rejected telemetry: {:?}", resp.status());
                }
                Err(e) => {
                    eprintln!("AgentWorker: Failed to send telemetry: {}", e);
                }
            }

            // Send Heartbeat if needed
            if last_heartbeat.elapsed() >= heartbeat_interval {
                let hb_res = self
                    .client
                    .post(format!("{}/api/nodes/{}/heartbeat", server_url, self.config.node_id))
                    .send()
                    .await;

                if let Err(e) = hb_res {
                    eprintln!("AgentWorker: Failed to send heartbeat: {}", e);
                } else {
                    last_heartbeat = tokio::time::Instant::now();
                }
            }
        }
    }
}
