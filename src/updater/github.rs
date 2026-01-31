use reqwest::Client;
use semver::Version;
use serde::Deserialize;
use tracing::{debug, info};

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Release {
    pub tag_name: String,
    pub name: String,
    pub prerelease: bool,
    pub draft: bool,
    pub published_at: String,
    pub body: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ReleaseAsset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
    pub content_type: String,
}

pub struct GitHubClient {
    client: Client,
    repo: String,
}

impl GitHubClient {
    pub fn new(repo: &str) -> Self {
        let client = Client::builder()
            .user_agent(format!("infractl/{}", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            client,
            repo: repo.to_string(),
        }
    }

    /// Fetch the latest release from GitHub
    pub async fn get_latest_release(&self, include_prerelease: bool) -> Result<Release, String> {
        if include_prerelease {
            // Get all releases and find the latest (including prereleases)
            let releases = self.get_releases(10).await?;
            releases
                .into_iter()
                .find(|r| !r.draft)
                .ok_or_else(|| "No releases found".to_string())
        } else {
            // Use the /latest endpoint which excludes prereleases
            let url = format!("https://api.github.com/repos/{}/releases/latest", self.repo);

            debug!(url = %url, "Fetching latest release");

            let response = self
                .client
                .get(&url)
                .header("Accept", "application/vnd.github.v3+json")
                .send()
                .await
                .map_err(|e| format!("Failed to fetch release: {}", e))?;

            if response.status() == 404 {
                return Err("No releases found".to_string());
            }

            if !response.status().is_success() {
                return Err(format!("GitHub API error: {}", response.status()));
            }

            response
                .json::<Release>()
                .await
                .map_err(|e| format!("Failed to parse release: {}", e))
        }
    }

    /// Fetch recent releases from GitHub
    pub async fn get_releases(&self, limit: usize) -> Result<Vec<Release>, String> {
        let url = format!(
            "https://api.github.com/repos/{}/releases?per_page={}",
            self.repo, limit
        );

        debug!(url = %url, "Fetching releases");

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch releases: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("GitHub API error: {}", response.status()));
        }

        response
            .json::<Vec<Release>>()
            .await
            .map_err(|e| format!("Failed to parse releases: {}", e))
    }

    /// Download a release asset
    pub async fn download_asset(&self, asset: &ReleaseAsset) -> Result<Vec<u8>, String> {
        info!(
            asset = %asset.name,
            size = asset.size,
            "Downloading release asset"
        );

        let response = self
            .client
            .get(&asset.browser_download_url)
            .header("Accept", "application/octet-stream")
            .send()
            .await
            .map_err(|e| format!("Failed to download asset: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Download failed: {}", response.status()));
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("Failed to read download: {}", e))
    }

    /// Download checksum file for a release
    pub async fn download_checksum(&self, release: &Release) -> Result<String, String> {
        // Look for common checksum file names
        let checksum_names: Vec<String> = vec![
            "SHA256SUMS".to_string(),
            "sha256sums.txt".to_string(),
            "checksums.txt".to_string(),
            format!("infractl-{}.sha256", release.tag_name),
        ];

        for name in &checksum_names {
            if let Some(asset) = release.assets.iter().find(|a| &a.name == name) {
                let data = self.download_asset(asset).await?;
                return String::from_utf8(data)
                    .map_err(|e| format!("Invalid checksum file encoding: {}", e));
            }
        }

        Err("No checksum file found in release".to_string())
    }

    /// Fetch raw file from GitHub (for config sync)
    #[allow(dead_code)]
    pub async fn fetch_raw_file(&self, url: &str) -> Result<String, String> {
        debug!(url = %url, "Fetching raw file");

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch file: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Fetch failed: {}", response.status()));
        }

        response
            .text()
            .await
            .map_err(|e| format!("Failed to read file: {}", e))
    }
}

impl Release {
    /// Parse version from tag name (strips 'v' prefix if present)
    #[allow(dead_code)]
    pub fn version(&self) -> Result<Version, String> {
        let version_str = self.tag_name.strip_prefix('v').unwrap_or(&self.tag_name);
        Version::parse(version_str).map_err(|e| format!("Invalid version '{}': {}", version_str, e))
    }

    /// Find binary asset for current platform
    pub fn find_binary_asset(&self) -> Option<&ReleaseAsset> {
        let target = get_target_triple();
        let binary_name = format!("infractl-{}", target);

        self.assets.iter().find(|a| {
            a.name == binary_name
                || a.name == format!("{}.tar.gz", binary_name)
                || a.name == format!("{}.zip", binary_name)
        })
    }
}

/// Check if a new version is available
pub fn is_newer_version(current: &str, remote: &str) -> Result<bool, String> {
    let current_ver = Version::parse(current.strip_prefix('v').unwrap_or(current))
        .map_err(|e| format!("Invalid current version '{}': {}", current, e))?;

    let remote_ver = Version::parse(remote.strip_prefix('v').unwrap_or(remote))
        .map_err(|e| format!("Invalid remote version '{}': {}", remote, e))?;

    Ok(remote_ver > current_ver)
}

/// Get the target triple for current platform
pub fn get_target_triple() -> String {
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };

    let os = if cfg!(target_os = "linux") {
        "unknown-linux-musl"
    } else if cfg!(target_os = "macos") {
        "apple-darwin"
    } else if cfg!(target_os = "windows") {
        "pc-windows-msvc"
    } else {
        "unknown"
    };

    format!("{}-{}", arch, os)
}

/// Parse checksum from SHA256SUMS format
pub fn parse_checksum(checksums: &str, filename: &str) -> Option<String> {
    for line in checksums.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let hash = parts[0];
            let file = parts.last().unwrap().trim_start_matches('*');
            if file == filename || file.ends_with(filename) {
                return Some(hash.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("0.1.0", "0.2.0").unwrap());
        assert!(is_newer_version("v0.1.0", "v0.2.0").unwrap());
        assert!(!is_newer_version("0.2.0", "0.1.0").unwrap());
        assert!(!is_newer_version("0.1.0", "0.1.0").unwrap());
    }

    #[test]
    fn test_parse_checksum() {
        let checksums = r#"abc123def456  infractl-x86_64-unknown-linux-musl
789xyz000111  infractl-aarch64-unknown-linux-musl"#;

        assert_eq!(
            parse_checksum(checksums, "infractl-x86_64-unknown-linux-musl"),
            Some("abc123def456".to_string())
        );
        assert_eq!(parse_checksum(checksums, "infractl-arm64"), None);
    }
}
