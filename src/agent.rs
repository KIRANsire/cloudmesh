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
            Some(url) => url.trim_end_matches('/').to_string(),
            None => {
                eprintln!(
                    "AgentWorker: No CLOUDMESH_SERVER_URL configured. Exiting agent loop."
                );
                return;
            }
        };

        let api_key = match &self.config.api_key {
            Some(key) if !key.trim().is_empty() => key.clone(),
            _ => {
                eprintln!(
                    "AgentWorker: CLOUDMESH_API_KEY is not configured. \
                     Agent cannot authenticate with CloudMesh."
                );
                return;
            }
        };

        println!(
            "AgentWorker: Starting authenticated agent for node {}",
            self.config.node_id
        );

        // ---------------------------------------------------------
        // Node Registration
        // ---------------------------------------------------------

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

            let response = self
                .client
                .post(format!("{}/api/nodes", server_url))
                .header("X-API-Key", &api_key)
                .json(&req)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    println!(
                        "AgentWorker: Successfully registered node {}",
                        self.config.node_id
                    );

                    break;
                }

                Ok(resp) => {
                    let status = resp.status();

                    let body = resp
                        .text()
                        .await
                        .unwrap_or_else(|_| "<unable to read response>".to_string());

                    eprintln!(
                        "AgentWorker: Failed to register node. \
                         Server returned {}: {}",
                        status,
                        body
                    );
                }

                Err(error) => {
                    eprintln!(
                        "AgentWorker: Failed to connect to CloudMesh server: {}",
                        error
                    );
                }
            }

            retry_count += 1;

            sleep(Duration::from_secs(5)).await;

            if retry_count % 12 == 0 {
                eprintln!(
                    "AgentWorker: Still trying to register... ({} attempts)",
                    retry_count
                );
            }
        }

        // ---------------------------------------------------------
        // Telemetry Collector
        // ---------------------------------------------------------

        let mut collector = TelemetryCollector::new();

        // Discard the first reading because sysinfo requires
        // an initial sample before CPU usage becomes meaningful.
        collector.collect();

        // ---------------------------------------------------------
        // Heartbeat configuration
        // ---------------------------------------------------------

        let mut last_heartbeat = tokio::time::Instant::now();

        let heartbeat_interval = Duration::from_secs(
            (self.config.offline_threshold_secs / 2).max(5) as u64,
        );

        // ---------------------------------------------------------
        // Main Agent Loop
        // ---------------------------------------------------------

        loop {
            sleep(Duration::from_secs(
                self.config.telemetry_interval_secs,
            ))
            .await;

            // -----------------------------------------------------
            // Collect telemetry
            // -----------------------------------------------------

            let metrics = collector.collect();

            let payload = TelemetryPayload {
                node_id: self.config.node_id.clone(),
                metrics,
            };

            // -----------------------------------------------------
            // Send telemetry
            // -----------------------------------------------------

            let telemetry_url = format!(
                "{}/api/nodes/{}/telemetry",
                server_url,
                self.config.node_id
            );

            let response = self
                .client
                .post(&telemetry_url)
                .header("X-API-Key", &api_key)
                .json(&payload)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    println!(
                        "AgentWorker [{}]: Telemetry sent | CPU: {:.2}% | Mem: {} MB",
                        self.config.node_id,
                        payload.metrics.cpu_usage,
                        payload.metrics.memory_used / 1024 / 1024
                    );
                }

                Ok(resp) => {
                    let status = resp.status();

                    let body = resp
                        .text()
                        .await
                        .unwrap_or_else(|_| "<unable to read response>".to_string());

                    eprintln!(
                        "AgentWorker [{}]: Server rejected telemetry {}: {}",
                        self.config.node_id,
                        status,
                        body
                    );
                }

                Err(error) => {
                    eprintln!(
                        "AgentWorker [{}]: Failed to send telemetry: {}",
                        self.config.node_id,
                        error
                    );
                }
            }

            // -----------------------------------------------------
            // Heartbeat
            // -----------------------------------------------------

            if last_heartbeat.elapsed() >= heartbeat_interval {
                let heartbeat_url = format!(
                    "{}/api/nodes/{}/heartbeat",
                    server_url,
                    self.config.node_id
                );

                let response = self
                    .client
                    .post(&heartbeat_url)
                    .header("X-API-Key", &api_key)
                    .send()
                    .await;

                match response {
                    Ok(resp) if resp.status().is_success() => {
                        println!(
                            "AgentWorker [{}]: Heartbeat sent",
                            self.config.node_id
                        );

                        last_heartbeat = tokio::time::Instant::now();
                    }

                    Ok(resp) => {
                        let status = resp.status();

                        let body = resp
                            .text()
                            .await
                            .unwrap_or_else(|_| {
                                "<unable to read response>".to_string()
                            });

                        eprintln!(
                            "AgentWorker [{}]: Heartbeat rejected {}: {}",
                            self.config.node_id,
                            status,
                            body
                        );
                    }

                    Err(error) => {
                        eprintln!(
                            "AgentWorker [{}]: Failed to send heartbeat: {}",
                            self.config.node_id,
                            error
                        );
                    }
                }
            }
        }
    }
}