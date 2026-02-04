mod docker;
mod executor;
mod git;
mod queue;
mod script;

pub use executor::DeployExecutor;
pub use queue::{DeployJob, DeployQueue, JobStatus};

use crate::config::DeploymentConfig;
use crate::storage::{Database, DeployRecord, DeployStatus};
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
    deployments: Arc<Vec<DeploymentConfig>>,
) {
    info!("Starting deployment worker");

    loop {
        if let Some(job) = queue.next_job().await {
            info!(
                deployment = %job.deployment_name,
                agent = %job.agent_name,
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

            // Execute deployment
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

                // Process triggers
                if !job.config.trigger.is_empty() {
                    process_triggers(&job, &queue, &deployments).await;
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
            }
        }

        // Small delay to prevent busy loop
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

/// Process trigger pipeline after successful deployment
async fn process_triggers(
    parent_job: &DeployJob,
    queue: &Arc<DeployQueue>,
    deployments: &[DeploymentConfig],
) {
    let triggers = parent_job.config.trigger.as_vec();
    let continue_on_failure = parent_job.config.continue_on_failure;

    for trigger_name in triggers {
        // Find the deployment config by name
        let trigger_config = deployments.iter().find(|d| d.name == trigger_name);

        match trigger_config {
            Some(config) => {
                info!(
                    parent = %parent_job.deployment_name,
                    trigger = %trigger_name,
                    "Triggering next deployment in pipeline"
                );

                let trigger_source = format!("trigger:{}", parent_job.deployment_name);

                let job = DeployJob::new(
                    parent_job.agent_name.clone(),
                    trigger_name.to_string(),
                    config.clone(),
                    Some(trigger_source),
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
