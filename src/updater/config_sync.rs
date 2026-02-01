use super::github::GitHubClient;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Result of a config sync operation
#[derive(Debug)]
#[allow(dead_code)]
pub struct ConfigSyncResult {
    pub changed: bool,
    pub backup_path: Option<PathBuf>,
    pub message: String,
}

/// Config synchronizer
pub struct ConfigSync {
    config_path: PathBuf,
    backup_dir: PathBuf,
    github_client: Option<GitHubClient>,
    raw_url: Option<String>,
}

impl ConfigSync {
    pub fn new(config_path: &Path) -> Self {
        let backup_dir = config_path
            .parent()
            .map(|p| p.join(".config-backup"))
            .unwrap_or_else(|| PathBuf::from("/etc/infractl/.config-backup"));

        Self {
            config_path: config_path.to_path_buf(),
            backup_dir,
            github_client: None,
            raw_url: None,
        }
    }

    /// Configure GitHub raw file URL for config sync
    pub fn with_github_url(mut self, url: &str) -> Self {
        if !url.is_empty() {
            self.raw_url = Some(url.to_string());

            // Extract repo from URL if it's a raw.githubusercontent.com URL
            if url.contains("raw.githubusercontent.com") {
                let parts: Vec<&str> = url.split('/').collect();
                if parts.len() >= 5 {
                    let repo = format!("{}/{}", parts[3], parts[4]);
                    self.github_client = Some(GitHubClient::new(&repo));
                }
            }
        }
        self
    }

    /// Check if remote config differs from local
    pub async fn check_for_changes(&self) -> Result<bool, String> {
        let raw_url = self
            .raw_url
            .as_ref()
            .ok_or("No remote config URL configured")?;

        let remote_content = self.fetch_remote_config(raw_url).await?;
        let local_content = self.read_local_config()?;

        let remote_hash = compute_hash(&remote_content);
        let local_hash = compute_hash(&local_content);

        debug!(
            remote_hash = %remote_hash,
            local_hash = %local_hash,
            "Comparing config hashes"
        );

        Ok(remote_hash != local_hash)
    }

