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
    /// Returns (output, has_changes) where has_changes indicates if commit changed
    pub async fn pull(
        &self,
        repo_path: &str,
        remote: &str,
        branch: &str,
        ssh_key: Option<&str>,
    ) -> Result<(String, bool), String> {
        let path = Path::new(repo_path);

        if !path.exists() {
            return Err(format!("Repository path does not exist: {}", repo_path));
        }

        let mut output = String::new();

        // Get commit hash BEFORE fetch
        let before_commit = self
            .run_git_command(repo_path, &["rev-parse", "HEAD"], None)
            .await
            .unwrap_or_default()
            .trim()
            .to_string();

        // Set up SSH command if key is provided
        let git_ssh_command = ssh_key.map(|key| {
            format!(
                "ssh -i '{}' -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=/dev/null",
                key.replace('\'', "'\\''")
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

        // Get commit hash AFTER reset
        let after_commit = self
            .run_git_command(repo_path, &["rev-parse", "HEAD"], None)
            .await?
            .trim()
            .to_string();

        let short_commit = self
            .run_git_command(repo_path, &["rev-parse", "--short", "HEAD"], None)
            .await?;
        output.push_str(&format!("[commit] {}\n", short_commit.trim()));

        let has_changes = before_commit != after_commit;
        if has_changes {
            output.push_str(&format!(
                "[changes] {} -> {}\n",
                &before_commit[..8.min(before_commit.len())],
                &after_commit[..8.min(after_commit.len())]
            ));
        } else {
            output.push_str("[no changes] already up to date\n");
        }

        Ok((output, has_changes))
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
                "ssh -i '{}' -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=/dev/null",
                key.replace('\'', "'\\''")
            )
        });

        self.run_git_command(".", &args, git_ssh_command.as_deref())
            .await
    }

    /// Fetch specific files from a git repository
    /// Uses shallow clone + copy (compatible with GitHub SSH)
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

        // Create secure temp directory (random name, auto-cleanup on drop)
        let temp_dir_handle =
            tempfile::tempdir().map_err(|e| format!("Failed to create temp directory: {}", e))?;
        let temp_dir = temp_dir_handle.path().to_string_lossy().to_string();

        let mut output = String::new();

        let source_paths: Vec<&str> = file_mappings
            .iter()
            .map(|(from, _)| from.as_str())
            .collect();

        info!(
            repo = %repo_url,
            branch = %branch,
            files = ?source_paths,
            "Fetching files from git (shallow clone)"
        );

        let git_ssh_command = ssh_key.map(|key| {
            format!(
                "ssh -i '{}' -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=/dev/null",
                key.replace('\'', "'\\''")
            )
        });

        // Shallow clone the repo
        let clone_result = self
            .run_git_command(
                ".",
                &["clone", "--depth", "1", "-b", branch, repo_url, &temp_dir],
                git_ssh_command.as_deref(),
            )
            .await;

        if let Err(e) = &clone_result {
            // temp_dir_handle drops here, auto-cleaning
            return Err(format!("git clone failed: {}", e));
        }

        output.push_str(&format!(
            "[git clone --depth 1] {}\n",
            clone_result.unwrap_or_default().trim()
        ));

        // Copy files according to mappings
        let base_path = Path::new(dest_path);
        for (from, to) in file_mappings {
            let src = Path::new(&temp_dir).join(from.trim_end_matches('/'));
            let dst_raw = base_path.join(to.trim_end_matches('/'));

            // Validate path containment (prevent path traversal attacks)
            let dst = validate_path_containment(base_path, &dst_raw)?;

            let is_dir = from.ends_with('/') || to.ends_with('/');

            if is_dir {
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
                // Parent directory already created by validate_path_containment
                if src.exists() {
                    fs::copy(&src, &dst)
                        .map_err(|e| format!("Failed to copy {} to {}: {}", from, to, e))?;
                    output.push_str(&format!("[copy] {} -> {}\n", from, to));
                } else {
                    return Err(format!("File not found in repo: {}", from));
                }
            }
        }

        // temp_dir_handle drops here, auto-cleaning
        Ok(output)
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

/// Validate that target path stays within base path (prevent path traversal)
fn validate_path_containment(
    base: &std::path::Path,
    target: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    use std::fs;

    let canonical_base = base
        .canonicalize()
        .map_err(|e| format!("Cannot resolve base path '{}': {}", base.display(), e))?;

    // For target: if it doesn't exist, canonicalize parent and append filename
    let canonical_target = if target.exists() {
        target.canonicalize()
    } else {
        let parent = target
            .parent()
            .ok_or_else(|| format!("Target path '{}' has no parent directory", target.display()))?;
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create parent directory: {}", e))?;
        parent
            .canonicalize()
            .map(|p| p.join(target.file_name().unwrap()))
    }
    .map_err(|e| format!("Cannot resolve target path '{}': {}", target.display(), e))?;

    if !canonical_target.starts_with(&canonical_base) {
        return Err(format!(
            "Path traversal detected: '{}' escapes base directory '{}'",
            target.display(),
            base.display()
        ));
    }

    Ok(canonical_target)
}
