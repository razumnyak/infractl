use crate::server::middleware::ErrorResponse;
use crate::server::AppState;
use crate::storage::{AggregationType, DeployRecord, MetricRecord, MetricsQuery};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct MetricsQueryParams {
    pub agent: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<u32>,
    #[serde(rename = "type")]
    pub aggregation_type: Option<String>,
}

#[allow(dead_code)]
#[derive(Serialize)]
pub struct MetricsResponse {
    pub metrics: Vec<MetricRecord>,
    pub count: usize,
}

#[allow(dead_code)]
#[derive(Serialize)]
pub struct AggregatedMetricsResponse {
    pub metrics: Vec<crate::storage::AggregatedMetric>,
    pub count: usize,
}

/// GET /api/metrics - Get historical metrics
pub async fn get_metrics(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MetricsQueryParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        ErrorResponse::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Database not available (Agent mode?)",
        )
    })?;

    let from = params
        .from
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let to = params
        .to
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let aggregation = params
        .aggregation_type
        .as_ref()
        .map(|s| match s.as_str() {
            "hourly" => AggregationType::Hourly,
            "daily" => AggregationType::Daily,
            _ => AggregationType::Raw,
        })
        .unwrap_or(AggregationType::Raw);

    match aggregation {
        AggregationType::Raw => {
            let query = MetricsQuery {
                agent_name: params.agent,
                from,
                to,
                limit: params.limit.or(Some(100)),
                aggregation: Some(aggregation),
            };

            let metrics = db.get_metrics(&query).map_err(|e| {
                ErrorResponse::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Database error: {}", e),
                )
            })?;

            let count = metrics.len();
            Ok(Json(serde_json::json!({
                "metrics": metrics,
                "count": count,
                "type": "raw"
            })))
        }
        AggregationType::Hourly => {
            let agent = params.agent.unwrap_or_default();
            let metrics = db.get_hourly_metrics(&agent, from, to).map_err(|e| {
                ErrorResponse::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Database error: {}", e),
                )
            })?;

            let count = metrics.len();
            Ok(Json(serde_json::json!({
                "metrics": metrics,
                "count": count,
                "type": "hourly"
            })))
        }
        AggregationType::Daily => {
            // For daily, we'd need to add a get_daily_metrics method
            // For now, use hourly as fallback
            let agent = params.agent.unwrap_or_default();
            let metrics = db.get_hourly_metrics(&agent, from, to).map_err(|e| {
                ErrorResponse::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Database error: {}", e),
                )
            })?;

            let count = metrics.len();
            Ok(Json(serde_json::json!({
                "metrics": metrics,
                "count": count,
                "type": "daily"
            })))
        }
    }
}

#[derive(Deserialize)]
pub struct DeployQueryParams {
    pub agent: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct DeployHistoryResponse {
    pub deployments: Vec<DeployRecord>,
    pub count: usize,
}

/// GET /api/deploys - Get deployment history
pub async fn get_deploy_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DeployQueryParams>,
) -> Result<Json<DeployHistoryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        ErrorResponse::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Database not available (Agent mode?)",
        )
    })?;

    let deployments = db
        .get_deploy_history(params.agent.as_deref(), params.limit.unwrap_or(50))
        .map_err(|e| {
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Database error: {}", e),
            )
        })?;

    let count = deployments.len();
    Ok(Json(DeployHistoryResponse { deployments, count }))
}

#[derive(Deserialize)]
pub struct SuspiciousQueryParams {
    pub limit: Option<u32>,
}

/// GET /api/suspicious - Get suspicious requests log
pub async fn get_suspicious_requests(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SuspiciousQueryParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        ErrorResponse::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Database not available (Agent mode?)",
        )
    })?;

    let requests = db
        .get_suspicious_requests(params.limit.unwrap_or(100))
        .map_err(|e| {
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Database error: {}", e),
            )
        })?;

    let count = requests.len();
    Ok(Json(serde_json::json!({
        "requests": requests,
        "count": count
    })))
}

/// GET /api/agents/:name/status - Get agent status
pub async fn get_agent_status(
    State(state): State<Arc<AppState>>,
    Path(agent_name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        ErrorResponse::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Database not available (Agent mode?)",
        )
    })?;

    let status = db.get_agent_status(&agent_name).map_err(|e| {
        ErrorResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Database error: {}", e),
        )
    })?;

    match status {
        Some(s) => Ok(Json(serde_json::json!(s))),
        None => Err(ErrorResponse::new(StatusCode::NOT_FOUND, "Agent not found")),
    }
}

/// GET /api/agents/statuses - Get all agent statuses
pub async fn get_all_agent_statuses(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        ErrorResponse::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Database not available (Agent mode?)",
        )
    })?;

    let statuses = db.get_all_agent_statuses().map_err(|e| {
        ErrorResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Database error: {}", e),
        )
    })?;

    Ok(Json(serde_json::json!({
        "agents": statuses,
        "count": statuses.len()
    })))
}
