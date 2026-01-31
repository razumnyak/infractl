use super::docker::DockerDeploy;
use super::git::GitDeploy;
use super::script::ScriptRunner;
use super::DeployResult;
use crate::config::{DeployType, DeploymentConfig};
use std::time::Instant;
use tracing::{error, info};

pub struct DeployExecutor {
    git: GitDeploy,
    docker: DockerDeploy,
    script: ScriptRunner,
}

impl DeployExecutor {
    pub fn new() -> Self {
        Self {
            git: GitDeploy::new(),
            docker: DockerDeploy::new(),
            script: ScriptRunner::new(),
        }
    }

    pub async fn execute(&self, config: &DeploymentConfig) -> DeployResult {
        let start = Instant::now();
        let mut output = String::new();

        info!(
            deployment = %config.name,
            deploy_type = ?config.deploy_type,
            "Starting deployment"
        );

        // Run pre-deploy commands
        if !config.pre_deploy.is_empty() {
            info!("Running pre-deploy commands");
            for cmd in &config.pre_deploy {
                match self
                    .script
                    .run_command(cmd, config.path.as_deref(), &config.env)
                    .await
                {
                    Ok(cmd_output) => {
                        output.push_str(&format!("[pre-deploy] {}\n{}\n", cmd, cmd_output));
                    }
                    Err(e) => {
                        let error_msg = format!("Pre-deploy command failed: {}", e);
                        error!("{}", error_msg);
                        return DeployResult {
                            success: false,
                            output,
                            error: Some(error_msg),
                            duration_ms: start.elapsed().as_millis() as i64,
                        };
                    }
                }
            }
        }

        // Execute main deployment based on type
        let result = match config.deploy_type {
            DeployType::GitPull => self.execute_git_pull(config).await,
            DeployType::DockerPull => self.execute_docker_pull(config).await,
            DeployType::CustomScript => self.execute_custom_script(config).await,
        };

        match result {
            Ok(deploy_output) => {
                output.push_str(&deploy_output);
            }
            Err(e) => {
                let error_msg = format!("Deployment failed: {}", e);
                error!("{}", error_msg);
                return DeployResult {
                    success: false,
                    output,
                    error: Some(error_msg),
                    duration_ms: start.elapsed().as_millis() as i64,
                };
            }
        }

        // Run post-deploy commands
        if !config.post_deploy.is_empty() {
            info!("Running post-deploy commands");
            for cmd in &config.post_deploy {
                match self
                    .script
                    .run_command(cmd, config.path.as_deref(), &config.env)
                    .await
                {
                    Ok(cmd_output) => {
                        output.push_str(&format!("[post-deploy] {}\n{}\n", cmd, cmd_output));
                    }
                    Err(e) => {
                        let error_msg = format!("Post-deploy command failed: {}", e);
                        error!("{}", error_msg);
                        return DeployResult {
                            success: false,
                            output,
                            error: Some(error_msg),
                            duration_ms: start.elapsed().as_millis() as i64,
                        };
                    }
                }
            }
        }

        DeployResult {
            success: true,
            output,
            error: None,
            duration_ms: start.elapsed().as_millis() as i64,
        }
    }

    async fn execute_git_pull(&self, config: &DeploymentConfig) -> Result<String, String> {
        let path = config
            .path
            .as_ref()
            .ok_or_else(|| "Git pull requires 'path' to be set".to_string())?;

        let branch = config.branch.as_deref().unwrap_or("main");
        let remote = config.remote.as_deref().unwrap_or("origin");

        self.git
            .pull(path, remote, branch, config.ssh_key.as_deref())
            .await
    }

    async fn execute_docker_pull(&self, config: &DeploymentConfig) -> Result<String, String> {
        let compose_file = config
            .compose_file
            .as_ref()
            .ok_or_else(|| "Docker pull requires 'compose_file' to be set".to_string())?;

        self.docker
            .pull_and_restart(compose_file, config.services.as_slice(), config.prune)
            .await
    }

    async fn execute_custom_script(&self, config: &DeploymentConfig) -> Result<String, String> {
        let script = config
            .script
            .as_ref()
            .ok_or_else(|| "Custom script requires 'script' to be set".to_string())?;

        let working_dir = config.working_dir.as_deref().or(config.path.as_deref());

        self.script
            .run_script(script, working_dir, &config.env, config.user.as_deref())
            .await
    }
}

impl Default for DeployExecutor {
    fn default() -> Self {
        Self::new()
    }
}
