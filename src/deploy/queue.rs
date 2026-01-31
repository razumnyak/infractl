use crate::config::DeploymentConfig;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct DeployJob {
    pub id: String,
    pub agent_name: String,
    pub deployment_name: String,
    pub config: DeploymentConfig,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub trigger_source: Option<String>,
}

impl DeployJob {
    pub fn new(
        agent_name: String,
        deployment_name: String,
        config: DeploymentConfig,
        trigger_source: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent_name,
            deployment_name,
            config,
            status: JobStatus::Pending,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            trigger_source,
        }
    }
}

pub struct DeployQueue {
    jobs: RwLock<VecDeque<DeployJob>>,
    history: RwLock<Vec<DeployJob>>,
    max_history: usize,
}

impl DeployQueue {
    pub fn new(max_history: usize) -> Self {
        Self {
            jobs: RwLock::new(VecDeque::new()),
            history: RwLock::new(Vec::new()),
            max_history,
        }
    }

    /// Add a new job to the queue
    pub async fn enqueue(&self, job: DeployJob) -> String {
        let id = job.id.clone();
        let mut jobs = self.jobs.write().await;
        jobs.push_back(job);
        id
    }

    /// Get the next pending job
    pub async fn next_job(&self) -> Option<DeployJob> {
        let mut jobs = self.jobs.write().await;

        // Find the first pending job
        if let Some(pos) = jobs.iter().position(|j| j.status == JobStatus::Pending) {
            let mut job = jobs.remove(pos)?;
            job.status = JobStatus::Running;
            job.started_at = Some(Utc::now());

            // Keep in queue for status tracking
            jobs.push_front(job.clone());

            Some(job)
        } else {
            None
        }
    }

    /// Update job status
    pub async fn update_status(&self, job_id: &str, status: JobStatus) {
        let mut jobs = self.jobs.write().await;

        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            job.status = status.clone();

            if matches!(
                status,
                JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
            ) {
                job.completed_at = Some(Utc::now());

                // Move to history
                let completed_job = job.clone();
                drop(jobs);

                let mut history = self.history.write().await;
                history.push(completed_job);

                // Trim history if needed
                while history.len() > self.max_history {
                    history.remove(0);
                }

                // Remove from active queue
                let mut jobs = self.jobs.write().await;
                jobs.retain(|j| j.id != job_id);
            }
        }
    }

    /// Get current queue status
    pub async fn get_queue_status(&self) -> Vec<DeployJob> {
        let jobs = self.jobs.read().await;
        jobs.iter().cloned().collect()
    }

    /// Get job history
    pub async fn get_history(&self, limit: usize) -> Vec<DeployJob> {
        let history = self.history.read().await;
        history.iter().rev().take(limit).cloned().collect()
    }

    /// Get a specific job by ID
    pub async fn get_job(&self, job_id: &str) -> Option<DeployJob> {
        // Check active queue first
        {
            let jobs = self.jobs.read().await;
            if let Some(job) = jobs.iter().find(|j| j.id == job_id) {
                return Some(job.clone());
            }
        }

        // Check history
        {
            let history = self.history.read().await;
            if let Some(job) = history.iter().find(|j| j.id == job_id) {
                return Some(job.clone());
            }
        }

        None
    }

    /// Cancel a pending job
    #[allow(dead_code)]
    pub async fn cancel(&self, job_id: &str) -> bool {
        let mut jobs = self.jobs.write().await;

        if let Some(job) = jobs
            .iter_mut()
            .find(|j| j.id == job_id && j.status == JobStatus::Pending)
        {
            job.status = JobStatus::Cancelled;
            job.completed_at = Some(Utc::now());
            true
        } else {
            false
        }
    }

    /// Get queue length
    pub async fn len(&self) -> usize {
        let jobs = self.jobs.read().await;
        jobs.iter()
            .filter(|j| j.status == JobStatus::Pending)
            .count()
    }

    /// Check if queue is empty
    #[allow(dead_code)]
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

impl Default for DeployQueue {
    fn default() -> Self {
        Self::new(100)
    }
}
