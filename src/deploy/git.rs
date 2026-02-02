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

    /// Fetch specific files from a git repository
    /// Uses git archive to download only the specified files
    /// file_mappings format: [(from_path, to_path)] where trailing / means directory
    pub async fn fetch_files(
        &self,
        repo_url: &str,
        branch: &str,
        file_mappings: &[(String, String)],
        dest_path: &str,
        ssh_key: Option<&str>,
    ) -> Result<String, String> {
        use std::fs;
        use std::path::Path;

        // Create destination directory
        fs::create_dir_all(dest_path)
            .map_err(|e| format!("Failed to create directory {}: {}", dest_path, e))?;

        // Create temp directory for extraction
        let temp_dir = format!("{}/._git_temp_{}", dest_path, std::process::id());
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp directory: {}", e))?;

        let mut output = String::new();

        // Collect source paths for git archive
        let source_paths: Vec<&str> = file_mappings
            .iter()
            .map(|(from, _)| from.as_str())
            .collect();

        info!(
            repo = %repo_url,
            branch = %branch,
            files = ?source_paths,
            "Fetching files from git"
        );

        // Build git archive command
        // git archive --remote=<repo> <branch> <paths...> | tar -x -C <temp>
        let paths_str = source_paths.join(" ");
        let archive_cmd = format!(
            "git archive --remote={} {} {} | tar -x -C {}",
            repo_url, branch, paths_str, temp_dir
        );

        let git_ssh_command = ssh_key.map(|key| {
            format!(
                "ssh -i {} -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=/dev/null",
                key
            )
        });

        // Run archive command
        let archive_result = self
            .run_shell_command(&archive_cmd, ".", git_ssh_command.as_deref())
            .await;

        if let Err(e) = &archive_result {
            // Cleanup temp dir on error
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(e.clone());
        }

        output.push_str(&format!(
            "[git archive] {}\n",
            archive_result.unwrap_or_default()
        ));

        // Copy files according to mappings
        for (from, to) in file_mappings {
            let src = Path::new(&temp_dir).join(from.trim_end_matches('/'));
            let dst = Path::new(dest_path).join(to.trim_end_matches('/'));

            let is_dir = from.ends_with('/') || to.ends_with('/');

            if is_dir {
                // Copy directory recursively
                if src.is_dir() {
                    fs::create_dir_all(&dst)
                        .map_err(|e| format!("Failed to create dir {}: {}", dst.display(), e))?;

                    copy_dir_recursive(&src, &dst)
                        .map_err(|e| format!("Failed to copy directory: {}", e))?;

                    output.push_str(&format!("[copy] {}/ -> {}/\n", from, to));
                } else {
                    return Err(format!("Expected directory but found file: {}", from));
                }
            } else {
                // Copy single file
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create parent dir: {}", e))?;
                }

                if src.exists() {
                    fs::copy(&src, &dst)
                        .map_err(|e| format!("Failed to copy {} to {}: {}", from, to, e))?;
                    output.push_str(&format!("[copy] {} -> {}\n", from, to));
                } else {
                    return Err(format!("File not found in archive: {}", from));
                }
            }
        }

        // Cleanup temp directory
        let _ = fs::remove_dir_all(&temp_dir);

        Ok(output)
    }

    async fn run_shell_command(
        &self,
        command: &str,
        working_dir: &str,
        git_ssh_command: Option<&str>,
    ) -> Result<String, String> {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(command)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(ssh_cmd) = git_ssh_command {
            cmd.env("GIT_SSH_COMMAND", ssh_cmd);
        }

        debug!(command = %command, "Running shell command");

        let output = cmd
            .output()
            .await
            .map_err(|e| format!("Failed to execute command: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("{}{}", stdout, stderr))
        } else {
            Err(format!("Command failed: {}\n{}", stderr, stdout))
        }
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

/// Recursively copy directory contents
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    use std::fs;

    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
