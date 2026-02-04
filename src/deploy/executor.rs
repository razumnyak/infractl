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

        // Ensure working directory exists (create recursively if needed)
        if let Some(ref path) = config.path {
            let p = std::path::Path::new(path.as_str());
            if !p.exists() {
                if let Err(e) = std::fs::create_dir_all(p) {
                    let error_msg = format!("Failed to create path '{}': {}", path, e);
                    error!("{}", error_msg);
                    return DeployResult {
                        success: false,
                        skipped: false,
                        output,
                        error: Some(error_msg),
                        duration_ms: start.elapsed().as_millis() as i64,
                    };
                }
                info!(path = %path, "Created deployment directory");
            }
        }

        // Run pre-deploy commands
        if !config.pre_deploy.is_empty() {
            info!("Running pre-deploy commands");
            for cmd in config.pre_deploy.as_vec() {
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
                            skipped: false,
                            output,
                            error: Some(error_msg),
                            duration_ms: start.elapsed().as_millis() as i64,
                        };
                    }
                }
            }
        }

        // Fetch files from git if configured (works for all deploy types)
        if !config.git_files.is_empty() {
            let path = config
                .path
                .as_ref()
                .ok_or_else(|| "git_files requires 'path' to be set".to_string());
            let repo = config
                .repo
                .as_ref()
                .ok_or_else(|| "git_files requires 'repo' to be set".to_string());

            match (path, repo) {
                (Ok(path), Ok(repo)) => {
                    let branch = config.branch.as_deref().unwrap_or("main");

                    let file_mappings = parse_file_mappings(&config.git_files);
                    match file_mappings {
                        Ok(mappings) => {
                            info!(
                                repo = %repo,
                                files = ?config.git_files,
                                "Fetching files from git"
                            );
                            match self
                                .git
                                .fetch_files(
                                    repo,
                                    branch,
                                    &mappings,
                                    path,
                                    config.ssh_key.as_deref(),
                                )
                                .await
                            {
                                Ok(fetch_output) => output.push_str(&fetch_output),
                                Err(e) => {
                                    let error_msg = format!("git_files fetch failed: {}", e);
                                    error!("{}", error_msg);
                                    return DeployResult {
                                        success: false,
                                        skipped: false,
                                        output,
                                        error: Some(error_msg),
                                        duration_ms: start.elapsed().as_millis() as i64,
                                    };
                                }
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("git_files parse failed: {}", e);
                            error!("{}", error_msg);
                            return DeployResult {
                                success: false,
                                skipped: false,
                                output,
                                error: Some(error_msg),
                                duration_ms: start.elapsed().as_millis() as i64,
                            };
                        }
                    }
                }
                (Err(e), _) | (_, Err(e)) => {
                    let error_msg = e;
                    error!("{}", error_msg);
                    return DeployResult {
                        success: false,
                        skipped: false,
                        output,
                        error: Some(error_msg),
                        duration_ms: start.elapsed().as_millis() as i64,
                    };
                }
            }
        }

        // Execute main deployment based on type
        let mut skipped = false;
        let result = match config.deploy_type {
            DeployType::GitPull => match self.execute_git_pull(config).await {
                Ok((deploy_output, has_changes)) => {
                    if !has_changes {
                        skipped = true;
                    }
                    Ok(deploy_output)
                }
                Err(e) => Err(e),
            },
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
                    skipped: false,
                    output,
                    error: Some(error_msg),
                    duration_ms: start.elapsed().as_millis() as i64,
                };
            }
        }

        // Run post-deploy commands (skip if no changes detected)
        if !skipped && !config.post_deploy.is_empty() {
            info!("Running post-deploy commands");
            for cmd in config.post_deploy.as_vec() {
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
                            skipped: false,
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
            skipped,
            output,
            error: None,
            duration_ms: start.elapsed().as_millis() as i64,
        }
    }

    async fn execute_git_pull(&self, config: &DeploymentConfig) -> Result<(String, bool), String> {
        let path = config
            .path
            .as_ref()
            .ok_or_else(|| "Git pull requires 'path' to be set".to_string())?;

        let branch = config.branch.as_deref().unwrap_or("main");
        let remote = config.remote.as_deref().unwrap_or("origin");
        let git_dir = std::path::Path::new(path).join(".git");

        // Check if repo exists, if not - clone first (always has changes)
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

            return Ok((format!("[git clone] {}\n", clone_output), true));
        }

        // Repo exists, do pull — returns (output, has_changes)
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

        // Determine compose file path
        let compose_file = config
            .compose_file
            .as_deref()
            .unwrap_or("docker-compose.yaml");
        let full_compose_path = std::path::Path::new(path)
            .join(compose_file)
            .to_string_lossy()
            .to_string();

        // Check if compose file exists
        if !std::path::Path::new(&full_compose_path).exists() {
            return Err(format!("Compose file not found: {}", full_compose_path));
        }

        // Run docker compose
        let strategy = config.strategy.clone().unwrap_or_default();
        let docker_output = self
            .docker
            .pull_and_restart(
                &full_compose_path,
                config.services.as_slice(),
                config.prune,
                &strategy,
            )
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

        // Check if script is a file path or inline script
        let is_file = std::path::Path::new(script.trim()).exists()
            || (!script.contains('\n') && !script.contains(' ') && script.ends_with(".sh"));

        if is_file {
            self.script
                .run_script(script, working_dir, &config.env, config.user.as_deref())
                .await
        } else {
            // Inline script — run via sh -c
            self.script
                .run_command(script, working_dir.or(config.path.as_deref()), &config.env)
                .await
        }
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
            for cmd in config.shutdown.as_vec() {
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
                            skipped: false,
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
                let full_compose_path = std::path::Path::new(path)
                    .join(compose_file)
                    .to_string_lossy()
                    .to_string();

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
                                skipped: false,
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
            skipped: false,
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