    /// Fetch remote config content
    async fn fetch_remote_config(&self, url: &str) -> Result<String, String> {
        let client = reqwest::Client::builder()
            .user_agent(format!("infractl/{}", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        debug!(url = %url, "Fetching remote config");

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch remote config: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Fetch failed: {}", response.status()));
        }

        response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))
    }

    /// Read local config file
    fn read_local_config(&self) -> Result<String, String> {
        fs::read_to_string(&self.config_path)
            .map_err(|e| format!("Failed to read local config: {}", e))
    }

    /// Sync config from remote
    pub async fn sync(&self, backup: bool) -> Result<ConfigSyncResult, String> {
        let raw_url = self
            .raw_url
            .as_ref()
            .ok_or("No remote config URL configured")?;

        let remote_content = self.fetch_remote_config(raw_url).await?;
        let local_content = self.read_local_config()?;

        // Check if different
        let remote_hash = compute_hash(&remote_content);
        let local_hash = compute_hash(&local_content);

        if remote_hash == local_hash {
            return Ok(ConfigSyncResult {
                changed: false,
                backup_path: None,
                message: "Config is already up to date".to_string(),
            });
        }

        // Validate remote config before applying
        self.validate_config(&remote_content)?;

        // Create backup if requested
        let backup_path = if backup {
            Some(self.create_backup()?)
        } else {
            None
        };

        // Write new config
        let mut file = File::create(&self.config_path)
            .map_err(|e| format!("Failed to create config file: {}", e))?;

        file.write_all(remote_content.as_bytes())
            .map_err(|e| format!("Failed to write config: {}", e))?;

        file.sync_all()
            .map_err(|e| format!("Failed to sync config file: {}", e))?;

        info!(
            remote_hash = %remote_hash,
            "Config synced successfully"
        );

        Ok(ConfigSyncResult {
            changed: true,
            backup_path,
            message: "Config synced successfully".to_string(),
        })
    }

    /// Validate config content
    fn validate_config(&self, content: &str) -> Result<(), String> {
        // Try to parse as YAML
        let _: serde_yaml::Value = serde_yaml::from_str(content)
            .map_err(|e| format!("Invalid YAML in remote config: {}", e))?;

        // Basic sanity checks
        if content.is_empty() {
            return Err("Remote config is empty".to_string());
        }

        if content.len() < 50 {
            return Err("Remote config seems too small".to_string());
        }

        // Check for required fields
        if !content.contains("mode:") {
            return Err("Remote config missing 'mode' field".to_string());
        }

        Ok(())
    }

    /// Create backup of current config
    fn create_backup(&self) -> Result<PathBuf, String> {
        fs::create_dir_all(&self.backup_dir)
            .map_err(|e| format!("Failed to create backup directory: {}", e))?;

        let now = time::OffsetDateTime::now_utc();
        let timestamp = format!(
            "{:04}{:02}{:02}{:02}{:02}{:02}",
            now.year(),
            now.month() as u8,
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        );
        let backup_path = self.backup_dir.join(format!("config-{}.yaml", timestamp));

        fs::copy(&self.config_path, &backup_path)
            .map_err(|e| format!("Failed to create backup: {}", e))?;

        info!(path = %backup_path.display(), "Created config backup");

        // Cleanup old backups (keep last 5)
        self.cleanup_backups(5);

        Ok(backup_path)
    }

    /// Restore config from backup
    #[allow(dead_code)]
    pub fn restore_backup(&self, backup_path: &Path) -> Result<(), String> {
        if !backup_path.exists() {
            return Err(format!("Backup file not found: {}", backup_path.display()));
        }

        fs::copy(backup_path, &self.config_path)
            .map_err(|e| format!("Failed to restore backup: {}", e))?;

        info!(
            backup = %backup_path.display(),
            "Config restored from backup"
        );

        Ok(())
    }

    /// List available config backups
    #[allow(dead_code)]
    pub fn list_backups(&self) -> Result<Vec<PathBuf>, String> {
        if !self.backup_dir.exists() {
            return Ok(Vec::new());
        }

        let mut backups: Vec<PathBuf> = fs::read_dir(&self.backup_dir)
            .map_err(|e| format!("Failed to read backup directory: {}", e))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
            .collect();

        backups.sort();
        backups.reverse();

        Ok(backups)
    }

    /// Cleanup old backups, keeping the most recent n
    fn cleanup_backups(&self, keep: usize) {
        if !self.backup_dir.exists() {
            return;
        }

        let mut backups: Vec<_> = fs::read_dir(&self.backup_dir)
            .ok()
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let metadata = e.metadata().ok()?;
                        let modified = metadata.modified().ok()?;
                        Some((e.path(), modified))
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Sort by modification time (newest first)
        backups.sort_by(|a, b| b.1.cmp(&a.1));

        // Remove old backups
        for (path, _) in backups.into_iter().skip(keep) {
            if let Err(e) = fs::remove_file(&path) {
                warn!(path = %path.display(), error = %e, "Failed to remove old backup");
            } else {
                debug!(path = %path.display(), "Removed old config backup");
            }
        }
    }

    /// Compute diff between local and remote config
    #[allow(dead_code)]
    pub async fn diff(&self) -> Result<String, String> {
        let raw_url = self
            .raw_url
            .as_ref()
            .ok_or("No remote config URL configured")?;

        let remote_content = self.fetch_remote_config(raw_url).await?;
        let local_content = self.read_local_config()?;

        // Simple line-by-line diff
        let local_lines: Vec<&str> = local_content.lines().collect();
        let remote_lines: Vec<&str> = remote_content.lines().collect();

        let mut diff = String::new();
        let max_lines = local_lines.len().max(remote_lines.len());

        for i in 0..max_lines {
            let local = local_lines.get(i).copied().unwrap_or("");
            let remote = remote_lines.get(i).copied().unwrap_or("");

            if local != remote {
                if !local.is_empty() {
                    diff.push_str(&format!("- {}\n", local));
                }
                if !remote.is_empty() {
                    diff.push_str(&format!("+ {}\n", remote));
                }
            }
        }

        if diff.is_empty() {
            Ok("No differences found".to_string())
        } else {
            Ok(diff)
        }
    }
}

/// Compute SHA256 hash of content
fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash() {
        let hash1 = compute_hash("test content");
        let hash2 = compute_hash("test content");
        let hash3 = compute_hash("different content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
