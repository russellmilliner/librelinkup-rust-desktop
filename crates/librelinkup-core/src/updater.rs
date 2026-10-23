//! Update checker against GitHub Releases (`russellmilliner/librelinkup-rust-desktop`).

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const GITHUB_RELEASES_LATEST_URL: &str =
    "https://api.github.com/repos/russellmilliner/librelinkup-rust-desktop/releases/latest";
pub const GITHUB_RELEASES_PAGE_URL: &str =
    "https://github.com/russellmilliner/librelinkup-rust-desktop/releases";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub checked: bool,
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub download_url: String,
    pub release_notes: String,
    pub message: String,
}

/// Parses a version string such as `"v0.4.1"` or `"0.4.1-rc1"` into `(major, minor, patch)`.
pub fn parse_semver(version: &str) -> Option<(u32, u32, u32)> {
    let cleaned = version
        .trim()
        .trim_start_matches(['v', 'V'])
        .split(['-', '+'])
        .next()?
        .trim();
    if cleaned.is_empty() {
        return None;
    }
    let mut parts = cleaned.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u32>().ok()?;
    Some((major, minor, patch))
}

/// Normalizes a version string to `"vX.Y.Z"` display form.
pub fn normalize_version_tag(version: &str) -> String {
    let trimmed = version.trim().trim_start_matches(['v', 'V']);
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("v{trimmed}")
    }
}

/// Returns `true` if `latest` is strictly newer than `current`.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    match (parse_semver(current), parse_semver(latest)) {
        (Some(cur), Some(lat)) => lat > cur,
        _ => false,
    }
}

/// Parses a GitHub Releases `/releases/latest` JSON payload into an [`UpdateCheckResult`].
pub fn parse_github_release_json(
    raw_json: &str,
    current_version: &str,
) -> Result<UpdateCheckResult, String> {
    let val: Value = serde_json::from_str(raw_json)
        .map_err(|e| format!("Invalid GitHub release JSON: {e}"))?;

    let tag_name = val
        .get("tag_name")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "GitHub release response is missing 'tag_name'.".to_string())?;

    let release_url = val
        .get("html_url")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(GITHUB_RELEASES_PAGE_URL)
        .to_string();

    let release_notes = val
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let mut flatpak_url = None;
    let mut windows_installer_url = None;
    if let Some(assets) = val.get("assets").and_then(|v| v.as_array()) {
        for asset in assets {
            let name = asset
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let url = asset
                .get("browser_download_url")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if url.is_empty() || name.ends_with(".sha256") {
                continue;
            }
            if name.ends_with(".flatpak") && flatpak_url.is_none() {
                flatpak_url = Some(url.to_string());
            } else if name.ends_with(".exe") && windows_installer_url.is_none() {
                windows_installer_url = Some(url.to_string());
            }
        }
    }

    let preferred_asset = if cfg!(target_os = "windows") {
        windows_installer_url.or(flatpak_url)
    } else {
        flatpak_url.or(windows_installer_url)
    };
    let download_url = preferred_asset.unwrap_or_else(|| release_url.clone());

    let cur_norm = normalize_version_tag(current_version);
    let lat_norm = normalize_version_tag(tag_name);
    let update_available = is_newer_version(&cur_norm, &lat_norm);

    let message = if update_available {
        format!("Update available: {lat_norm} (you have {cur_norm})")
    } else {
        format!("Up to date ({cur_norm})")
    };

    Ok(UpdateCheckResult {
        checked: true,
        update_available,
        current_version: cur_norm,
        latest_version: lat_norm,
        release_url,
        download_url,
        release_notes,
        message,
    })
}

fn github_headers(current_version: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let ua = format!(
        "librelinkup-rust-desktop/{}",
        current_version.trim().trim_start_matches(['v', 'V'])
    );
    if let Ok(hv) = HeaderValue::from_str(&ua) {
        headers.insert(USER_AGENT, hv);
    } else {
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(concat!(
                "librelinkup-rust-desktop/",
                env!("CARGO_PKG_VERSION")
            )),
        );
    }
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    headers
}

