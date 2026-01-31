use crate::error::{InfraError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Home,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub mode: Mode,
    #[serde(default = "default_version")]
    pub version: String,
    pub server: ServerConfig,
    pub auth: AuthConfig,
    #[serde(default)]
    pub updates: UpdatesConfig,
    #[serde(default)]
    pub agents: Vec<AgentConfig>,
    #[serde(default)]
    pub modules: ModulesConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
}

fn default_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_true")]
    pub isolation_mode: bool,
    #[serde(default = "default_allowed_networks")]
    pub allowed_networks: Vec<String>,
}

fn default_bind() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8111
}

fn default_true() -> bool {
    true
}

fn default_allowed_networks() -> Vec<String> {
    vec![
        "10.0.0.0/8".to_string(),
        "172.16.0.0/12".to_string(),
        "192.168.0.0/16".to_string(),
        "127.0.0.1/32".to_string(),
    ]
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            port: default_port(),
            isolation_mode: true,
            allowed_networks: default_allowed_networks(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    #[serde(default = "default_token_ttl")]
    pub token_ttl: String,
    #[serde(default)]
    pub webhook_secrets: HashMap<String, String>,
}

fn default_token_ttl() -> String {
    "24h".to_string()
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: String::new(),
            token_ttl: default_token_ttl(),
            webhook_secrets: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdatesConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub self_update: SelfUpdateConfig,
    #[serde(default)]
    pub config_update: ConfigUpdateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelfUpdateConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub github_repo: String,
    #[serde(default = "default_check_interval")]
    pub check_interval: String,
    #[serde(default)]
    pub prerelease: bool,
}

fn default_check_interval() -> String {
    "6h".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigUpdateConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub github_raw_url: String,
    #[serde(default = "default_config_check_interval")]
    pub check_interval: String,
    #[serde(default = "default_true")]
    pub backup: bool,
}

fn default_config_check_interval() -> String {
    "1h".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub address: String,
    #[serde(default = "default_timeout")]
    pub timeout: String,
    #[serde(default = "default_health_interval")]
    pub health_interval: String,
}

fn default_timeout() -> String {
    "10s".to_string()
}

fn default_health_interval() -> String {
    "30s".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulesConfig {
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub deploy: DeployConfig,
    #[serde(default)]
    pub webhooks: WebhooksConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_collect_interval")]
    pub collect_interval: String,
    #[serde(default = "default_true")]
    pub docker_stats: bool,
    #[serde(default)]
    pub docker_socket: Option<String>,
    #[serde(default = "default_true")]
    pub compose_projects: bool,
}

fn default_collect_interval() -> String {
    "30s".to_string()
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collect_interval: default_collect_interval(),
            docker_stats: true,
            docker_socket: None,
            compose_projects: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default)]
    pub retention: RetentionConfig,
    #[serde(default)]
    pub aggregation: AggregationConfig,
}

fn default_db_path() -> String {
    "/var/lib/infractl/metrics.db".to_string()
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            db_path: default_db_path(),
            retention: RetentionConfig::default(),
            aggregation: AggregationConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    #[serde(default = "default_raw_retention")]
    pub raw_data: String,
    #[serde(default = "default_hourly_retention")]
    pub hourly_data: String,
    #[serde(default = "default_daily_retention")]
    pub daily_data: String,
}

fn default_raw_retention() -> String {
    "7d".to_string()
}

fn default_hourly_retention() -> String {
    "30d".to_string()
}

