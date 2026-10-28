//! Update checker against GitHub Releases (`russellmilliner/librelinkup-rust-desktop`).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const GITHUB_RELEASES_LATEST_URL: &str =
    "https://api.github.com/repos/russellmilliner/librelinkup-rust-desktop/releases/latest";
pub const GITHUB_RELEASES_PAGE_URL: &str =
    "https://github.com/russellmilliner/librelinkup-rust-desktop/releases";
pub const GITHUB_RELEASE_DOWNLOAD_PREFIX: &str =
    "/russellmilliner/librelinkup-rust-desktop/releases/download/";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub checked: bool,
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub download_url: String,
    #[serde(default)]
    pub sha256_url: String,
    pub release_notes: String,
    pub message: String,
}

/// Validates that `url` is an HTTPS URL on `github.com` under
/// `/russellmilliner/librelinkup-rust-desktop/releases/download/<tag>/<file>`
/// with no userinfo, custom port, query string, fragment, or path traversal.
pub fn is_trusted_github_asset_url(url: &str) -> bool {
    let trimmed = url.trim();
    let Ok(parsed) = reqwest::Url::parse(trimmed) else {
        return false;
    };
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return false;
    }
    let path = parsed.path();
    let Some(rest) = path.strip_prefix(GITHUB_RELEASE_DOWNLOAD_PREFIX) else {
        return false;
    };
    let segments: Vec<&str> = rest.split('/').collect();
    if segments.len() != 2
        || segments
            .iter()
            .any(|s| s.is_empty() || *s == "." || *s == "..")
    {
        return false;
    }
    true
}

/// Validates that `url` is an HTTPS GitHub release page URL for this repository.
pub fn is_trusted_github_release_url(url: &str) -> bool {
    let trimmed = url.trim();
    let Ok(parsed) = reqwest::Url::parse(trimmed) else {
        return false;
    };
    parsed.scheme() == "https"
        && parsed.host_str() == Some("github.com")
        && parsed.port().is_none()
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && (parsed.path() == "/russellmilliner/librelinkup-rust-desktop/releases"
            || parsed
                .path()
                .starts_with("/russellmilliner/librelinkup-rust-desktop/releases/"))
        && !parsed.path().split('/').any(|s| s == "..")
}

/// Computes the lowercase 64-character hex SHA-256 digest of `bytes`.
pub fn compute_sha256_hex(bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    let mut hex = String::with_capacity(64);
    for b in digest.as_ref() {
        let _ = write!(&mut hex, "{b:02x}");
    }
    hex
}

/// Parses a `.sha256` checksum file (`<64-hex>  <filename>` or `<64-hex>`) and returns
/// the lowercase 64-character hex SHA-256 digest. When `expected_file_name` is provided
/// and a filename column is present in the `.sha256` file, the filename must match.
pub fn parse_sha256_checksum(
    raw: &str,
    expected_file_name: Option<&str>,
) -> Result<String, String> {
    let cleaned = raw.trim_start_matches('\u{feff}');
    let mut saw_any_line = false;
    for line in cleaned.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        saw_any_line = true;
        let mut parts = line.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Invalid SHA-256 digest format in .sha256 file.".into());
        }
        if let Some(file_col) = parts.next() {
            let clean_file = file_col
                .trim_start_matches('*')
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or("");
            if let Some(expected) = expected_file_name {
                if !clean_file.is_empty() && clean_file != expected {
                    continue;
                }
            }
        }
        return Ok(hash.to_ascii_lowercase());
    }
    if saw_any_line {
        Err("No matching file entry found in .sha256 checksum file.".into())
    } else {
        Err("Empty .sha256 checksum file.".into())
    }
}

