pub mod binary;
pub mod config_sync;
pub mod github;

use crate::config::UpdatesConfig;
use binary::{BinaryUpdater, UpdateResult};
use config_sync::{ConfigSync, ConfigSyncResult};
use github::{is_newer_version, GitHubClient};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// State of the auto-updater
#[derive(Debug, Clone, Default)]
pub struct UpdaterState {
    pub last_check: Option<time::OffsetDateTime>,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub last_error: Option<String>,
}

/// Main updater orchestrator
#[allow(dead_code)]
pub struct Updater {
    config: UpdatesConfig,
    config_path: String,
    github_client: Option<GitHubClient>,
    binary_updater: Option<BinaryUpdater>,
    config_sync: Option<ConfigSync>,
    state: Arc<RwLock<UpdaterState>>,
}

impl Updater {
    pub fn new(updates_config: &UpdatesConfig, config_path: &str) -> Self {
        let github_client = if !updates_config.self_update.github_repo.is_empty() {
            Some(GitHubClient::new(&updates_config.self_update.github_repo))
        } else {
            None
        };

        let binary_updater = if updates_config.self_update.enabled {
            match BinaryUpdater::new() {
                Ok(u) => Some(u),
                Err(e) => {
                    warn!(error = %e, "Failed to initialize binary updater");
                    None
                }
            }
        } else {
            None
        };

        let config_sync = if updates_config.config_update.enabled {
            let sync = ConfigSync::new(Path::new(config_path))
                .with_github_url(&updates_config.config_update.github_raw_url);
            Some(sync)
        } else {
            None
        };

        Self {
            config: updates_config.clone(),
            config_path: config_path.to_string(),
            github_client,
            binary_updater,
            config_sync,
            state: Arc::new(RwLock::new(UpdaterState::default())),
        }
    }

    /// Check for available updates
    pub async fn check_for_updates(&self) -> Result<bool, String> {
        if !self.config.self_update.enabled {
            return Ok(false);
        }

        let client = self
            .github_client
            .as_ref()
            .ok_or("GitHub client not configured")?;

        info!("Checking for updates...");

        let release = client
            .get_latest_release(self.config.self_update.prerelease)
            .await?;

        let current_version = env!("CARGO_PKG_VERSION");
        let remote_version = release.tag_name.clone();

        let update_available = is_newer_version(current_version, &remote_version)?;

        // Update state
        {
            let mut state = self.state.write().await;
            state.last_check = Some(time::OffsetDateTime::now_utc());
            state.latest_version = Some(remote_version.clone());
            state.update_available = update_available;
            state.last_error = None;
        }

        if update_available {
            info!(
                current = %current_version,
                latest = %remote_version,
                "Update available"
            );
        } else {
            debug!(
                current = %current_version,
                latest = %remote_version,
                "Already up to date"
            );
        }

        Ok(update_available)
    }

    /// Perform self-update
    pub async fn self_update(&self) -> Result<UpdateResult, String> {
        if !self.config.self_update.enabled {
            return Err("Self-update is disabled".to_string());
        }

        let client = self
            .github_client
            .as_ref()
            .ok_or("GitHub client not configured")?;

        let updater = self
            .binary_updater
            .as_ref()
            .ok_or("Binary updater not initialized")?;

        info!("Starting self-update...");

        // Get latest release
        let release = client
            .get_latest_release(self.config.self_update.prerelease)
            .await?;

        let current_version = env!("CARGO_PKG_VERSION");
        let remote_version = release.tag_name.clone();

        // Check if update is needed
        if !is_newer_version(current_version, &remote_version)? {
            return Ok(UpdateResult {
                success: true,
                from_version: current_version.to_string(),
                to_version: remote_version,
                message: "Already up to date".to_string(),
                requires_restart: false,
            });
        }

        // Find binary asset
        let asset = release
            .find_binary_asset()
            .ok_or("No compatible binary found in release")?;

        info!(
            asset = %asset.name,
            size = asset.size,
            "Found binary asset"
        );

        // Download binary
        let binary_data = client.download_asset(asset).await?;

        // Try to get checksum
        let checksum_result = client.download_checksum(&release).await;

        // Handle compressed archives
        let final_binary = if asset.name.ends_with(".tar.gz") {
            binary::extract_from_tarball(&binary_data, "infractl")?
        } else if asset.name.ends_with(".zip") {
            return Err("ZIP archives not yet supported".to_string());
        } else {
            binary_data
        };

        // Verify checksum if available
        if let Ok(checksums) = checksum_result {
            if let Some(expected) = github::parse_checksum(&checksums, &asset.name) {
                BinaryUpdater::verify_checksum(&final_binary, &expected)?;
                info!("Checksum verified successfully");
            } else {
                warn!(
                    "Checksum file found but no matching entry for {}",
                    asset.name
                );
            }
        } else {
            warn!("No checksum file available, skipping verification");
        }

        // Perform update
        let result = updater.replace_binary(&final_binary, current_version, &remote_version)?;

        // Update state
        {
            let mut state = self.state.write().await;
            state.update_available = false;
            state.latest_version = Some(remote_version);
        }

        Ok(result)
    }

