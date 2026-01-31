use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricRecord {
    pub id: Option<i64>,
    pub agent_name: String,
    pub collected_at: DateTime<Utc>,
    pub cpu_usage: f64,
    pub memory_usage_percent: f64,
    pub memory_used: u64,
    pub memory_total: u64,
    pub load_one: f64,
    pub load_five: f64,
    pub load_fifteen: f64,
    pub disk_usage_percent: Option<f64>,
    pub containers_running: Option<u32>,
    pub containers_total: Option<u32>,
    pub raw_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetric {
    pub id: Option<i64>,
    pub agent_name: String,
    pub period_start: DateTime<Utc>,
    pub cpu_avg: f64,
    pub cpu_max: f64,
    pub memory_avg: f64,
    pub memory_max: f64,
    pub load_avg: f64,
    pub load_max: f64,
    pub samples_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployRecord {
    pub id: Option<i64>,
    pub agent_name: String,
    pub deployment_name: String,
    pub deploy_type: String,
    pub status: DeployStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub trigger_source: Option<String>,
    pub commit_sha: Option<String>,
    pub output: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeployStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl std::fmt::Display for DeployStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeployStatus::Pending => write!(f, "pending"),
            DeployStatus::Running => write!(f, "running"),
            DeployStatus::Success => write!(f, "success"),
            DeployStatus::Failed => write!(f, "failed"),
            DeployStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for DeployStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(DeployStatus::Pending),
            "running" => Ok(DeployStatus::Running),
            "success" => Ok(DeployStatus::Success),
            "failed" => Ok(DeployStatus::Failed),
            "cancelled" => Ok(DeployStatus::Cancelled),
            _ => Err(format!("Unknown deploy status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspiciousRequest {
    pub id: Option<i64>,
    pub recorded_at: DateTime<Utc>,
    pub source_ip: String,
    pub method: Option<String>,
    pub path: Option<String>,
    pub reason: String,
    pub user_agent: Option<String>,
    pub headers: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub agent_name: String,
    pub last_seen: DateTime<Utc>,
    pub status: String,
    pub version: Option<String>,
    pub uptime_seconds: Option<u64>,
}

/// Query parameters for historical metrics
#[derive(Debug, Clone, Deserialize)]
pub struct MetricsQuery {
    pub agent_name: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    #[allow(dead_code)]
    pub aggregation: Option<AggregationType>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AggregationType {
    #[default]
    Raw,
    Hourly,
    Daily,
}