/// Verifies that the SHA-256 digest of `bytes` matches `expected_hex`.
pub fn verify_sha256_bytes(bytes: &[u8], expected_hex: &str) -> Result<(), String> {
    let expected = expected_hex.trim();
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid expected SHA-256 hex digest.".into());
    }
    let actual = compute_sha256_hex(bytes);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "SHA-256 checksum mismatch (expected {}, got {})",
            expected.to_ascii_lowercase(),
            actual
        ));
    }
    Ok(())
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

    let (lat_major, lat_minor, lat_patch) = parse_semver(tag_name)
        .ok_or_else(|| format!("GitHub release has invalid semver tag_name: '{tag_name}'"))?;
    let lat_norm = format!("v{lat_major}.{lat_minor}.{lat_patch}");

    let release_url = val
        .get("html_url")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| is_trusted_github_release_url(s))
        .unwrap_or(GITHUB_RELEASES_PAGE_URL)
        .to_string();

    let release_notes = val
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let mut flatpak_asset: Option<(String, String)> = None;
    let mut windows_installer_asset: Option<(String, String)> = None;
    let mut sha256_assets: HashMap<String, String> = HashMap::new();

    if let Some(assets) = val.get("assets").and_then(|v| v.as_array()) {
        for asset in assets {
            let name = asset
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .unwrap_or("");
            let url = asset
                .get("browser_download_url")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .unwrap_or("");
            if name.is_empty() || !is_trusted_github_asset_url(url) {
                continue;
            }
            if name.ends_with(".sha256") {
                sha256_assets.insert(name.to_string(), url.to_string());
            } else if name.ends_with(".flatpak") && flatpak_asset.is_none() {
                flatpak_asset = Some((name.to_string(), url.to_string()));
            } else if name.ends_with(".exe") && windows_installer_asset.is_none() {
                windows_installer_asset = Some((name.to_string(), url.to_string()));
            }
        }
    }

    let preferred_asset = if cfg!(target_os = "windows") {
        windows_installer_asset.or(flatpak_asset)
    } else {
        flatpak_asset.or(windows_installer_asset)
    };
    let (download_url, sha256_url) = match preferred_asset {
        Some((asset_name, asset_url)) => {
            let sha_name = format!("{asset_name}.sha256");
            let sha_url = sha256_assets
                .get(&sha_name)
                .cloned()
                .unwrap_or_else(|| format!("{asset_url}.sha256"));
            (asset_url, sha_url)
        }
        None => (release_url.clone(), String::new()),
    };

    let cur_norm = normalize_version_tag(current_version);
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
        sha256_url,
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

