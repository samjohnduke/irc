//! Self-update functionality.
//!
//! Downloads the latest release from GitHub and replaces the current binary,
//! keeping the previous version as a backup.

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde::Deserialize;

const REPO: &str = "samjohnduke/irc";
const GITHUB_API: &str = "https://api.github.com/repos";

/// Current version from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Release info from GitHub API.
#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

/// Asset info from GitHub API.
#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// Update result.
#[derive(Debug)]
pub enum UpdateResult {
    /// Updated successfully.
    Updated { from: String, to: String, backup: PathBuf },
    /// Already up to date.
    UpToDate { version: String },
    /// Error occurred.
    Error(String),
}

/// Check for available updates.
pub async fn check_update() -> Result<Option<String>, String> {
    let latest = fetch_latest_release().await?;
    let latest_version = latest.tag_name.trim_start_matches('v');

    if is_newer(latest_version, VERSION) {
        Ok(Some(latest.tag_name))
    } else {
        Ok(None)
    }
}

/// Perform self-update.
pub async fn update() -> UpdateResult {
    eprintln!("Checking for updates...");

    // Fetch latest release info
    let release = match fetch_latest_release().await {
        Ok(r) => r,
        Err(e) => return UpdateResult::Error(e),
    };

    let latest_version = release.tag_name.trim_start_matches('v');

    // Check if update needed
    if !is_newer(latest_version, VERSION) {
        return UpdateResult::UpToDate {
            version: VERSION.to_string(),
        };
    }

    eprintln!("New version available: {} -> {}", VERSION, latest_version);

    // Determine asset name for current platform
    let asset_name = match get_asset_name() {
        Ok(name) => name,
        Err(e) => return UpdateResult::Error(e),
    };

    // Find the asset URL
    let asset = match release.assets.iter().find(|a| a.name == asset_name) {
        Some(a) => a,
        None => {
            return UpdateResult::Error(format!(
                "No binary found for this platform: {}",
                asset_name
            ))
        }
    };

    eprintln!("Downloading {}...", asset_name);

    // Download to temp file
    let temp_path = match download_binary(&asset.browser_download_url).await {
        Ok(p) => p,
        Err(e) => return UpdateResult::Error(e),
    };

    // Get current executable path
    let current_exe = match env::current_exe() {
        Ok(p) => p,
        Err(e) => return UpdateResult::Error(format!("Cannot find current executable: {}", e)),
    };

    // Create backup path
    let backup_path = current_exe.with_extension("backup");

    // On Windows, we need different extension handling
    #[cfg(windows)]
    let backup_path = {
        let mut p = current_exe.clone();
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        p.set_file_name(format!("{}.backup.exe", name));
        p
    };

    eprintln!("Backing up current version to: {}", backup_path.display());

    // Remove old backup if exists
    if backup_path.exists() {
        if let Err(e) = fs::remove_file(&backup_path) {
            return UpdateResult::Error(format!("Cannot remove old backup: {}", e));
        }
    }

    // Rename current to backup
    if let Err(e) = fs::rename(&current_exe, &backup_path) {
        return UpdateResult::Error(format!("Cannot create backup: {}", e));
    }

    // Move new binary into place
    if let Err(e) = fs::rename(&temp_path, &current_exe) {
        // Try to restore backup
        let _ = fs::rename(&backup_path, &current_exe);
        return UpdateResult::Error(format!("Cannot install new binary: {}", e));
    }

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(&current_exe, fs::Permissions::from_mode(0o755)) {
            eprintln!("Warning: Could not set permissions: {}", e);
        }
    }

    UpdateResult::Updated {
        from: VERSION.to_string(),
        to: release.tag_name,
        backup: backup_path,
    }
}

/// Fetch latest release from GitHub.
async fn fetch_latest_release() -> Result<Release, String> {
    let url = format!("{}/{}/releases/latest", GITHUB_API, REPO);

    let client = reqwest::Client::builder()
        .user_agent(format!("irc-cli/{}", VERSION))
        .build()
        .map_err(|e| format!("Cannot create HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Cannot fetch release info: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("GitHub API error: {}", response.status()));
    }

    response
        .json::<Release>()
        .await
        .map_err(|e| format!("Cannot parse release info: {}", e))
}

/// Download binary to temp file.
async fn download_binary(url: &str) -> Result<PathBuf, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("irc-cli/{}", VERSION))
        .build()
        .map_err(|e| format!("Cannot create HTTP client: {}", e))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Cannot download binary: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Download failed: {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Cannot read download: {}", e))?;

    // Write to temp file
    let temp_dir = env::temp_dir();
    let temp_path = temp_dir.join("irc-update");

    let mut file = fs::File::create(&temp_path)
        .map_err(|e| format!("Cannot create temp file: {}", e))?;

    file.write_all(&bytes)
        .map_err(|e| format!("Cannot write temp file: {}", e))?;

    Ok(temp_path)
}

/// Get the asset name for the current platform.
fn get_asset_name() -> Result<String, String> {
    let os = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        return Err("Unsupported operating system".to_string());
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return Err("Unsupported architecture".to_string());
    };

    let ext = if cfg!(target_os = "windows") { ".exe" } else { "" };

    Ok(format!("irc-{}-{}{}", os, arch, ext))
}

/// Compare versions (simple semver comparison).
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };

    let latest_parts = parse(latest);
    let current_parts = parse(current);

    for (l, c) in latest_parts.iter().zip(current_parts.iter()) {
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }

    // If we get here with equal lengths, they're the same
    // If latest has more parts, it's newer (e.g., 1.0.1 > 1.0)
    latest_parts.len() > current_parts.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("1.1.0", "1.0.0"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(is_newer("1.0.1", "1.0"));

        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.1"));
        assert!(!is_newer("0.9.0", "1.0.0"));
    }
}