#[cfg(unix)]
fn curl_get_github_release(url: &str, current_version: &str) -> Result<String, String> {
    let ua = format!(
        "User-Agent: librelinkup-rust-desktop/{}",
        current_version.trim().trim_start_matches(['v', 'V'])
    );
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "10",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            &ua,
            url,
        ])
        .output()
        .map_err(|e| format!("Failed to invoke curl for update check: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "curl exited with status {}",
            out.status.code().unwrap_or(-1)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Checks GitHub Releases asynchronously for a newer version of LibreLinkUp Rust Desktop.
pub async fn check_for_updates_async(current_version: &str) -> Result<UpdateCheckResult, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .default_headers(github_headers(current_version))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    match client.get(GITHUB_RELEASES_LATEST_URL).send().await {
        Ok(resp) => {
            let status = resp.status();
            let text = resp
                .text()
                .await
                .map_err(|e| format!("Failed reading update response: {e}"))?;
            if !status.is_success() {
                return Err(format!("GitHub API returned HTTP {}", status.as_u16()));
            }
            parse_github_release_json(&text, current_version)
        }
        Err(err) => {
            #[cfg(unix)]
            {
                let cur = current_version.to_string();
                if let Ok(Ok(raw)) = tokio::task::spawn_blocking(move || {
                    curl_get_github_release(GITHUB_RELEASES_LATEST_URL, &cur)
                })
                .await
                {
                    return parse_github_release_json(&raw, current_version);
                }
            }
            Err(format!("Update check failed: {err}"))
        }
    }
}

/// Checks GitHub Releases synchronously for a newer version.
pub fn check_for_updates_blocking(current_version: &str) -> Result<UpdateCheckResult, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .default_headers(github_headers(current_version))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    match client.get(GITHUB_RELEASES_LATEST_URL).send() {
        Ok(resp) => {
            let status = resp.status();
            let text = resp
                .text()
                .map_err(|e| format!("Failed reading update response: {e}"))?;
            if !status.is_success() {
                return Err(format!("GitHub API returned HTTP {}", status.as_u16()));
            }
            parse_github_release_json(&text, current_version)
        }
        Err(err) => {
            #[cfg(unix)]
            if let Ok(raw) = curl_get_github_release(GITHUB_RELEASES_LATEST_URL, current_version) {
                return parse_github_release_json(&raw, current_version);
            }
            Err(format!("Update check failed: {err}"))
        }
    }
}

