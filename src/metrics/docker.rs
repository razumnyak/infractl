use bollard::container::{ListContainersOptions, Stats, StatsOptions};
use bollard::Docker;
use futures::StreamExt;
use serde::Serialize;
use std::collections::HashMap;
use tracing::warn;

#[derive(Debug, Clone, Serialize)]
pub struct DockerMetrics {
    pub available: bool,
    pub version: Option<String>,
    pub containers_running: u32,
    pub containers_paused: u32,
    pub containers_stopped: u32,
    pub containers_total: u32,
    pub images_count: u32,
    pub containers: Vec<ContainerInfo>,
    pub compose_projects: Vec<ComposeProject>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<ContainerStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compose_project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compose_service: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContainerStats {
    pub cpu_percent: f64,
    pub memory_usage: u64,
    pub memory_limit: u64,
    pub memory_percent: f64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
    pub pids: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComposeProject {
    pub name: String,
    pub working_dir: Option<String>,
    pub services: Vec<String>,
    pub containers_running: u32,
    pub containers_total: u32,
}

pub struct DockerCollector {
    client: Docker,
}

impl DockerCollector {
    pub async fn new() -> Result<Self, bollard::errors::Error> {
        // Try to connect to Docker socket
        let client = Docker::connect_with_socket_defaults()?;

        // Verify connection
        client.ping().await?;

        Ok(Self { client })
    }

    #[allow(dead_code)]
    pub async fn new_with_socket(socket_path: &str) -> Result<Self, bollard::errors::Error> {
        let client = Docker::connect_with_socket(socket_path, 120, bollard::API_DEFAULT_VERSION)?;
        client.ping().await?;
        Ok(Self { client })
    }

    pub async fn collect(&self) -> DockerMetrics {
        let version = self.get_version().await;
        let info = self.get_info().await;
        let containers = self.list_containers().await;
        let compose_projects = Self::detect_compose_projects(&containers);

        DockerMetrics {
            available: true,
            version,
            containers_running: info.0,
            containers_paused: info.1,
            containers_stopped: info.2,
            containers_total: containers.len() as u32,
            images_count: info.3,
            containers,
            compose_projects,
        }
    }

    async fn get_version(&self) -> Option<String> {
        self.client.version().await.ok().and_then(|v| v.version)
    }

    async fn get_info(&self) -> (u32, u32, u32, u32) {
        match self.client.info().await {
            Ok(info) => (
                info.containers_running.unwrap_or(0) as u32,
                info.containers_paused.unwrap_or(0) as u32,
                info.containers_stopped.unwrap_or(0) as u32,
                info.images.unwrap_or(0) as u32,
            ),
            Err(e) => {
                warn!("Failed to get Docker info: {}", e);
                (0, 0, 0, 0)
            }
        }
    }

    async fn list_containers(&self) -> Vec<ContainerInfo> {
        let options = ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        };

        let containers = match self.client.list_containers(Some(options)).await {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to list containers: {}", e);
                return Vec::new();
            }
        };

        let mut result = Vec::new();

        for container in containers {
            let id = container.id.clone().unwrap_or_default();
            let short_id = id.chars().take(12).collect::<String>();

            let name = container
                .names
                .as_ref()
                .and_then(|n| n.first())
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_else(|| short_id.clone());

            let labels = container.labels.clone().unwrap_or_default();
            let compose_project = labels.get("com.docker.compose.project").cloned();
            let compose_service = labels.get("com.docker.compose.service").cloned();

            let state = container.state.clone().unwrap_or_default();

            // Only get stats for running containers
            let stats = if state == "running" {
                self.get_container_stats(&id).await
            } else {
                None
            };

            result.push(ContainerInfo {
                id: short_id,
                name,
                image: container.image.unwrap_or_default(),
                state,
                status: container.status.unwrap_or_default(),
                created: container.created.unwrap_or(0),
                stats,
                compose_project,
                compose_service,
            });
        }

        result
    }

    async fn get_container_stats(&self, container_id: &str) -> Option<ContainerStats> {
        let options = StatsOptions {
            stream: false,
            one_shot: true,
        };

        let mut stream = self.client.stats(container_id, Some(options));

        if let Some(Ok(stats)) = stream.next().await {
            Some(Self::parse_stats(&stats))
        } else {
            None
        }
    }

    fn parse_stats(stats: &Stats) -> ContainerStats {
        // Calculate CPU percentage
        let cpu_percent = Self::calculate_cpu_percent(stats);

        // Memory stats
        let memory_usage = stats.memory_stats.usage.unwrap_or(0);
        let memory_limit = stats.memory_stats.limit.unwrap_or(1);
        let memory_percent = if memory_limit > 0 {
            (memory_usage as f64 / memory_limit as f64) * 100.0
        } else {
            0.0
        };

        // Network stats
        let (network_rx, network_tx) = stats
            .networks
            .as_ref()
            .map(|networks| {
                networks.values().fold((0u64, 0u64), |(rx, tx), net| {
                    (rx + net.rx_bytes, tx + net.tx_bytes)
                })
            })
            .unwrap_or((0, 0));

        // Block I/O stats
        let (block_read, block_write) = stats
            .blkio_stats
            .io_service_bytes_recursive
            .as_ref()
            .map(|io_stats| {
                io_stats
                    .iter()
                    .fold((0u64, 0u64), |(read, write), stat| match stat.op.as_str() {
                        "read" | "Read" => (read + stat.value, write),
                        "write" | "Write" => (read, write + stat.value),
                        _ => (read, write),
                    })
            })
            .unwrap_or((0, 0));

        // PIDs
        let pids = stats.pids_stats.current.unwrap_or(0);

        ContainerStats {
            cpu_percent,
            memory_usage,
            memory_limit,
            memory_percent,
            network_rx_bytes: network_rx,
            network_tx_bytes: network_tx,
            block_read_bytes: block_read,
            block_write_bytes: block_write,
            pids,
        }
    }

    fn calculate_cpu_percent(stats: &Stats) -> f64 {
        let cpu_delta = stats.cpu_stats.cpu_usage.total_usage as f64
            - stats.precpu_stats.cpu_usage.total_usage as f64;

        let system_delta = stats.cpu_stats.system_cpu_usage.unwrap_or(0) as f64
            - stats.precpu_stats.system_cpu_usage.unwrap_or(0) as f64;

        let cpu_count = stats.cpu_stats.online_cpus.unwrap_or(1) as f64;

        if system_delta > 0.0 && cpu_delta > 0.0 {
            (cpu_delta / system_delta) * cpu_count * 100.0
        } else {
            0.0
        }
    }

    fn detect_compose_projects(containers: &[ContainerInfo]) -> Vec<ComposeProject> {
        let mut projects: HashMap<String, ComposeProject> = HashMap::new();

        for container in containers {
            if let Some(project_name) = &container.compose_project {
                let project =
                    projects
                        .entry(project_name.clone())
                        .or_insert_with(|| ComposeProject {
                            name: project_name.clone(),
                            working_dir: None,
                            services: Vec::new(),
                            containers_running: 0,
                            containers_total: 0,
                        });

                project.containers_total += 1;
                if container.state == "running" {
                    project.containers_running += 1;
                }

                if let Some(service) = &container.compose_service {
                    if !project.services.contains(service) {
                        project.services.push(service.clone());
                    }
                }
            }
        }

        projects.into_values().collect()
    }
}

/// Check if Docker is available on the system
#[allow(dead_code)]
pub async fn is_docker_available() -> bool {
    DockerCollector::new().await.is_ok()
}
