use crate::metrics::{DockerCollector, SystemCollector};
use crate::server::AppState;
use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub mode: String,
    pub system: crate::metrics::SystemMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker: Option<crate::metrics::DockerMetrics>,
}

pub async fn health_check(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let system = SystemCollector::collect();

    // Collect Docker metrics if enabled
    let docker = if state.config.modules.metrics.docker_stats {
        match DockerCollector::new().await {
            Ok(collector) => Some(collector.collect().await),
            Err(_) => None,
        }
    } else {
        None
    };

    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.uptime_seconds(),
        mode: format!("{:?}", state.config.mode).to_lowercase(),
        system,
        docker,
    };

    Json(response)
}
