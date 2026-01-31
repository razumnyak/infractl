use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::{debug, info};

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

    /// Run a shell command
    pub async fn run_command(
        &self,
        command: &str,
        working_dir: Option<&str>,
        env: &HashMap<String, String>,
    ) -> Result<String, String> {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);

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

    /// Run a script file
    pub async fn run_script(
        &self,
        script_path: &str,
        working_dir: Option<&str>,
        env: &HashMap<String, String>,
        run_as_user: Option<&str>,
    ) -> Result<String, String> {
        let command = if let Some(user) = run_as_user {
            format!("sudo -u {} bash {}", user, script_path)
        } else {
            format!("bash {}", script_path)
        };

        info!(script = %script_path, user = ?run_as_user, "Running script");

        self.run_command(&command, working_dir, env).await
    }

    /// Run multiple commands in sequence
    #[allow(dead_code)]
    pub async fn run_commands(
        &self,
        commands: &[String],
        working_dir: Option<&str>,
        env: &HashMap<String, String>,
    ) -> Result<String, String> {
        let mut output = String::new();

        for cmd in commands {
            let result = self.run_command(cmd, working_dir, env).await?;
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
