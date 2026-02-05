use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::{debug, info, warn};

/// Validate command for dangerous shell metacharacters
/// Returns Ok(()) if command is safe, Err with reason if not
fn validate_command(cmd: &str) -> Result<(), String> {
    // Block shell metacharacters that enable command injection
    let dangerous_patterns = [
        ("$(", "command substitution"),
        ("`", "backtick command substitution"),
        ("&&", "command chaining"),
        ("||", "conditional execution"),
        (";", "command separator"),
        ("|", "pipe"),
        (">>", "append redirect"),
        (">&", "file descriptor redirect"),
        ("<(", "process substitution"),
        (">(", "process substitution"),
    ];

    for (pattern, description) in dangerous_patterns {
        if cmd.contains(pattern) {
            return Err(format!(
                "Command contains forbidden pattern '{}' ({})",
                pattern, description
            ));
        }
    }

    // Allow single > for output redirect but warn
    if cmd.contains('>') && !cmd.contains(">>") && !cmd.contains(">&") {
        warn!(command = %cmd, "Command contains output redirect, ensure this is intentional");
    }

    Ok(())
}

pub struct ScriptRunner {
    default_timeout: Duration,
}

impl ScriptRunner {
    pub fn new() -> Self {
        Self {
            default_timeout: Duration::from_secs(300), // 5 minutes default
        }
    }

    #[allow(dead_code)]
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            default_timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Run a shell command (optionally as specified user via sudo)
    ///
    /// # Security
    /// Commands are validated against dangerous shell metacharacters
    /// to prevent command injection attacks.
    pub async fn run_command(
        &self,
        command: &str,
        working_dir: Option<&str>,
        env: &HashMap<String, String>,
        run_as_user: Option<&str>,
    ) -> Result<String, String> {
        // Validate command for injection attacks
        validate_command(command)?;

        let mut cmd = if let Some(user) = run_as_user {
            let mut c = Command::new("sudo");
            c.args(["-u", user, "sh", "-c", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.arg("-c").arg(command);
            c
        };

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        // Add environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        debug!(command = %command, "Running shell command");

        let output = timeout(self.default_timeout, cmd.output())
            .await
            .map_err(|_| format!("Command timed out after {:?}", self.default_timeout))?
            .map_err(|e| format!("Failed to execute command: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("{}{}", stdout, stderr))
        } else {
            Err(format!(
                "Command failed with exit code {}: {}\n{}",
                output.status.code().unwrap_or(-1),
                stderr,
                stdout
            ))
        }
    }

    /// Run a script file (executes directly, not via shell, to prevent injection)
    pub async fn run_script(
        &self,
        script_path: &str,
        working_dir: Option<&str>,
        env: &HashMap<String, String>,
        run_as_user: Option<&str>,
    ) -> Result<String, String> {
        info!(script = %script_path, user = ?run_as_user, "Running script");

        let mut cmd = if let Some(user) = run_as_user {
            let mut c = Command::new("sudo");
            c.args(["-u", user, "bash", script_path]);
            c
        } else {
            let mut c = Command::new("bash");
            c.arg(script_path);
            c
        };

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        for (key, value) in env {
            cmd.env(key, value);
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let output = timeout(self.default_timeout, cmd.output())
            .await
            .map_err(|_| format!("Script timed out after {:?}", self.default_timeout))?
            .map_err(|e| format!("Failed to execute script: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("{}{}", stdout, stderr))
        } else {
            Err(format!(
                "Script failed with exit code {}: {}\n{}",
                output.status.code().unwrap_or(-1),
                stderr,
                stdout
            ))
        }
    }

    /// Run multiple commands in sequence
    #[allow(dead_code)]
    pub async fn run_commands(
        &self,
        commands: &[String],
        working_dir: Option<&str>,
        env: &HashMap<String, String>,
        run_as_user: Option<&str>,
    ) -> Result<String, String> {
        let mut output = String::new();

        for cmd in commands {
            let result = self.run_command(cmd, working_dir, env, run_as_user).await?;
            output.push_str(&format!("$ {}\n{}\n", cmd, result));
        }

        Ok(output)
    }
}

impl Default for ScriptRunner {
    fn default() -> Self {
        Self::new()
    }
}
