use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, error, info, warn};

/// Result of a binary update operation
#[derive(Debug)]
#[allow(dead_code)]
pub struct UpdateResult {
    pub success: bool,
    pub from_version: String,
    pub to_version: String,
    pub message: String,
    pub requires_restart: bool,
}

/// Self-updater for the binary
pub struct BinaryUpdater {
    current_exe: PathBuf,
    backup_dir: PathBuf,
}

impl BinaryUpdater {
    pub fn new() -> Result<Self, String> {
        let current_exe = std::env::current_exe()
            .map_err(|e| format!("Failed to get current executable path: {}", e))?;

        let backup_dir = current_exe
            .parent()
            .map(|p| p.join(".infractl-backup"))
            .unwrap_or_else(|| PathBuf::from("/tmp/.infractl-backup"));

        Ok(Self {
            current_exe,
            backup_dir,
        })
    }

    /// Verify checksum of downloaded binary
    pub fn verify_checksum(data: &[u8], expected: &str) -> Result<(), String> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let computed = hex::encode(hasher.finalize());

        debug!(expected = %expected, computed = %computed, "Verifying checksum");

        if computed == expected.to_lowercase() {
            Ok(())
        } else {
            Err(format!(
                "Checksum mismatch: expected {}, got {}",
                expected, computed
            ))
        }
    }

    /// Compute SHA256 checksum of data
    #[allow(dead_code)]
    pub fn compute_checksum(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// Perform atomic binary replacement
    pub fn replace_binary(
        &self,
        new_binary: &[u8],
        from_version: &str,
        to_version: &str,
    ) -> Result<UpdateResult, String> {
        info!(
            from = %from_version,
            to = %to_version,
            "Starting binary replacement"
        );

        // Create backup directory
        fs::create_dir_all(&self.backup_dir)
            .map_err(|e| format!("Failed to create backup directory: {}", e))?;

        // Create backup of current binary
        let now = time::OffsetDateTime::now_utc();
        let timestamp = format!(
            "{:04}{:02}{:02}{:02}{:02}{:02}",
            now.year(), now.month() as u8, now.day(), now.hour(), now.minute(), now.second()
        );
        let backup_path = self.backup_dir.join(format!(
            "infractl-{}-{}",
            from_version,
            timestamp
        ));

        fs::copy(&self.current_exe, &backup_path)
            .map_err(|e| format!("Failed to backup current binary: {}", e))?;

        info!(backup = %backup_path.display(), "Created backup");

        // Write new binary to temp file
        let temp_path = self.current_exe.with_extension("new");

        let mut temp_file =
            File::create(&temp_path).map_err(|e| format!("Failed to create temp file: {}", e))?;

        temp_file
            .write_all(new_binary)
            .map_err(|e| format!("Failed to write new binary: {}", e))?;

        temp_file
            .sync_all()
            .map_err(|e| format!("Failed to sync temp file: {}", e))?;

        drop(temp_file);

        // Set executable permissions
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&temp_path)
                .map_err(|e| format!("Failed to get temp file metadata: {}", e))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&temp_path, perms)
                .map_err(|e| format!("Failed to set executable permissions: {}", e))?;
        }

        // Atomic rename
        fs::rename(&temp_path, &self.current_exe).map_err(|e| {
            // Try to restore backup on failure
            if let Err(restore_err) = fs::copy(&backup_path, &self.current_exe) {
                error!(error = %restore_err, "Failed to restore backup after update failure");
            }
            format!("Failed to replace binary: {}", e)
        })?;

        info!(
            new_version = %to_version,
            "Binary updated successfully"
        );

        // Cleanup old backups (keep last 3)
        self.cleanup_backups(3);

        Ok(UpdateResult {
            success: true,
            from_version: from_version.to_string(),
            to_version: to_version.to_string(),
            message: "Binary updated successfully".to_string(),
            requires_restart: true,
        })
    }

    /// Restore from backup
    #[allow(dead_code)]
    pub fn restore_backup(&self, version: &str) -> Result<(), String> {
        let backup_files: Vec<_> = fs::read_dir(&self.backup_dir)
            .map_err(|e| format!("Failed to read backup directory: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .contains(&format!("infractl-{}", version))
            })
            .collect();

        let backup_file = backup_files
            .last()
            .ok_or_else(|| format!("No backup found for version {}", version))?;

        fs::copy(backup_file.path(), &self.current_exe)
            .map_err(|e| format!("Failed to restore backup: {}", e))?;

        info!(
            version = %version,
            "Restored from backup"
        );

        Ok(())
    }

    /// List available backups
    #[allow(dead_code)]
    pub fn list_backups(&self) -> Result<Vec<String>, String> {
        if !self.backup_dir.exists() {
            return Ok(Vec::new());
        }

        let backups = fs::read_dir(&self.backup_dir)
            .map_err(|e| format!("Failed to read backup directory: {}", e))?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();

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
                debug!(path = %path.display(), "Removed old backup");
            }
        }
    }
}

