use crate::deploy::DeployJob;
use crate::server::middleware::ErrorResponse;
use crate::server::AppState;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracing::{info, warn};

type HmacSha256 = Hmac<Sha256>;

fn format_rfc3339(dt: OffsetDateTime) -> String {
    dt.format(&Rfc3339).unwrap_or_default()
}

#[derive(Serialize)]
pub struct WebhookResponse {
    pub success: bool,
    pub message: String,
    pub job_id: Option<String>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct TriggerRequest {
    pub source: Option<String>,
}

/// POST /webhook/deploy/:name - Trigger a deployment
pub async fn trigger_deploy(
    State(state): State<Arc<AppState>>,
    Path(deployment_name): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<WebhookResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Find the deployment config
    let deployment = state
        .config
        .modules
        .deploy
        .deployments
        .iter()
        .find(|d| d.name == deployment_name)
        .cloned()
        .ok_or_else(|| {
            ErrorResponse::new(
                StatusCode::NOT_FOUND,
                &format!("Deployment '{}' not found", deployment_name),
            )
        })?;

    // Find webhook config for this deployment
    let webhook_config = state
        .config
        .modules
        .webhooks
        .endpoints
        .iter()
        .find(|e| e.deployment.as_ref() == Some(&deployment_name));

    // Verify webhook signature if configured
    if let Some(wh) = webhook_config {
        if let Some(secret) = &wh.secret {
            if !secret.is_empty() {
                verify_signature(&headers, &body, secret).map_err(|e| {
                    warn!(deployment = %deployment_name, error = %e, "Webhook signature verification failed");
                    ErrorResponse::new(StatusCode::UNAUTHORIZED, &e)
                })?;
            }
        }
    }

    // Determine trigger source
    let trigger_source = detect_trigger_source(&headers, &body);

    // Queue the deployment
    let queue = state.deploy_queue.as_ref().ok_or_else(|| {
        ErrorResponse::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Deployment queue not available",
        )
    })?;

    let job = DeployJob::new(
        "local".to_string(), // Agent name for local deployments
        deployment_name.clone(),
        deployment,
        trigger_source,
    );

    let job_id = queue.enqueue(job).await;

    info!(
        deployment = %deployment_name,
        job_id = %job_id,
        "Deployment queued"
    );

    Ok(Json(WebhookResponse {
        success: true,
        message: format!("Deployment '{}' queued", deployment_name),
        job_id: Some(job_id),
    }))
}

/// GET /webhook/status/:job_id - Get deployment job status
pub async fn get_job_status(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let queue = state.deploy_queue.as_ref().ok_or_else(|| {
        ErrorResponse::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Deployment queue not available",
        )
    })?;

    let job = queue
        .get_job(&job_id)
        .await
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "Job not found"))?;

    Ok(Json(serde_json::json!({
        "id": job.id,
        "deployment": job.deployment_name,
        "agent": job.agent_name,
        "status": format!("{:?}", job.status),
        "created_at": format_rfc3339(job.created_at),
        "started_at": job.started_at.map(format_rfc3339),
        "completed_at": job.completed_at.map(format_rfc3339),
        "trigger_source": job.trigger_source,
    })))
}

/// GET /webhook/queue - Get deployment queue status
pub async fn get_queue_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let queue = state.deploy_queue.as_ref().ok_or_else(|| {
        ErrorResponse::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Deployment queue not available",
        )
    })?;

    let jobs = queue.get_queue_status().await;
    let history = queue.get_history(20).await;

    Ok(Json(serde_json::json!({
        "pending": queue.len().await,
        "jobs": jobs.iter().map(|j| serde_json::json!({
            "id": j.id,
            "deployment": j.deployment_name,
            "status": format!("{:?}", j.status),
            "created_at": format_rfc3339(j.created_at),
        })).collect::<Vec<_>>(),
        "history": history.iter().map(|j| serde_json::json!({
            "id": j.id,
            "deployment": j.deployment_name,
            "status": format!("{:?}", j.status),
            "created_at": format_rfc3339(j.created_at),
            "completed_at": j.completed_at.map(format_rfc3339),
        })).collect::<Vec<_>>(),
    })))
}

/// Verify webhook signature (GitHub-style HMAC-SHA256)
fn verify_signature(headers: &HeaderMap, body: &[u8], secret: &str) -> Result<(), String> {
    // Try GitHub signature first
    if let Some(sig) = headers.get("x-hub-signature-256") {
        let sig_str = sig.to_str().map_err(|_| "Invalid signature header")?;
        let expected = sig_str
            .strip_prefix("sha256=")
            .ok_or("Invalid signature format")?;

        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| "Invalid secret key")?;
        mac.update(body);
        let computed = hex::encode(mac.finalize().into_bytes());

        if computed == expected {
            return Ok(());
        } else {
            return Err("Signature mismatch".to_string());
        }
    }

    // Try GitLab token
    if let Some(token) = headers.get("x-gitlab-token") {
        let token_str = token.to_str().map_err(|_| "Invalid token header")?;
        if token_str == secret {
            return Ok(());
        } else {
            return Err("Token mismatch".to_string());
        }
    }

    Err("No signature or token provided".to_string())
}

/// Detect the source of the webhook trigger
fn detect_trigger_source(headers: &HeaderMap, _body: &[u8]) -> Option<String> {
    // Check for GitHub
    if headers.contains_key("x-github-event") {
        return Some("github".to_string());
    }

    // Check for GitLab
    if headers.contains_key("x-gitlab-event") {
        return Some("gitlab".to_string());
    }

    // Check for Bitbucket
    if headers.contains_key("x-event-key") {
        return Some("bitbucket".to_string());
    }

    // Manual trigger
    Some("manual".to_string())
}
