use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

pub struct DockerDeploy;

impl DockerDeploy {
    pub fn new() -> Self {
        Self
    }

    /// Pull images and restart containers using docker-compose
    pub async fn pull_and_restart(
        &self,
        compose_file: &str,
        services: &[String],
        prune: bool,
    ) -> Result<String, String> {
        let compose_path = Path::new(compose_file);
        let working_dir = compose_path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());

        let compose_filename = compose_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "docker-compose.yml".to_string());

        let mut output = String::new();

        // Pull images
        info!("Pulling Docker images");
        let pull_output = self
            .run_compose_command(&working_dir, &compose_filename, "pull", services)
            .await?;
        output.push_str(&format!("[docker compose pull]\n{}\n", pull_output));

        // Restart containers
        info!("Restarting containers");
        let up_output = self
            .run_compose_command(&working_dir, &compose_filename, "up", services)
            .await?;
        output.push_str(&format!("[docker compose up -d]\n{}\n", up_output));

        // Prune old images if requested
        if prune {
            info!("Pruning old images");
            match self.prune_images().await {
                Ok(prune_output) => {
                    output.push_str(&format!("[docker image prune]\n{}\n", prune_output));
                }
                Err(e) => {
                    output.push_str(&format!("[docker image prune] Warning: {}\n", e));
                }
            }
        }

        Ok(output)
    }

    /// Pull a specific Docker image
    #[allow(dead_code)]
    pub async fn pull_image(&self, image: &str) -> Result<String, String> {
        self.run_docker_command(&["pull", image]).await
    }

    /// Restart a container by name
    #[allow(dead_code)]
    pub async fn restart_container(&self, container: &str) -> Result<String, String> {
        self.run_docker_command(&["restart", container]).await
    }

    /// Prune unused images
    pub async fn prune_images(&self) -> Result<String, String> {
        self.run_docker_command(&["image", "prune", "-f"]).await
    }

    /// Stop containers using docker-compose down
    pub async fn down(&self, compose_file: &str) -> Result<String, String> {
        let compose_path = Path::new(compose_file);
        let working_dir = compose_path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());

        let compose_filename = compose_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "docker-compose.yml".to_string());

        self.run_compose_command(&working_dir, &compose_filename, "down", &[])
            .await
    }

    async fn run_compose_command(
        &self,
        working_dir: &str,
        compose_file: &str,
        action: &str,
        services: &[String],
    ) -> Result<String, String> {
        let mut args = vec!["compose", "-f", compose_file, action];

        // Add -d flag for "up" command
        if action == "up" {
            args.push("-d");
            args.push("--remove-orphans");
        }

        // Add specific services if provided
        let service_refs: Vec<&str> = services.iter().map(|s| s.as_str()).collect();
        args.extend(service_refs);

        let mut cmd = Command::new("docker");
        cmd.args(&args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        debug!(args = ?args, "Running docker compose command");

        let output = cmd
            .output()
            .await
            .map_err(|e| format!("Failed to execute docker compose: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("{}{}", stdout, stderr))
        } else {
            Err(format!(
                "Docker compose command failed: {}\n{}",
                stderr, stdout
            ))
        }
    }

    async fn run_docker_command(&self, args: &[&str]) -> Result<String, String> {
        let mut cmd = Command::new("docker");
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

        debug!(args = ?args, "Running docker command");

        let output = cmd
            .output()
            .await
            .map_err(|e| format!("Failed to execute docker: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("{}{}", stdout, stderr))
        } else {
            Err(format!("Docker command failed: {}\n{}", stderr, stdout))
        }
    }
}

impl Default for DockerDeploy {
    fn default() -> Self {
        Self::new()
    }
}