fn default_daily_retention() -> String {
    "365d".to_string()
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            raw_data: default_raw_retention(),
            hourly_data: default_hourly_retention(),
            daily_data: default_daily_retention(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationConfig {
    #[serde(default = "default_hourly_cron")]
    pub hourly: String,
    #[serde(default = "default_daily_cron")]
    pub daily: String,
}

fn default_hourly_cron() -> String {
    "0 * * * *".to_string()
}

fn default_daily_cron() -> String {
    "0 0 * * *".to_string()
}

impl Default for AggregationConfig {
    fn default() -> Self {
        Self {
            hourly: default_hourly_cron(),
            daily: default_daily_cron(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeployConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_work_dir")]
    pub work_dir: String,
    #[serde(default = "default_deploy_timeout")]
    pub default_timeout: String,
    #[serde(default)]
    pub deployments: Vec<DeploymentConfig>,
}

fn default_work_dir() -> String {
    "/opt/apps".to_string()
}

fn default_deploy_timeout() -> String {
    "300s".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub deploy_type: DeployType,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub remote: Option<String>,
    #[serde(default)]
    pub ssh_key: Option<String>,
    #[serde(default)]
    pub compose_file: Option<String>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub pre_deploy: Vec<String>,
    #[serde(default)]
    pub post_deploy: Vec<String>,
    #[serde(default)]
    pub timeout: Option<String>,
    #[serde(default)]
    pub prune: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeployType {
    GitPull,
    DockerPull,
    CustomScript,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebhooksConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub endpoints: Vec<WebhookEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub path: String,
    #[serde(default)]
    pub deployment: Option<String>,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub allowed_ips: Vec<String>,
    #[serde(default)]
    pub schedule_constraint: Option<ScheduleConstraint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConstraint {
    pub allowed_hours: Vec<u8>,
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

fn default_timezone() -> String {
    "UTC".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_format")]
    pub format: String,
    #[serde(default = "default_log_file")]
    pub file: Option<String>,
    #[serde(default = "default_suspicious_log")]
    pub suspicious_requests: Option<String>,
    #[serde(default)]
    pub rotation: Option<LogRotationConfig>,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "json".to_string()
}

fn default_log_file() -> Option<String> {
    Some("/var/log/infractl/infractl.log".to_string())
}

fn default_suspicious_log() -> Option<String> {
    Some("/var/log/infractl/suspicious.log".to_string())
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            file: default_log_file(),
            suspicious_requests: default_suspicious_log(),
            rotation: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRotationConfig {
    pub max_size: String,
    pub max_files: u32,
    #[serde(default)]
    pub compress: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub on_deploy: OnDeployNotify,
    #[serde(default)]
    pub channels: Vec<NotificationChannel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OnDeployNotify {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub failure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationChannel {
    #[serde(rename = "type")]
    pub channel_type: String,
    #[serde(default)]
    pub webhook_url: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

/// Load config from file with environment variable substitution
pub fn load(path: &Path) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| InfraError::Config(format!("Failed to read config file: {}", e)))?;

    let content = substitute_env_vars(&content)?;

    let config: Config = serde_yaml::from_str(&content)?;

    validate(&config)?;

    Ok(config)
}

/// Substitute ${VAR} patterns with environment variables
fn substitute_env_vars(content: &str) -> Result<String> {
    let re = Regex::new(r"\$\{([^}]+)\}").unwrap();
    let mut result = content.to_string();
    let mut missing_vars = Vec::new();

    for cap in re.captures_iter(content) {
        let var_name = &cap[1];
        let placeholder = &cap[0];

        match std::env::var(var_name) {
            Ok(value) => {
                result = result.replace(placeholder, &value);
            }
            Err(_) => {
                missing_vars.push(var_name.to_string());
            }
        }
    }

    if !missing_vars.is_empty() {
        // For non-critical vars, we can leave them empty or use defaults
        // For critical vars like JWT_SECRET in production, this would error
        for var in &missing_vars {
            let placeholder = format!("${{{}}}", var);
            result = result.replace(&placeholder, "");
        }
        tracing::warn!(
            missing = ?missing_vars,
            "Some environment variables are not set"
        );
    }

    Ok(result)
}

/// Validate configuration
fn validate(config: &Config) -> Result<()> {
    // JWT secret must be set in production
    if config.auth.jwt_secret.is_empty() {
        return Err(InfraError::Config(
            "JWT secret must be set (auth.jwt_secret or JWT_SECRET env var)".to_string(),
        ));
    }

    // Validate allowed networks are valid CIDR
    for network in &config.server.allowed_networks {
        network
            .parse::<ipnetwork::IpNetwork>()
            .map_err(|_| InfraError::Config(format!("Invalid network CIDR: {}", network)))?;
    }

    // Home mode must have at least one agent defined
    if config.mode == Mode::Home && config.agents.is_empty() {
        tracing::warn!("Home mode with no agents configured");
    }

    // Validate agent addresses
    for agent in &config.agents {
        if agent.address.is_empty() {
            return Err(InfraError::Config(format!(
                "Agent '{}' has empty address",
                agent.name
            )));
        }
    }

    // Validate deployments
    for deploy in &config.modules.deploy.deployments {
        match deploy.deploy_type {
            DeployType::GitPull => {
                if deploy.path.is_none() {
                    return Err(InfraError::Config(format!(
                        "Deployment '{}' of type git_pull requires 'path'",
                        deploy.name
                    )));
                }
            }
            DeployType::DockerPull => {
                if deploy.compose_file.is_none() {
                    return Err(InfraError::Config(format!(
                        "Deployment '{}' of type docker_pull requires 'compose_file'",
                        deploy.name
                    )));
                }
            }
            DeployType::CustomScript => {
                if deploy.script.is_none() {
                    return Err(InfraError::Config(format!(
                        "Deployment '{}' of type custom_script requires 'script'",
                        deploy.name
                    )));
                }
            }
        }
    }

    Ok(())
}

/// Check if an IP is allowed based on network configuration
#[allow(dead_code)]
pub fn is_ip_allowed(ip: &IpAddr, allowed_networks: &[String]) -> bool {
    for network_str in allowed_networks {
        if let Ok(network) = network_str.parse::<ipnetwork::IpNetwork>() {
            if network.contains(*ip) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_substitution() {
        std::env::set_var("TEST_VAR", "test_value");
        let content = "key: ${TEST_VAR}";
        let result = substitute_env_vars(content).unwrap();
        assert_eq!(result, "key: test_value");
        std::env::remove_var("TEST_VAR");
    }

    #[test]
    fn test_ip_allowed() {
        let networks = vec!["10.0.0.0/8".to_string(), "127.0.0.1/32".to_string()];

        assert!(is_ip_allowed(&"10.1.2.3".parse().unwrap(), &networks));
        assert!(is_ip_allowed(&"127.0.0.1".parse().unwrap(), &networks));
        assert!(!is_ip_allowed(&"8.8.8.8".parse().unwrap(), &networks));
    }
}
