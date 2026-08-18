use std::time::Instant;

use sysinfo::{Disks, Networks, System};

use crate::models::SystemMetrics;

pub struct TelemetryCollector {
    system: System,
    networks: Networks,
    last_network_sample: Instant,
}

impl TelemetryCollector {
    pub fn new() -> Self {
        let mut system = System::new_all();

        system.refresh_cpu_all();
        system.refresh_memory();

        let mut networks = Networks::new_with_refreshed_list();

        // Take the first network snapshot.
        networks.refresh(true);

        Self {
            system,
            networks,
            last_network_sample: Instant::now(),
        }
    }

    pub fn collect(&mut self) -> SystemMetrics {
        self.system.refresh_cpu_all();
        self.system.refresh_memory();

        let cpu_usage = self.system.global_cpu_usage();

        let memory_used = self.system.used_memory();
        let memory_total = self.system.total_memory();

        // -------------------------
        // Disk telemetry
        // -------------------------

        let disks = Disks::new_with_refreshed_list();

        let disk_total: u64 = disks
            .list()
            .iter()
            .map(|disk| disk.total_space())
            .sum();

        let disk_available: u64 = disks
            .list()
            .iter()
            .map(|disk| disk.available_space())
            .sum();

        let disk_used = disk_total.saturating_sub(disk_available);

        // -------------------------
        // Network telemetry
        // -------------------------

        let previous_received: u64 = self
            .networks
            .iter()
            .map(|(_, network)| network.received())
            .sum();

        let previous_transmitted: u64 = self
            .networks
            .iter()
            .map(|(_, network)| network.transmitted())
            .sum();

        self.networks.refresh(true);

        let current_received: u64 = self
            .networks
            .iter()
            .map(|(_, network)| network.received())
            .sum();

        let current_transmitted: u64 = self
            .networks
            .iter()
            .map(|(_, network)| network.transmitted())
            .sum();

        let elapsed = self.last_network_sample.elapsed();

        let network_received = if elapsed.as_secs_f64() > 0.0 {
            ((current_received.saturating_sub(previous_received)) as f64
                / elapsed.as_secs_f64()) as u64
        } else {
            0
        };

        let network_transmitted = if elapsed.as_secs_f64() > 0.0 {
            ((current_transmitted.saturating_sub(previous_transmitted)) as f64
                / elapsed.as_secs_f64()) as u64
        } else {
            0
        };

        self.last_network_sample = Instant::now();

        SystemMetrics {
            timestamp: chrono::Utc::now().to_rfc3339(),

            cpu_usage,

            memory_used,
            memory_total,

            disk_used,
            disk_total,

            network_received,
            network_transmitted,
        }
    }
}