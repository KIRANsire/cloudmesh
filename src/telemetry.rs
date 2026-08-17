use std::{thread, time::Duration};
use sysinfo::System;

pub fn collect_cpu_usage() -> f32 {
    let mut system = System::new();

    system.refresh_cpu_all();

    thread::sleep(Duration::from_millis(500));

    system.refresh_cpu_all();

    system.global_cpu_usage()
}