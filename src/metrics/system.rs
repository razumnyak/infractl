use serde::Serialize;
use sysinfo::{Disks, Networks, System};

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub cpu_count: usize,
    pub memory_total: u64,
    pub memory_used: u64,
    pub memory_free: u64,
    pub memory_usage_percent: f32,
    pub swap_total: u64,
    pub swap_used: u64,
    pub load_avg: LoadAverage,
    pub uptime: u64,
    pub hostname: String,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub disks: Vec<DiskInfo>,
    pub networks: Vec<NetworkInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoadAverage {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub usage_percent: f32,
    pub fs_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkInfo {
    pub name: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
    pub received_packets: u64,
    pub transmitted_packets: u64,
    pub errors_in: u64,
    pub errors_out: u64,
}

pub struct SystemCollector;

impl SystemCollector {
    pub fn collect() -> SystemMetrics {
        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu_usage = sys.global_cpu_usage();
        let cpu_count = sys.cpus().len();

        let memory_total = sys.total_memory();
        let memory_used = sys.used_memory();
        let memory_free = sys.free_memory();
        let memory_usage_percent = if memory_total > 0 {
            (memory_used as f32 / memory_total as f32) * 100.0
        } else {
            0.0
        };

        let swap_total = sys.total_swap();
        let swap_used = sys.used_swap();

        let load_avg = System::load_average();

        let disks = Self::collect_disks();
        let networks = Self::collect_networks();

        SystemMetrics {
            cpu_usage,
            cpu_count,
            memory_total,
            memory_used,
            memory_free,
            memory_usage_percent,
            swap_total,
            swap_used,
            load_avg: LoadAverage {
                one: load_avg.one,
                five: load_avg.five,
                fifteen: load_avg.fifteen,
            },
            uptime: System::uptime(),
            hostname: System::host_name().unwrap_or_default(),
            os_name: System::name(),
            os_version: System::os_version(),
            kernel_version: System::kernel_version(),
            disks,
            networks,
        }
    }

    fn collect_disks() -> Vec<DiskInfo> {
        let disks = Disks::new_with_refreshed_list();

        disks
            .iter()
            .map(|disk| {
                let total = disk.total_space();
                let free = disk.available_space();
                let used = total.saturating_sub(free);
                let usage_percent = if total > 0 {
                    (used as f32 / total as f32) * 100.0
                } else {
                    0.0
                };

                DiskInfo {
                    name: disk.name().to_string_lossy().to_string(),
                    mount_point: disk.mount_point().to_string_lossy().to_string(),
                    total,
                    used,
                    free,
                    usage_percent,
                    fs_type: disk.file_system().to_string_lossy().to_string(),
                }
            })
            .collect()
    }

    fn collect_networks() -> Vec<NetworkInfo> {
        let networks = Networks::new_with_refreshed_list();

        networks
            .iter()
            .map(|(name, data)| NetworkInfo {
                name: name.clone(),
                received_bytes: data.total_received(),
                transmitted_bytes: data.total_transmitted(),
                received_packets: data.total_packets_received(),
                transmitted_packets: data.total_packets_transmitted(),
                errors_in: data.total_errors_on_received(),
                errors_out: data.total_errors_on_transmitted(),
            })
            .collect()
    }
}