/// Downloads the platform release package (`.exe` installer on Windows or `.flatpak` bundle on Linux)
/// and launches the upgrade process.
pub async fn download_and_install_update_async(
    download_url: &str,
    latest_version: &str,
) -> Result<PathBuf, String> {
    if download_url.trim().is_empty()
        || (!download_url.ends_with(".exe") && !download_url.ends_with(".flatpak"))
    {
        return Err("Direct installer asset URL is not available; please use View Release.".into());
    }

    let clean_ver = latest_version.trim().trim_start_matches(['v', 'V']);
    let file_name = if cfg!(target_os = "windows") {
        format!("LibreLinkUp-Setup-{clean_ver}-x64.exe")
    } else {
        format!("librelinkup-rust-desktop-v{clean_ver}-x86_64.flatpak")
    };

    let dest_path = std::env::temp_dir().join(&file_name);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .default_headers(github_headers(env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let mut downloaded = false;
    if let Ok(resp) = client.get(download_url).send().await {
        if resp.status().is_success() {
            if let Ok(bytes) = resp.bytes().await {
                if bytes.len() > 1024 {
                    std::fs::write(&dest_path, &bytes)
                        .map_err(|e| format!("Failed writing update package: {e}"))?;
                    downloaded = true;
                }
            }
        }
    }

    #[cfg(unix)]
    if !downloaded {
        let url_owned = download_url.to_string();
        let dest_clone = dest_path.clone();
        tokio::task::spawn_blocking(move || {
            let status = Command::new("curl")
                .args([
                    "-fsSL",
                    "--max-time",
                    "180",
                    "-o",
                    &dest_clone.to_string_lossy(),
                    &url_owned,
                ])
                .status()
                .map_err(|e| format!("Failed to invoke curl to download update: {e}"))?;
            if !status.success() {
                return Err("curl failed to download update package".to_string());
            }
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| format!("Download task join error: {e}"))??;
        downloaded = dest_path.is_file();
    }

    if !downloaded || !dest_path.is_file() {
        return Err(format!("Failed to download update from {download_url}"));
    }

    #[cfg(target_os = "windows")]
    {
        Command::new(&dest_path)
            .args(["/SILENT", "/CLOSEAPPLICATIONS", "/RESTARTAPPLICATIONS"])
            .spawn()
            .map_err(|e| format!("Failed to launch Windows update installer: {e}"))?;
        Ok(dest_path)
    }

    #[cfg(unix)]
    {
        let dest_clone = dest_path.clone();
        tokio::task::spawn_blocking(move || {
            let in_flatpak = std::path::Path::new("/.flatpak-info").exists();
            if in_flatpak {
                // Inside Flatpak sandbox: copy bundle to ~/.cache so host flatpak can read it,
                // then invoke flatpak-spawn --host to install and relaunch.
                let host_cache = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join(".cache");
                let _ = std::fs::create_dir_all(&host_cache);
                let shared_bundle = host_cache.join(
                    dest_clone
                        .file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new("update.flatpak")),
                );
                if dest_clone != shared_bundle {
                    let _ = std::fs::copy(&dest_clone, &shared_bundle);
                }
                let script = format!(
                    "flatpak install --user -y --reinstall '{}' && (flatpak kill io.github.russellmilliner.LibreLinkUpDesktop 2>/dev/null || true) && nohup flatpak run io.github.russellmilliner.LibreLinkUpDesktop >/dev/null 2>&1 &",
                    shared_bundle.display()
                );
                Command::new("flatpak-spawn")
                    .args(["--host", "sh", "-c", &script])
                    .spawn()
                    .map_err(|e| format!("Failed to invoke flatpak-spawn updater: {e}"))?;
                Ok(shared_bundle)
            } else {
                let status = Command::new("flatpak")
                    .args([
                        "install",
                        "--user",
                        "-y",
                        "--reinstall",
                        &dest_clone.to_string_lossy(),
                    ])
                    .status()
                    .map_err(|e| format!("Failed to run flatpak install: {e}"))?;
                if !status.success() {
                    return Err("flatpak install returned non-zero status".into());
                }
                Ok(dest_clone)
            }
        })
        .await
        .map_err(|e| format!("Install task join error: {e}"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_compares_semver() {
        assert_eq!(parse_semver("v0.4.0"), Some((0, 4, 0)));
        assert_eq!(parse_semver("0.4.0"), Some((0, 4, 0)));
        assert_eq!(parse_semver("V1.2.3-beta.1"), Some((1, 2, 3)));
        assert!(is_newer_version("0.4.0", "v0.4.1"));
        assert!(is_newer_version("v0.4.0", "v0.5.0"));
        assert!(is_newer_version("v0.4.0", "v1.0.0"));
        assert!(!is_newer_version("v0.4.0", "v0.4.0"));
        assert!(!is_newer_version("v0.4.0", "v0.3.9"));
    }

    #[test]
    fn parses_github_release_payload() {
        let sample = r####"{
            "tag_name": "v0.5.0",
            "html_url": "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.0",
            "body": "### Highlights\n- New update checker",
            "assets": [
                {
                    "name": "LibreLinkUp-Setup-0.5.0-x64.exe",
                    "browser_download_url": "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.5.0/LibreLinkUp-Setup-0.5.0-x64.exe"
                },
                {
                    "name": "librelinkup-rust-desktop-v0.5.0-x86_64.flatpak",
                    "browser_download_url": "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.5.0/librelinkup-rust-desktop-v0.5.0-x86_64.flatpak"
                }
            ]
        }"####;

        let res = parse_github_release_json(sample, "0.4.1").expect("valid release JSON");
        assert!(res.checked);
        assert!(res.update_available);
        assert_eq!(res.current_version, "v0.4.1");
        assert_eq!(res.latest_version, "v0.5.0");
        assert_eq!(
            res.release_url,
            "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.0"
        );
        assert!(res.message.contains("v0.5.0"));

        let up_to_date = parse_github_release_json(sample, "0.5.0").expect("valid release JSON");
        assert!(!up_to_date.update_available);
        assert_eq!(up_to_date.message, "Up to date (v0.5.0)");
    }
}
