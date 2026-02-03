use anyhow::{Context, Result};
use log::debug;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::cmp::Ordering;

const GITHUB_API_URL: &str =
    "https://api.github.com/repos/WeekendSuperhero/dj-uploader/releases/latest";

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug)]
pub struct UpdateInfo {
    pub version: String,
    pub release_url: String,
    #[allow(dead_code)]
    pub dmg_download_url: Option<String>,
}

/// Compare two date-based versions like "2026.6.0" vs "2026.7.0".
/// Returns Ordering::Greater if `remote` is newer than `current`.
fn compare_versions(current: &str, remote: &str) -> Ordering {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .filter_map(|part| part.parse::<u64>().ok())
            .collect()
    };
    let current_parts = parse(current);
    let remote_parts = parse(remote);
    remote_parts.cmp(&current_parts)
}

/// Check if a newer version is available on GitHub Releases.
/// Returns `Some(UpdateInfo)` if an update is available, `None` otherwise.
pub fn check_for_update() -> Result<Option<UpdateInfo>> {
    let current_version = env!("CARGO_PKG_VERSION");

    debug!("Checking for updates (current: v{})...", current_version);

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let response = client
        .get(GITHUB_API_URL)
        .header("User-Agent", format!("dj-uploader/{}", current_version))
        .header("Accept", "application/vnd.github+json")
        .send()
        .context("Failed to check for updates")?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let release: GitHubRelease = response
        .json()
        .context("Failed to parse GitHub release response")?;

    let remote_version = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);

    if compare_versions(current_version, remote_version) == Ordering::Greater {
        let dmg_url = release
            .assets
            .iter()
            .find(|a| a.name.ends_with(".dmg"))
            .map(|a| a.browser_download_url.clone());

        debug!("Update available: v{}", remote_version);

        Ok(Some(UpdateInfo {
            version: remote_version.to_string(),
            release_url: release.html_url,
            dmg_download_url: dmg_url,
        }))
    } else {
        debug!("Already up to date");
        Ok(None)
    }
}

/// Open the GitHub release page in the default browser.
pub fn open_release_page(url: &str) {
    let _ = webbrowser::open(url);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions() {
        assert_eq!(compare_versions("2026.6.0", "2026.7.0"), Ordering::Greater);
        assert_eq!(compare_versions("2026.6.0", "2026.6.0"), Ordering::Equal);
        assert_eq!(compare_versions("2026.7.0", "2026.6.0"), Ordering::Less);
        assert_eq!(compare_versions("2026.6.0", "2027.1.0"), Ordering::Greater);
        assert_eq!(compare_versions("2026.6.0", "2025.12.0"), Ordering::Less);
        assert_eq!(compare_versions("2026.6.0", "2026.6.1"), Ordering::Greater);
    }
}