static SHARED_UPDATE_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static SHARED_DOWNLOAD_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn shared_update_client() -> Result<&'static reqwest::Client, String> {
    if let Some(c) = SHARED_UPDATE_CLIENT.get() {
        return Ok(c);
    }
    let built = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .default_headers(github_headers(env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;
    let _ = SHARED_UPDATE_CLIENT.set(built);
    SHARED_UPDATE_CLIENT
        .get()
        .ok_or_else(|| "Failed initializing update HTTP client".to_string())
}

fn shared_download_client() -> Result<&'static reqwest::Client, String> {
    if let Some(c) = SHARED_DOWNLOAD_CLIENT.get() {
        return Ok(c);
    }
    let built = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .default_headers(github_headers(env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;
    let _ = SHARED_DOWNLOAD_CLIENT.set(built);
    SHARED_DOWNLOAD_CLIENT
        .get()
        .ok_or_else(|| "Failed initializing download HTTP client".to_string())
}

/// Checks GitHub Releases asynchronously for a newer version of LibreLinkUp Rust Desktop.
pub async fn check_for_updates_async(current_version: &str) -> Result<UpdateCheckResult, String> {
    let client = shared_update_client()?;

    match client
        .get(GITHUB_RELEASES_LATEST_URL)
        .headers(github_headers(current_version))
        .send()
        .await
    {
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

/// Downloads the platform release package (`.exe` installer on Windows or `.flatpak` bundle on Linux),
/// verifies its `.sha256` checksum against the official GitHub Release asset, and launches the upgrade.
pub async fn download_and_install_update_async(
    download_url: &str,
    latest_version: &str,
) -> Result<PathBuf, String> {
    let download_url = download_url.trim();
    if download_url.is_empty()
        || (!download_url.ends_with(".exe") && !download_url.ends_with(".flatpak"))
    {
        return Err("Direct installer asset URL is not available; please use View Release.".into());
    }
    if !is_trusted_github_asset_url(download_url) {
        return Err(format!(
            "Refusing to download update from untrusted URL: {download_url}"
        ));
    }
    #[cfg(target_os = "windows")]
    if !download_url.ends_with(".exe") {
        return Err("Direct Windows installer (.exe) asset URL is required on Windows.".into());
    }
    #[cfg(not(target_os = "windows"))]
    if !download_url.ends_with(".flatpak") {
        return Err("Direct Flatpak bundle (.flatpak) asset URL is required on Linux.".into());
    }

    // Strictly parse and reconstruct clean_ver from numeric (major, minor, patch) components
    // so version strings can never inject path traversal or shell characters.
    let (major, minor, patch) = parse_semver(latest_version).ok_or_else(|| {
        format!("Refusing update with invalid version tag: '{latest_version}'")
    })?;
    let clean_ver = format!("{major}.{minor}.{patch}");

    let asset_file_name = download_url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Invalid asset filename in download URL.".to_string())?;
    let sha256_url = format!("{download_url}.sha256");
    if !is_trusted_github_asset_url(&sha256_url) {
        return Err("Invalid SHA-256 checksum URL.".into());
    }

    let file_name = if cfg!(target_os = "windows") {
        format!("LibreLinkUp-Setup-{clean_ver}-x64.exe")
    } else {
        format!("librelinkup-rust-desktop-v{clean_ver}-x86_64.flatpak")
    };

    let dest_dir = crate::paths::data_dir();
    let dest_path = dest_dir.join(&file_name);

    let client = shared_download_client()?;

    // 1. Fetch and parse the mandatory .sha256 checksum file before trusting the installer.
    let mut sha256_text: Option<String> = None;
    if let Ok(resp) = client.get(&sha256_url).send().await {
        if resp.status().is_success() {
            if let Ok(text) = resp.text().await {
                if !text.trim().is_empty() {
                    sha256_text = Some(text);
                }
            }
        }
    }
    #[cfg(unix)]
    if sha256_text.is_none() {
        let sha_url_owned = sha256_url.clone();
        if let Ok(Ok(raw)) = tokio::task::spawn_blocking(move || {
            curl_get_github_release(&sha_url_owned, env!("CARGO_PKG_VERSION"))
        })
        .await
        {
            if !raw.trim().is_empty() {
                sha256_text = Some(raw);
            }
        }
    }
    let sha256_raw = sha256_text.ok_or_else(|| {
        format!("Refusing to install update: could not fetch SHA-256 checksum from {sha256_url}")
    })?;
    let expected_sha256 = parse_sha256_checksum(&sha256_raw, Some(asset_file_name))?;

    // 2. Download the installer/bundle and verify its SHA-256 digest before execution.
    let mut verified_bytes: Option<Vec<u8>> = None;
    if let Ok(resp) = client.get(download_url).send().await {
        if resp.status().is_success() {
            if let Ok(bytes) = resp.bytes().await {
                if bytes.len() > 1024 {
                    verified_bytes = Some(bytes.to_vec());
                }
            }
        }
    }

    #[cfg(unix)]
    if verified_bytes.is_none() {
        let url_owned = download_url.to_string();
        let (tmp_dl_path, tmp_dl_file) =
            crate::paths::create_private_temp_file(&dest_dir, ".llu-update-", ".part")?;
        drop(tmp_dl_file);
        let tmp_clone = tmp_dl_path.clone();
        let dl_res = tokio::task::spawn_blocking(move || {
            let status = Command::new("curl")
                .args(["-fsSL", "--max-time", "180", "-o"])
                .arg(&tmp_clone)
                .arg(&url_owned)
                .status()
                .map_err(|e| format!("Failed to invoke curl to download update: {e}"))?;
            if !status.success() {
                return Err("curl failed to download update package".to_string());
            }
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| format!("Download task join error: {e}"));
        if let Err(e) = dl_res.and_then(|r| r) {
            let _ = std::fs::remove_file(&tmp_dl_path);
            return Err(e);
        }
        let bytes = std::fs::read(&tmp_dl_path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp_dl_path);
            format!("Failed reading downloaded update package: {e}")
        })?;
        let _ = std::fs::remove_file(&tmp_dl_path);
        if let Err(err) = verify_sha256_bytes(&bytes, &expected_sha256) {
            return Err(format!("Refusing to install update: {err}"));
        }
        verified_bytes = Some(bytes);
    }

    let Some(bytes) = verified_bytes else {
        return Err(format!("Failed to download update from {download_url}"));
    };

    if let Err(err) = verify_sha256_bytes(&bytes, &expected_sha256) {
        let _ = std::fs::remove_file(&dest_path);
        return Err(format!("Refusing to install update: {err}"));
    }

    crate::paths::write_file_atomic_0600(&dest_path, &bytes)
        .map_err(|e| format!("Failed writing verified update package: {e}"))?;

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
                // Inside Flatpak sandbox: dest_clone is in paths::data_dir()
                // ($XDG_DATA_HOME/librelinkup-kde), which exists at the same path on the host.
                // Invoke flatpak install directly via flatpak-spawn without sh -c interpolation.
                let out = Command::new("flatpak-spawn")
                    .args([
                        "--host",
                        "flatpak",
                        "install",
                        "--user",
                        "-y",
                        "--reinstall",
                        "--",
                    ])
                    .arg(&dest_clone)
                    .output()
                    .map_err(|e| format!("Failed to invoke flatpak-spawn updater: {e}"))?;
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(format!(
                        "Flatpak update install failed (status {}): {}",
                        out.status.code().unwrap_or(-1),
                        stderr.trim()
                    ));
                }
                let _ = Command::new("flatpak-spawn")
                    .args([
                        "--host",
                        "sh",
                        "-c",
                        "(flatpak kill io.github.russellmilliner.LibreLinkUpDesktop 2>/dev/null || true) && nohup flatpak run io.github.russellmilliner.LibreLinkUpDesktop >/dev/null 2>&1 &",
                    ])
                    .spawn();
                Ok(dest_clone)
            } else {
                let status = Command::new("flatpak")
                    .args(["install", "--user", "-y", "--reinstall", "--"])
                    .arg(&dest_clone)
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
                    "name": "LibreLinkUp-Setup-0.5.0-x64.exe.sha256",
                    "browser_download_url": "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.5.0/LibreLinkUp-Setup-0.5.0-x64.exe.sha256"
                },
                {
                    "name": "librelinkup-rust-desktop-v0.5.0-x86_64.flatpak",
                    "browser_download_url": "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.5.0/librelinkup-rust-desktop-v0.5.0-x86_64.flatpak"
                },
                {
                    "name": "librelinkup-rust-desktop-v0.5.0-x86_64.flatpak.sha256",
                    "browser_download_url": "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.5.0/librelinkup-rust-desktop-v0.5.0-x86_64.flatpak.sha256"
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
        assert_eq!(res.sha256_url, format!("{}.sha256", res.download_url));
        assert!(res.message.contains("v0.5.0"));

        let up_to_date = parse_github_release_json(sample, "0.5.0").expect("valid release JSON");
        assert!(!up_to_date.update_available);
        assert_eq!(up_to_date.message, "Up to date (v0.5.0)");
    }

    #[test]
    fn validates_trusted_github_urls_and_rejects_foreign_hosts() {
        assert!(is_trusted_github_asset_url(
            "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-Setup-0.4.4-x64.exe"
        ));
        assert!(is_trusted_github_asset_url(
            "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/librelinkup-kde-v0.4.4-x86_64.flatpak.sha256"
        ));
        // Reject HTTP, foreign hosts, wrong repo, path traversal, query strings, and credentials
        assert!(!is_trusted_github_asset_url(
            "http://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-Setup-0.4.4-x64.exe"
        ));
        assert!(!is_trusted_github_asset_url(
            "https://evil.example.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-Setup-0.4.4-x64.exe"
        ));
        assert!(!is_trusted_github_asset_url(
            "https://github.com/otheruser/otherrepo/releases/download/v0.4.4/LibreLinkUp-Setup-0.4.4-x64.exe"
        ));
        assert!(!is_trusted_github_asset_url(
            "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/../v0.4.4/LibreLinkUp-Setup-0.4.4-x64.exe"
        ));
        assert!(!is_trusted_github_asset_url(
            "https://user@github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-Setup-0.4.4-x64.exe"
        ));
        assert!(!is_trusted_github_asset_url(
            "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-Setup-0.4.4-x64.exe?redirect=1"
        ));
    }

    #[test]
    fn parses_and_verifies_sha256_checksums() {
        let payload = b"LibreLinkUp verified installer payload";
        let expected_hex = compute_sha256_hex(payload);
        assert_eq!(expected_hex.len(), 64);

        let sha_file = format!("{expected_hex}  LibreLinkUp-Setup-0.5.0-x64.exe\r\n");
        let parsed =
            parse_sha256_checksum(&sha_file, Some("LibreLinkUp-Setup-0.5.0-x64.exe")).unwrap();
        assert_eq!(parsed, expected_hex);
        assert!(verify_sha256_bytes(payload, &parsed).is_ok());

        // Mismatched bytes must fail verification
        assert!(verify_sha256_bytes(b"tampered payload", &parsed).is_err());

        // Mismatched filename in .sha256 file must be rejected
        assert!(parse_sha256_checksum(&sha_file, Some("other.exe")).is_err());

        // Malformed digest must be rejected
        assert!(parse_sha256_checksum("not_a_sha256  LibreLinkUp-Setup-0.5.0-x64.exe", None).is_err());
    }

    #[test]
    fn rejects_shell_injection_tags_and_untrusted_asset_urls_in_release_json() {
        let bad_tag_json = r####"{
            "tag_name": "'; touch /tmp/pwned; '",
            "html_url": "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.0",
            "assets": []
        }"####;
        assert!(parse_github_release_json(bad_tag_json, "0.4.4").is_err());

        // Tag with trailing suffix is normalized strictly to vMAJOR.MINOR.PATCH
        let suffix_tag_json = r####"{
            "tag_name": "v0.5.0-rc1'; echo pwned'",
            "html_url": "https://evil.example.com/phishing",
            "assets": [
                {
                    "name": "LibreLinkUp-Setup-0.5.0-x64.exe",
                    "browser_download_url": "https://evil.example.com/LibreLinkUp-Setup-0.5.0-x64.exe"
                }
            ]
        }"####;
        let res = parse_github_release_json(suffix_tag_json, "0.4.4").unwrap();
        assert_eq!(res.latest_version, "v0.5.0");
        assert_eq!(res.release_url, GITHUB_RELEASES_PAGE_URL);
        // Untrusted asset URL was ignored, falling back to the safe release page URL
        assert_eq!(res.download_url, GITHUB_RELEASES_PAGE_URL);
        assert!(res.sha256_url.is_empty());
    }
}
