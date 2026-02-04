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
        let git_dir = std::path::Path::new(path).join(".git");

        // Check if repo exists, if not - clone first
        if !git_dir.exists() {
            let repo_url = config
                .repo
                .as_ref()
                .ok_or_else(|| "Git clone requires 'repo' URL to be set".to_string())?;

            info!(repo = %repo_url, path = %path, "Cloning repository (first deploy)");

            // Create parent directory if needed
            if let Some(parent) = std::path::Path::new(path).parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create directory: {}", e))?;
                }
            }

            let clone_output = self
                .git
                .clone(repo_url, path, Some(branch), config.ssh_key.as_deref())
                .await?;

            return Ok(format!("[git clone] {}\n", clone_output));
        }

        // Repo exists, do pull
        self.git
            .pull(path, remote, branch, config.ssh_key.as_deref())
            .await
    }

    async fn execute_docker_pull(&self, config: &DeploymentConfig) -> Result<String, String> {
        let path = config
            .path
            .as_ref()
            .ok_or_else(|| "docker_pull requires 'path' to be set".to_string())?;

        let mut output = String::new();

        // Create path directory if it doesn't exist
        if !std::path::Path::new(path).exists() {
            std::fs::create_dir_all(path)
                .map_err(|e| format!("Failed to create directory {}: {}", path, e))?;
            info!(path = %path, "Created deployment directory");
        }

        // If git_compose_files is set, fetch files from git
        if !config.git_compose_files.is_empty() {
            let repo = config
                .repo
                .as_ref()
                .ok_or_else(|| "git_compose_files requires 'repo' to be set".to_string())?;

            let branch = config.branch.as_deref().unwrap_or("main");

            // Parse file mappings (from:to format)
            let file_mappings = parse_file_mappings(&config.git_compose_files)?;

            info!(
                repo = %repo,
                files = ?config.git_compose_files,
                "Fetching files from git"
            );

            let fetch_output = self
                .git
                .fetch_files(
                    repo,
                    branch,
                    &file_mappings,
                    path,
                    config.ssh_key.as_deref(),
                )
                .await?;
            output.push_str(&fetch_output);
        }

        // Determine compose file path
        let compose_file = config
            .compose_file
            .as_deref()
            .unwrap_or("docker-compose.yaml");
        let full_compose_path = format!("{}/{}", path, compose_file);

        // Check if compose file exists
        if !std::path::Path::new(&full_compose_path).exists() {
            return Err(format!("Compose file not found: {}", full_compose_path));
        }

        // Run docker compose
        let docker_output = self
            .docker
            .pull_and_restart(&full_compose_path, config.services.as_slice(), config.prune)
            .await?;
        output.push_str(&docker_output);

        Ok(output)
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

    /// Execute shutdown commands for a deployment
    pub async fn shutdown(&self, config: &DeploymentConfig) -> DeployResult {
        let start = Instant::now();
        let mut output = String::new();

        info!(
            deployment = %config.name,
            deploy_type = ?config.deploy_type,
            "Shutting down deployment"
        );

        // If explicit shutdown commands are specified, use them
        if !config.shutdown.is_empty() {
            info!("Running shutdown commands");
            for cmd in &config.shutdown {
                match self
                    .script
                    .run_command(cmd, config.path.as_deref(), &config.env)
                    .await
                {
                    Ok(cmd_output) => {
                        output.push_str(&format!("[shutdown] {}\n{}\n", cmd, cmd_output));
                    }
                    Err(e) => {
                        let error_msg = format!("Shutdown command failed: {}", e);
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
        } else if config.deploy_type == DeployType::DockerPull {
            // Default: docker compose down for docker_pull
            if let Some(ref path) = config.path {
                let compose_file = config
                    .compose_file
                    .as_deref()
                    .unwrap_or("docker-compose.yaml");
                let full_compose_path = format!("{}/{}", path, compose_file);

                if std::path::Path::new(&full_compose_path).exists() {
                    info!(compose_file = %full_compose_path, "Running docker compose down");
                    match self.docker.down(&full_compose_path).await {
                        Ok(docker_output) => {
                            output.push_str(&format!(
                                "[shutdown] docker compose down\n{}\n",
                                docker_output
                            ));
                        }
                        Err(e) => {
                            let error_msg = format!("Docker compose down failed: {}", e);
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
        } else {
            output.push_str("[shutdown] No shutdown commands configured\n");
        }

        DeployResult {
            success: true,
            output,
            error: None,
            duration_ms: start.elapsed().as_millis() as i64,
        }
    }
}

/// Parse file mappings from "from:to" format
fn parse_file_mappings(files: &[String]) -> Result<Vec<(String, String)>, String> {
    files
        .iter()
        .map(|s| {
            let parts: Vec<&str> = s.splitn(2, ':').collect();
            if parts.len() != 2 {
                return Err(format!("Invalid format '{}', expected 'from:to'", s));
            }
            Ok((parts[0].to_string(), parts[1].to_string()))
        })
        .collect()
}

impl Default for DeployExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_mappings_valid() {
        let files = vec![
            "docker-compose.yaml:docker-compose.yaml".to_string(),
            "nginx/conf.d/:conf.d/".to_string(),
        ];
        let result = parse_file_mappings(&files).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            (
                "docker-compose.yaml".to_string(),
                "docker-compose.yaml".to_string()
            )
        );
        assert_eq!(
            result[1],
            ("nginx/conf.d/".to_string(), "conf.d/".to_string())
        );
    }

    #[test]
    fn test_parse_file_mappings_with_colons_in_path() {
        // Colon in the "to" part should work (splitn with 2)
        let files = vec!["from:to:with:colons".to_string()];
        let result = parse_file_mappings(&files).unwrap();
        assert_eq!(
            result[0],
            ("from".to_string(), "to:with:colons".to_string())
        );
    }

    #[test]
    fn test_parse_file_mappings_invalid() {
        let files = vec!["no-colon-here".to_string()];
        let result = parse_file_mappings(&files);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid format"));
    }

    #[test]
    fn test_parse_file_mappings_empty() {
        let files: Vec<String> = vec![];
        let result = parse_file_mappings(&files).unwrap();
        assert!(result.is_empty());
    }
}
