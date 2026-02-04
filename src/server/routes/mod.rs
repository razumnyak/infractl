mod api;
mod health;
mod webhook;

use crate::server::assets;
use crate::server::AppState;
use axum::{
    extract::State,
    response::Response,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

/// Routes common to both modes
pub fn common() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(root))
        // Webhook routes available on both modes
        .route("/webhook/deploy/:name", post(webhook::trigger_deploy))
        .route("/webhook/shutdown/:name", post(webhook::trigger_shutdown))
        .route("/webhook/status/:job_id", get(webhook::get_job_status))
        .route("/webhook/queue", get(webhook::get_queue_status))
}

/// Agent mode routes
pub fn agent() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(health::health_check))
}

/// Home mode routes
pub fn home() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health::health_check))
        .route("/monitoring", get(monitoring_dashboard))
        // Agent list
        .route("/api/agents", get(list_agents))
        .route("/api/agents/statuses", get(api::get_all_agent_statuses))
        .route("/api/agents/:name/status", get(api::get_agent_status))
        // Historical data
        .route("/api/metrics", get(api::get_metrics))
        .route("/api/deploys", get(api::get_deploy_history))
        .route("/api/suspicious", get(api::get_suspicious_requests))
        // Deployments config
        .route("/api/deployments", get(api::get_deployments))
}

async fn root() -> &'static str {
    "infractl"
}

async fn monitoring_dashboard(State(state): State<Arc<AppState>>) -> Response {
    assets::serve_dashboard_with_token(&state.config.auth.jwt_secret).await
}

async fn list_agents(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::Json<serde_json::Value> {
    let agents: Vec<_> = state
        .config
        .agents
        .iter()
        .map(|a| {
            serde_json::json!({
                "name": a.name,
                "address": a.address,
                "status": "unknown"
            })
        })
        .collect();

    axum::Json(serde_json::json!({ "agents": agents }))
}
