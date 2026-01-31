use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

pub struct GitDeploy;

impl GitDeploy {
    pub fn new() -> Self {
        Self
    }

    /// Pull latest changes from remote
    pub async fn pull(
        &self,
        repo_path: &str,
        remote: &str,
        branch: &str,
        ssh_key: Option<&str>,
    ) -> Result<String, String> {
        let path = Path::new(repo_path);

        if !path.exists() {
            return Err(format!("Repository path does not exist: {}", repo_path));
        }

        let mut output = String::new();

        // Set up SSH command if key is provided
        let git_ssh_command = ssh_key.map(|key| {
            format!(
                "ssh -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
                key
            )
        });

        // Fetch from remote
        info!(remote = %remote, branch = %branch, "Fetching from remote");
        let fetch_output = self
            .run_git_command(
                repo_path,
                &["fetch", remote, branch],
                git_ssh_command.as_deref(),
            )
            .await?;
        output.push_str(&format!("[git fetch] {}\n", fetch_output));

        // Reset to remote branch
        let reset_ref = format!("{}/{}", remote, branch);
        info!(ref_name = %reset_ref, "Resetting to remote branch");
        let reset_output = self
            .run_git_command(repo_path, &["reset", "--hard", &reset_ref], None)
            .await?;
        output.push_str(&format!("[git reset] {}\n", reset_output));

        // Clean untracked files
        let clean_output = self
            .run_git_command(repo_path, &["clean", "-fd"], None)
            .await?;
        output.push_str(&format!("[git clean] {}\n", clean_output));

        // Get current commit
        let commit = self
            .run_git_command(repo_path, &["rev-parse", "--short", "HEAD"], None)
            .await?;
        output.push_str(&format!("[commit] {}\n", commit.trim()));

        Ok(output)
    }

    /// Clone a repository
    #[allow(dead_code)]
    pub async fn clone(
        &self,
        url: &str,
        dest_path: &str,
        branch: Option<&str>,
        ssh_key: Option<&str>,
    ) -> Result<String, String> {
        let mut args = vec!["clone", "--depth", "1"];

        if let Some(b) = branch {
            args.push("-b");
            args.push(b);
        }

        args.push(url);
        args.push(dest_path);

        let git_ssh_command = ssh_key.map(|key| {
            format!(
                "ssh -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
                key
            )
        });

        self.run_git_command(".", &args, git_ssh_command.as_deref())
            .await
    }

    /// Get current branch name
    #[allow(dead_code)]
    pub async fn current_branch(&self, repo_path: &str) -> Result<String, String> {
        self.run_git_command(repo_path, &["rev-parse", "--abbrev-ref", "HEAD"], None)
            .await
            .map(|s| s.trim().to_string())
    }

    /// Get current commit SHA
    #[allow(dead_code)]
    pub async fn current_commit(&self, repo_path: &str) -> Result<String, String> {
        self.run_git_command(repo_path, &["rev-parse", "HEAD"], None)
            .await
            .map(|s| s.trim().to_string())
    }

    /// Check if repository has uncommitted changes
    #[allow(dead_code)]
    pub async fn has_changes(&self, repo_path: &str) -> Result<bool, String> {
        let output = self
            .run_git_command(repo_path, &["status", "--porcelain"], None)
            .await?;
        Ok(!output.trim().is_empty())
    }

    async fn run_git_command(
        &self,
        working_dir: &str,
        args: &[&str],
        git_ssh_command: Option<&str>,
    ) -> Result<String, String> {
        let mut cmd = Command::new("git");
        cmd.args(args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(ssh_cmd) = git_ssh_command {
            cmd.env("GIT_SSH_COMMAND", ssh_cmd);
        }

        debug!(args = ?args, "Running git command");

        let output = cmd
            .output()
            .await
            .map_err(|e| format!("Failed to execute git: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("{}{}", stdout, stderr))
        } else {
            Err(format!("Git command failed: {}\n{}", stderr, stdout))
        }
    }
}

impl Default for GitDeploy {
    fn default() -> Self {
        Self::new()
    }
}
