mod docker;
mod executor;
mod git;
mod queue;
mod script;
mod telegram;

pub use executor::DeployExecutor;
pub use queue::{DeployJob, DeployQueue, JobStatus};

use crate::config::{DeployConfig, DeploymentConfig, TriggerConfig};
use crate::storage::{Database, DeployRecord, DeployStatus};
use std::collections::HashMap;
use std::sync::Arc;
use time::OffsetDateTime;
use tracing::{error, info, warn};

/// Result of a deployment operation
#[derive(Debug, Clone)]
pub struct DeployResult {
    pub success: bool,
    pub skipped: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration_ms: i64,
}

/// Start the deployment worker
pub async fn start_worker(
    queue: Arc<DeployQueue>,
    executor: Arc<DeployExecutor>,
    db: Option<Arc<Database>>,
    deploy_config: Arc<DeployConfig>,
) {
    info!("Starting deployment worker");

    loop {
        if let Some(job) = queue.next_job().await {
            info!(
                deployment = %job.deployment_name,
                agent = %job.agent_name,
                pipeline_id = %job.pipeline_id,
                "Processing deployment job"
            );

            // Update job status to running
            queue.update_status(&job.id, JobStatus::Running).await;

            // Record in database
            let deploy_id = if let Some(ref db) = db {
                let record = DeployRecord {
                    id: None,
                    agent_name: job.agent_name.clone(),
                    deployment_name: job.deployment_name.clone(),
                    deploy_type: format!("{:?}", job.config.deploy_type),
                    status: DeployStatus::Running,
                    started_at: OffsetDateTime::now_utc(),
                    completed_at: None,
                    duration_ms: None,
                    trigger_source: job.trigger_source.clone(),
                    commit_sha: None,
                    output: None,
                    error_message: None,
                };
                db.insert_deploy(&record).ok()
            } else {
                None
            };

            // Check if this is the root of a pipeline chain
            let is_pipeline_root = job.trigger_source.is_none()
                || !job.trigger_source.as_ref().unwrap().starts_with("trigger:");

            // 1. Pipeline on_start (only for root of chain)
            if is_pipeline_root && !job.config.pipeline.on_start.is_empty() {
                fire_triggers(
                    &job.config.pipeline.on_start,
                    &job,
                    &queue,
                    &deploy_config.deployments,
                    &build_trigger_env(&job, None, "on_start"),
                )
                .await;
            }

            // 2. Execute deployment
            let result = executor.execute(&job.config).await;

            // Update status based on result
            let final_status = if result.success {
                JobStatus::Completed
            } else {
                JobStatus::Failed
            };
            queue.update_status(&job.id, final_status).await;

            // Update database record
            if let (Some(ref db), Some(id)) = (&db, deploy_id) {
                let status = if result.success {
                    DeployStatus::Success
                } else {
                    DeployStatus::Failed
                };
                let _ = db.update_deploy_status(
                    id,
                    status,
                    Some(OffsetDateTime::now_utc()),
                    Some(result.duration_ms),
                    Some(&result.output),
                    result.error.as_deref(),
                );
            }

            if result.success && !result.skipped {
                info!(
                    deployment = %job.deployment_name,
                    duration_ms = result.duration_ms,
                    "Deployment completed successfully"
                );

                // 3a. Deployment on_success triggers
                if !job.config.on_success.is_empty() {
                    let env = build_trigger_env(&job, Some(&result), "on_success");
                    fire_triggers(
                        &job.config.on_success,
                        &job,
                        &queue,
                        &deploy_config.deployments,
                        &env,
                    )
                    .await;
                }

                // 3b. Global on_success triggers
                if !deploy_config.on_success.is_empty() {
                    let env = build_trigger_env(&job, Some(&result), "on_success");
                    fire_triggers(
                        &deploy_config.on_success,
                        &job,
                        &queue,
                        &deploy_config.deployments,
                        &env,
                    )
                    .await;
                }
            } else if result.skipped {
                info!(
                    deployment = %job.deployment_name,
                    "Deployment skipped (no changes), triggers not fired"
                );
            } else {
                error!(
                    deployment = %job.deployment_name,
                    error = ?result.error,
                    "Deployment failed"
                );

                // 4a. Deployment on_error triggers
                if !job.config.on_error.is_empty() {
                    let env = build_trigger_env(&job, Some(&result), "on_error");
                    fire_triggers(
                        &job.config.on_error,
                        &job,
                        &queue,
                        &deploy_config.deployments,
                        &env,
                    )
                    .await;
                }

                // 4b. Global on_error triggers
                if !deploy_config.on_error.is_empty() {
                    let env = build_trigger_env(&job, Some(&result), "on_error");
                    fire_triggers(
                        &deploy_config.on_error,
                        &job,
                        &queue,
                        &deploy_config.deployments,
                        &env,
                    )
                    .await;
                }
            }

            // 5. Pipeline on_finish (ALWAYS, if this is a terminal node)
            if is_chain_terminal(&job, &result) && !job.config.pipeline.on_finish.is_empty() {
                let env = build_trigger_env(&job, Some(&result), "on_finish");
                fire_triggers(
                    &job.config.pipeline.on_finish,
                    &job,
                    &queue,
                    &deploy_config.deployments,
                    &env,
                )
                .await;
            }
        }

        // Small delay to prevent busy loop
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

/// Build context environment variables for triggered deployments
fn build_trigger_env(
    job: &DeployJob,
    result: Option<&DeployResult>,
    trigger_type: &str,
) -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("DEPLOY_NAME".to_string(), job.deployment_name.clone());
    env.insert("AGENT_NAME".to_string(), job.agent_name.clone());
    env.insert("PIPELINE_ID".to_string(), job.pipeline_id.clone());
    env.insert("TRIGGER_TYPE".to_string(), trigger_type.to_string());

    if let Some(result) = result {
        env.insert(
            "DEPLOY_STATUS".to_string(),
            if result.success {
                "success".to_string()
            } else {
                "error".to_string()
            },
        );
        env.insert(
            "DEPLOY_ERROR".to_string(),
            result.error.clone().unwrap_or_default(),
        );
        env.insert(
            "DEPLOY_DURATION_MS".to_string(),
            result.duration_ms.to_string(),
        );
    }

    env
}

/// Determine if this job is the last in the pipeline chain
fn is_chain_terminal(job: &DeployJob, result: &DeployResult) -> bool {
    // Find the pipeline root — only root has pipeline config
    let has_pipeline =
        !job.config.pipeline.on_start.is_empty() || !job.config.pipeline.on_finish.is_empty();

    if !has_pipeline {
        // Check if this job was triggered by a pipeline root
        // Pipeline on_finish is only on the root config, so propagate it
        return false;
    }

    // Error breaks the chain (unless continue_on_failure)
    if !result.success && !job.config.continue_on_failure {
        return true;
    }

    // No further on_success triggers means end of chain
    if result.success && job.config.on_success.is_empty() {
        return true;
    }

    false
}

/// Fire triggers: enqueue triggered deployments with context env vars
async fn fire_triggers(
    trigger_config: &TriggerConfig,
    parent_job: &DeployJob,
    queue: &Arc<DeployQueue>,
    deployments: &[DeploymentConfig],
    context_env: &HashMap<String, String>,
) {
    let triggers = trigger_config.as_vec();
    let continue_on_failure = parent_job.config.continue_on_failure;

    for trigger_name in triggers {
        let trigger_deploy = deployments.iter().find(|d| d.name == trigger_name);

        match trigger_deploy {
            Some(config) => {
                let mut config = config.clone();
                // Merge context env vars into the triggered deployment's env
                for (k, v) in context_env {
                    config.env.entry(k.clone()).or_insert_with(|| v.clone());
                }

                info!(
                    parent = %parent_job.deployment_name,
                    trigger = %trigger_name,
                    trigger_type = context_env.get("TRIGGER_TYPE").map(|s| s.as_str()).unwrap_or("unknown"),
                    "Triggering deployment"
                );

                let trigger_source = format!("trigger:{}", parent_job.deployment_name);
                let job = DeployJob::new(
                    parent_job.agent_name.clone(),
                    trigger_name.to_string(),
                    config,
                    Some(trigger_source),
                    Some(parent_job.pipeline_id.clone()),
                );

                queue.enqueue(job).await;
            }
            None => {
                warn!(
                    parent = %parent_job.deployment_name,
                    trigger = %trigger_name,
                    "Triggered deployment not found in config"
                );

                if !continue_on_failure {
                    error!(
                        parent = %parent_job.deployment_name,
                        "Pipeline stopped: triggered deployment '{}' not found",
                        trigger_name
                    );
                    break;
                }
            }
        }
    }
}