    /// Check for config changes
    pub async fn check_config_changes(&self) -> Result<bool, String> {
        if !self.config.config_update.enabled {
            return Ok(false);
        }

        let sync = self
            .config_sync
            .as_ref()
            .ok_or("Config sync not configured")?;

        sync.check_for_changes().await
    }

    /// Sync config from remote
    pub async fn sync_config(&self) -> Result<ConfigSyncResult, String> {
        if !self.config.config_update.enabled {
            return Err("Config sync is disabled".to_string());
        }

        let sync = self
            .config_sync
            .as_ref()
            .ok_or("Config sync not configured")?;

        sync.sync(self.config.config_update.backup).await
    }

    /// Get current state
    #[allow(dead_code)]
    pub async fn get_state(&self) -> UpdaterState {
        self.state.read().await.clone()
    }
}

/// Parse duration string (e.g., "6h", "30m", "1d")
pub fn parse_duration(s: &str) -> Result<std::time::Duration, String> {
    humantime::parse_duration(s).map_err(|e| format!("Invalid duration '{}': {}", s, e))
}

/// Start background update checker task
pub async fn start_update_checker(
    updater: Arc<Updater>,
    check_interval: std::time::Duration,
    auto_update: bool,
) {
    info!(
        interval = ?check_interval,
        auto_update = auto_update,
        "Starting update checker"
    );

    loop {
        tokio::time::sleep(check_interval).await;

        match updater.check_for_updates().await {
            Ok(available) => {
                if available && auto_update {
                    info!("Update available, starting automatic update");
                    match updater.self_update().await {
                        Ok(result) => {
                            info!(
                                from = %result.from_version,
                                to = %result.to_version,
                                "Update completed"
                            );
                            if result.requires_restart {
                                if let Err(e) = binary::signal_systemd_restart() {
                                    error!(error = %e, "Failed to restart after update");
                                }
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Auto-update failed");
                        }
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Update check failed");
                // Update state with error
                let mut state = updater.state.write().await;
                state.last_error = Some(e);
            }
        }
    }
}

/// Start background config sync task
pub async fn start_config_sync(updater: Arc<Updater>, check_interval: std::time::Duration) {
    info!(
        interval = ?check_interval,
        "Starting config sync checker"
    );

    loop {
        tokio::time::sleep(check_interval).await;

        match updater.check_config_changes().await {
            Ok(changed) => {
                if changed {
                    info!("Config changes detected, syncing...");
                    match updater.sync_config().await {
                        Ok(result) => {
                            if result.changed {
                                info!(
                                    backup = ?result.backup_path,
                                    "Config synced successfully"
                                );
                                // Signal for config reload
                                // This would typically send SIGHUP to the process
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Config sync failed");
                        }
                    }
                }
            }
            Err(e) => {
                debug!(error = %e, "Config check skipped");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(
            parse_duration("6h").unwrap(),
            std::time::Duration::from_secs(6 * 60 * 60)
        );
        assert_eq!(
            parse_duration("30m").unwrap(),
            std::time::Duration::from_secs(30 * 60)
        );
        assert_eq!(
            parse_duration("1d").unwrap(),
            std::time::Duration::from_secs(24 * 60 * 60)
        );
    }
}
