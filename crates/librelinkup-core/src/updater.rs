//! Update checker against GitHub Releases (`russellmilliner/librelinkup-rust-desktop`).

#[cfg(unix)]
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
