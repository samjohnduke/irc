//! Self-update functionality for irc-server.

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde::Deserialize;

const REPO: &str = "samjohnduke/irc";
const GITHUB_API: &str = "https://api.github.com/repos";

/// Current version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// Update result.
#[derive(Debug)]
pub enum UpdateResult {
    Updated {
        from: String,
        to: String,
        backup: PathBuf,
    },
    UpToDate {
        version: String,
    },
    Error(String),
}

/// Check for updates.
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

    let release = match fetch_latest_release().await {
        Ok(r) => r,
        Err(e) => return UpdateResult::Error(e),
    };

    let latest_version = release.tag_name.trim_start_matches('v');

    if !is_newer(latest_version, VERSION) {
        return UpdateResult::UpToDate {
            version: VERSION.to_string(),
        };
    }

    eprintln!("New version available: {} -> {}", VERSION, latest_version);

    let asset_name = match get_asset_name() {
        Ok(name) => name,
        Err(e) => return UpdateResult::Error(e),
    };

    let asset = match release.assets.iter().find(|a| a.name == asset_name) {
        Some(a) => a,
        None => {
            return UpdateResult::Error(format!(
                "No binary found for this platform: {}",
                asset_name
            ));
        }
    };

    eprintln!("Downloading {}...", asset_name);

    let temp_path = match download_binary(&asset.browser_download_url).await {
        Ok(p) => p,
        Err(e) => return UpdateResult::Error(e),
    };

    let current_exe = match env::current_exe() {
        Ok(p) => p,
        Err(e) => return UpdateResult::Error(format!("Cannot find current executable: {}", e)),
    };

    let backup_path = current_exe.with_extension("backup");

    #[cfg(windows)]
    let backup_path = {
        let mut p = current_exe.clone();
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        p.set_file_name(format!("{}.backup.exe", name));
        p
    };

    eprintln!("Backing up current version to: {}", backup_path.display());

    if backup_path.exists() {
        if let Err(e) = fs::remove_file(&backup_path) {
            return UpdateResult::Error(format!("Cannot remove old backup: {}", e));
        }
    }

    if let Err(e) = fs::rename(&current_exe, &backup_path) {
        return UpdateResult::Error(format!("Cannot create backup: {}", e));
    }

    if let Err(e) = fs::rename(&temp_path, &current_exe) {
        let _ = fs::rename(&backup_path, &current_exe);
        return UpdateResult::Error(format!("Cannot install new binary: {}", e));
    }

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

async fn fetch_latest_release() -> Result<Release, String> {
    let url = format!("{}/{}/releases/latest", GITHUB_API, REPO);

    let client = reqwest::Client::builder()
        .user_agent(format!("irc-server/{}", VERSION))
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

async fn download_binary(url: &str) -> Result<PathBuf, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("irc-server/{}", VERSION))
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

    let temp_dir = env::temp_dir();
    let temp_path = temp_dir.join("irc-server-update");

    let mut file =
        fs::File::create(&temp_path).map_err(|e| format!("Cannot create temp file: {}", e))?;

    file.write_all(&bytes)
        .map_err(|e| format!("Cannot write temp file: {}", e))?;

    Ok(temp_path)
}

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

    let ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };

    Ok(format!("irc-server-{}-{}{}", os, arch, ext))
}

fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> { v.split('.').filter_map(|s| s.parse().ok()).collect() };

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

    latest_parts.len() > current_parts.len()
}
