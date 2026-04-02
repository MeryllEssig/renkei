use std::fs;
use std::path::PathBuf;

use semver::Version;

use crate::config::Config;
use crate::error::{RenkeiError, Result};

const GITHUB_REPO: &str = "MeryllEssig/renkei";

pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("CARGO_PKG_VERSION is valid semver")
}

/// Fetch the latest stable release version from GitHub.
pub fn fetch_latest_version() -> Result<Version> {
    let api_url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let body: serde_json::Value = ureq::get(&api_url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "rk-self-update")
        .call()
        .map_err(|e| RenkeiError::SelfUpdateFailed(format!("GitHub API request failed: {e}")))?
        .body_mut()
        .read_json()
        .map_err(|e| RenkeiError::SelfUpdateFailed(format!("Failed to parse response: {e}")))?;

    let tag = body["tag_name"]
        .as_str()
        .ok_or_else(|| RenkeiError::SelfUpdateFailed("No tag_name in release".to_string()))?;

    let is_prerelease = body["prerelease"].as_bool().unwrap_or(false);
    if is_prerelease {
        return Err(RenkeiError::SelfUpdateFailed(
            "Latest release is a pre-release".to_string(),
        ));
    }

    let version_str = tag.strip_prefix('v').unwrap_or(tag);
    Version::parse(version_str)
        .map_err(|e| RenkeiError::SelfUpdateFailed(format!("Invalid version in tag '{tag}': {e}")))
}

/// Determine the artifact name for this platform.
pub fn artifact_name() -> Result<&'static str> {
    let name = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "rk-linux-x86_64",
        ("linux", "aarch64") => "rk-linux-aarch64",
        ("macos", "aarch64") => "rk-darwin-aarch64",
        ("windows", "x86_64") => "rk-windows-x86_64.exe",
        (os, arch) => {
            return Err(RenkeiError::SelfUpdateFailed(format!(
                "Unsupported platform: {os}/{arch}"
            )))
        }
    };
    Ok(name)
}

/// Build the download URL for a given version and artifact.
fn download_url(version: &Version, artifact: &str) -> String {
    format!("https://github.com/{GITHUB_REPO}/releases/download/v{version}/{artifact}")
}

/// Download the binary and replace the current executable.
pub fn perform_update(latest: &Version) -> Result<()> {
    let artifact = artifact_name()?;
    let url = download_url(latest, artifact);

    eprintln!("Downloading rk v{latest}...");

    let binary_data = ureq::get(&url)
        .header("User-Agent", "rk-self-update")
        .call()
        .map_err(|e| RenkeiError::SelfUpdateFailed(format!("Download failed: {e}")))?
        .into_body()
        .read_to_vec()
        .map_err(|e| RenkeiError::SelfUpdateFailed(format!("Failed to read binary: {e}")))?;

    let current_exe = std::env::current_exe()
        .map_err(|e| RenkeiError::SelfUpdateFailed(format!("Cannot locate current binary: {e}")))?;

    let tmp_path = current_exe.with_extension("tmp");
    fs::write(&tmp_path, &binary_data)
        .map_err(|e| RenkeiError::SelfUpdateFailed(format!("Failed to write temp file: {e}")))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o755)).map_err(|e| {
            RenkeiError::SelfUpdateFailed(format!("Failed to set permissions: {e}"))
        })?;
    }

    fs::rename(&tmp_path, &current_exe).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            RenkeiError::SelfUpdateFailed(format!(
                "Permission denied. Run with elevated privileges:\n  sudo rk self-update"
            ))
        } else {
            RenkeiError::SelfUpdateFailed(format!("Failed to replace binary: {e}"))
        }
    })?;

    Ok(())
}

/// Main entry point for `rk self-update`.
pub fn run_self_update() -> Result<()> {
    let current = current_version();
    let latest = fetch_latest_version()?;

    if current >= latest {
        if current > latest {
            eprintln!("Already up to date (v{current}, latest stable: v{latest})");
        } else {
            eprintln!("Already up to date (v{current})");
        }
        return Ok(());
    }

    eprintln!("Updating rk v{current} → v{latest}");
    perform_update(&latest)?;
    eprintln!("Updated to rk v{latest}");

    Ok(())
}

/// Return the path to the version check cache file.
pub fn cache_path() -> PathBuf {
    Config::with_home_dir(Config::default_home_dir())
        .renkei_dir()
        .join("last_version_check.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_version_is_valid() {
        let v = current_version();
        assert!(v.major >= 1);
    }

    #[test]
    fn test_artifact_name_returns_value() {
        // Should not panic on the current platform
        let name = artifact_name();
        assert!(name.is_ok());
        assert!(name.unwrap().starts_with("rk-"));
    }

    #[test]
    fn test_download_url_format() {
        let v = Version::new(1, 2, 3);
        let url = download_url(&v, "rk-darwin-aarch64");
        assert_eq!(
            url,
            "https://github.com/MeryllEssig/renkei/releases/download/v1.2.3/rk-darwin-aarch64"
        );
    }

    #[test]
    fn test_cache_path_under_renkei_dir() {
        let path = cache_path();
        assert!(path.to_string_lossy().contains(".renkei"));
        assert!(path.to_string_lossy().ends_with("last_version_check.json"));
    }
}