impl Default for BinaryUpdater {
    fn default() -> Self {
        Self::new().expect("Failed to initialize binary updater")
    }
}

/// Signal systemd to restart the service
pub fn signal_systemd_restart() -> Result<(), String> {
    // Check if running under systemd
    if std::env::var("INVOCATION_ID").is_ok() || std::env::var("NOTIFY_SOCKET").is_ok() {
        info!("Running under systemd, signaling restart");

        // Use systemctl to restart
        let service_name = "infractl";

        match Command::new("systemctl")
            .args(["restart", service_name])
            .status()
        {
            Ok(status) if status.success() => {
                info!("Systemd restart signal sent");
                Ok(())
            }
            Ok(status) => {
                warn!(
                    code = ?status.code(),
                    "systemctl restart returned non-zero"
                );
                // Fall back to self-restart
                self_restart()
            }
            Err(e) => {
                warn!(error = %e, "Failed to run systemctl");
                // Fall back to self-restart
                self_restart()
            }
        }
    } else {
        self_restart()
    }
}

/// Restart the current process
fn self_restart() -> Result<(), String> {
    info!("Performing self-restart");

    let exe =
        std::env::current_exe().map_err(|e| format!("Failed to get current executable: {}", e))?;

    let args: Vec<String> = std::env::args().skip(1).collect();

    // Spawn the new process
    match Command::new(&exe).args(&args).spawn() {
        Ok(child) => {
            info!(pid = child.id(), "New process spawned");
            // Exit current process
            std::process::exit(0);
        }
        Err(e) => Err(format!("Failed to spawn new process: {}", e)),
    }
}

/// Extract binary from tar.gz archive
pub fn extract_from_tarball(data: &[u8], binary_name: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;

    let decoder = flate2::read::GzDecoder::new(data);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read tar entries: {}", e))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read tar entry: {}", e))?;

        let path = entry
            .path()
            .map_err(|e| format!("Failed to get entry path: {}", e))?;

        if path.file_name().map(|n| n.to_string_lossy()).as_deref() == Some(binary_name) {
            let mut contents = Vec::new();
            entry
                .read_to_end(&mut contents)
                .map_err(|e| format!("Failed to read entry contents: {}", e))?;
            return Ok(contents);
        }
    }

    Err(format!("Binary '{}' not found in archive", binary_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_checksum() {
        let data = b"test data";
        let expected = "916f0027a575074ce72a331777c3478d6513f786a591bd892da1a577bf2335f9";

        assert!(BinaryUpdater::verify_checksum(data, expected).is_ok());
        assert!(BinaryUpdater::verify_checksum(data, "wrong").is_err());
    }

    #[test]
    fn test_compute_checksum() {
        let data = b"test data";
        let checksum = BinaryUpdater::compute_checksum(data);
        assert_eq!(
            checksum,
            "916f0027a575074ce72a331777c3478d6513f786a591bd892da1a577bf2335f9"
        );
    }
}
