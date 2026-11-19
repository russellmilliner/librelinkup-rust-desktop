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

const GITHUB_JSON_MAX_BYTES: usize = 1024 * 1024;
const SMALL_TEXT_MAX_BYTES: usize = 64 * 1024;
const UPDATE_BUNDLE_MAX_BYTES: usize = 256 * 1024 * 1024;

/// Embedded minisign public key for release artifacts (base64 blob, key id `a1274a...
///
/// If this line changes, already-installed copies can no longer verify new releases, so it is
/// part of the executable trust root: rotating it requires a manually distributed update.
/// The matching secret key lives ONLY in the `RELEASE_MINISIGN_KEY` CI secret and on the
/// maintainer's offline backup - it must never be committed.
pub const UPDATE_SIGNING_PUBLIC_KEY: &str =
    "RWSUaEnKXTS/QV+rIrbkaqqqZRXJHSwvVGW9g5EP5Nf2T8l6XqGLVqco";

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
    #[serde(default)]
    pub signature_url: String,
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
    segments.len() == 2
        && !segments
            .iter()
            .any(|s| s.is_empty() || *s == "." || *s == "..")
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
            if let Some(expected) = expected_file_name
                && !clean_file.is_empty() && clean_file != expected {
                    continue;
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

/// Verifies a minisign signature over `bytes` against `public_key_b64`.
///
/// When `expected_artifact` is `Some(file_name)`, also verifies that the signed
/// `trusted comment:` contains `artifact:<file_name>` so a signature cannot be replayed
/// across different release artifacts.
pub fn verify_minisign_signature_for_artifact(
    bytes: &[u8],
    signature_text: &str,
    public_key_b64: &str,
    expected_artifact: Option<&str>,
) -> Result<(), String> {
    let trimmed = signature_text.trim();
    if trimmed.is_empty() {
        return Err("Refusing update: release signature is missing.".into());
    }
    let public_key = minisign_verify::PublicKey::from_base64(public_key_b64.trim())
        .map_err(|e| format!("Invalid embedded update public key: {e}"))?;
    let signature = minisign_verify::Signature::decode(trimmed)
        .map_err(|e| format!("Malformed update signature: {e}"))?;
    public_key
        .verify(bytes, &signature, false)
        .map_err(|e| format!("Update signature verification failed: {e}"))?;
    if let Some(artifact_name) = expected_artifact {
        let bound = signature
            .trusted_comment()
            .split_whitespace()
            .any(|tok| tok.strip_prefix("artifact:") == Some(artifact_name));
        if !bound {
            return Err(format!(
                "Update signature is not bound to artifact '{artifact_name}'."
            ));
        }
    }
    Ok(())
}

/// Verifies a minisign signature over `bytes` against `public_key_b64`.
///
/// This is the actual authenticity check for auto-updates: the SHA-256 sidecar comes from the
/// same origin as the artifact and therefore only proves the download was not corrupted,
/// whereas the signature proves the artifact was produced by the holder of the release key.
pub fn verify_minisign_signature(
    bytes: &[u8],
    signature_text: &str,
    public_key_b64: &str,
) -> Result<(), String> {
    verify_minisign_signature_for_artifact(bytes, signature_text, public_key_b64, None)
}

/// Verifies `bytes` against the compiled-in release public key.
pub fn verify_update_signature(bytes: &[u8], signature_text: &str) -> Result<(), String> {
    verify_minisign_signature_for_artifact(bytes, signature_text, UPDATE_SIGNING_PUBLIC_KEY, None)
}

/// Verifies `bytes` against the compiled-in release public key and ensures the signed
/// trusted comment binds the signature to `expected_artifact`.
pub fn verify_update_signature_for_artifact(
    bytes: &[u8],
    signature_text: &str,
    expected_artifact: &str,
) -> Result<(), String> {
    verify_minisign_signature_for_artifact(
        bytes,
        signature_text,
        UPDATE_SIGNING_PUBLIC_KEY,
        Some(expected_artifact),
    )
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
    let mut signature_assets: HashMap<String, String> = HashMap::new();

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
            } else if name.ends_with(".minisig") {
                signature_assets.insert(name.to_string(), url.to_string());
            } else if name.ends_with(".flatpak")
                && asset_matches_host_arch(name)
                && flatpak_asset.is_none()
            {
                flatpak_asset = Some((name.to_string(), url.to_string()));
            } else if name.ends_with(".exe")
                && asset_matches_host_arch(name)
                && windows_installer_asset.is_none()
            {
                windows_installer_asset = Some((name.to_string(), url.to_string()));
            }
        }
    }

    let preferred_asset = if cfg!(target_os = "windows") {
        windows_installer_asset.or(flatpak_asset)
    } else {
        flatpak_asset.or(windows_installer_asset)
    };
    let (download_url, sha256_url, signature_url) = match preferred_asset {
        Some((asset_name, asset_url)) => {
            let sha_url = sha256_assets
                .get(&format!("{asset_name}.sha256"))
                .cloned()
                .unwrap_or_else(|| format!("{asset_url}.sha256"));
            let sig_url = signature_assets
                .get(&format!("{asset_name}.minisig"))
                .cloned()
                .unwrap_or_else(|| format!("{asset_url}.minisig"));
            (asset_url, sha_url, sig_url)
        }
        None => (release_url.clone(), String::new(), String::new()),
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
        signature_url,
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

/// Returns whether a release asset filename matches the current CPU architecture
/// when the filename includes an explicit architecture marker.
fn asset_matches_host_arch(asset_name: &str) -> bool {
    let lower = asset_name.to_ascii_lowercase();
    let has_x64 = lower.contains("x86_64") || lower.contains("-x64") || lower.contains("_x64");
    let has_arm64 = lower.contains("aarch64") || lower.contains("arm64");
    match (has_x64, has_arm64) {
        (true, false) => std::env::consts::ARCH == "x86_64",
        (false, true) => std::env::consts::ARCH == "aarch64",
        _ => true,
    }
}

/// Resolves the system `curl` executable path without relying on untrusted `PATH` entries
/// on Windows (via Win32 `GetSystemDirectoryW`).
#[cfg(windows)]
pub(crate) fn system_curl_program() -> Option<PathBuf> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemDirectoryW(lp_buffer: *mut u16, u_size: u32) -> u32;
    }
    let mut buf = vec![0u16; 260];
    // SAFETY: `buf` points to writable `u16` storage of length `buf.len()`.
    let len = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) };
    if len == 0 {
        return None;
    }
    let len = if (len as usize) >= buf.len() {
        buf.resize((len as usize) + 1, 0);
        // SAFETY: `buf` resized to the required UTF-16 length reported by Windows.
        let retry = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) };
        if retry == 0 || (retry as usize) >= buf.len() {
            return None;
        }
        retry as usize
    } else {
        len as usize
    };
    use std::os::windows::ffi::OsStringExt;
    let dir = PathBuf::from(std::ffi::OsString::from_wide(&buf[..len]));
    let candidate = dir.join("curl.exe");
    candidate.is_file().then_some(candidate)
}

#[cfg(not(windows))]
pub(crate) fn system_curl_program() -> Option<PathBuf> {
    Some(PathBuf::from("curl"))
}

/// Whether a usable system `curl` exists.
///
/// The curl fallback exists because Cloudflare can block the rustls TLS fingerprint; when the
/// binary is absent (minimal containers, some Flatpak runtimes) the fallback cannot work, and
/// silently retrying just turns a clear error into a confusing one. Probed once and cached.
pub fn system_curl_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let ok = system_curl_program()
            .and_then(|prog| {
                let mut cmd = Command::new(prog);
                cmd.arg("--version")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(0x0800_0000);
                }
                cmd.status().ok()
            })
            .is_some_and(|s| s.success());
        if !ok {
            crate::event_log::log_warn(
                "SYSTEM",
                "System curl is unavailable: the TLS-fingerprint fallback for blocked requests is disabled.",
            );
        }
        ok
    })
}

async fn read_response_bytes_bounded(
    mut resp: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    if let Some(len) = resp.content_length()
        && len > max_bytes as u64
    {
        return Err(format!(
            "Response body too large ({len} bytes > {max_bytes} byte limit)"
        ));
    }
    let cap = resp
        .content_length()
        .and_then(|l| usize::try_from(l).ok())
        .map_or(0, |l| l.min(max_bytes));
    let mut buf = Vec::with_capacity(cap);
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("Failed reading response stream: {e}"))?
    {
        if buf.len().saturating_add(chunk.len()) > max_bytes {
            return Err(format!("Response stream exceeded {max_bytes} byte limit"));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

async fn read_response_text_bounded(
    resp: reqwest::Response,
    max_bytes: usize,
) -> Result<String, String> {
    let bytes = read_response_bytes_bounded(resp, max_bytes).await?;
    Ok(String::from_utf8(bytes)
        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

#[cfg(unix)]
fn curl_get_github_release(
    url: &str,
    current_version: &str,
    max_bytes: usize,
) -> Result<String, String> {
    let curl_prog =
        system_curl_program().ok_or_else(|| "System curl is unavailable.".to_string())?;
    let ua = format!(
        "User-Agent: librelinkup-rust-desktop/{}",
        current_version.trim().trim_start_matches(['v', 'V'])
    );
    let max_size_str = max_bytes.to_string();
    let out = Command::new(curl_prog)
        .args([
            "-fsSL",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-filesize",
            &max_size_str,
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
    if out.stdout.len() > max_bytes {
        return Err(format!("curl response exceeded {max_bytes} byte limit"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Fetches a small text asset (checksum / signature) with a `curl` fallback on Unix, used
/// when Cloudflare-style TLS fingerprint filtering blocks the rustls client.
async fn fetch_release_text(client: &reqwest::Client, url: &str) -> Option<String> {
    if let Ok(resp) = client.get(url).send().await
        && resp.status().is_success()
            && let Ok(text) = read_response_text_bounded(resp, SMALL_TEXT_MAX_BYTES).await
                && !text.trim().is_empty() {
                    return Some(text);
                }
    #[cfg(unix)]
    if system_curl_available() {
        let owned = url.to_string();
        if let Ok(Ok(raw)) = tokio::task::spawn_blocking(move || {
            curl_get_github_release(&owned, env!("CARGO_PKG_VERSION"), SMALL_TEXT_MAX_BYTES)
        })
        .await
            && !raw.trim().is_empty() {
                return Some(raw);
            }
    }
    None
}

static SHARED_UPDATE_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static SHARED_DOWNLOAD_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn shared_update_client() -> Result<&'static reqwest::Client, String> {
    if let Some(c) = SHARED_UPDATE_CLIENT.get() {
        return Ok(c);
    }
    let built = reqwest::Client::builder()
        .https_only(true)
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
        .https_only(true)
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
            let text = read_response_text_bounded(resp, GITHUB_JSON_MAX_BYTES)
                .await
                .map_err(|e| format!("Failed reading update response: {e}"))?;
            if !status.is_success() {
                return Err(format!("GitHub API returned HTTP {}", status.as_u16()));
            }
            parse_github_release_json(&text, current_version)
        }
        Err(err) => {
            #[cfg(unix)]
            if system_curl_available() {
                let cur = current_version.to_string();
                if let Ok(Ok(raw)) = tokio::task::spawn_blocking(move || {
                    curl_get_github_release(
                        GITHUB_RELEASES_LATEST_URL,
                        &cur,
                        GITHUB_JSON_MAX_BYTES,
                    )
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
    use std::io::Read as _;

    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .timeout(Duration::from_secs(10))
        .default_headers(github_headers(current_version))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    match client.get(GITHUB_RELEASES_LATEST_URL).send() {
        Ok(resp) => {
            let status = resp.status();
            let mut limited = resp.take((GITHUB_JSON_MAX_BYTES + 1) as u64);
            let mut buf = Vec::new();
            limited
                .read_to_end(&mut buf)
                .map_err(|e| format!("Failed reading update response: {e}"))?;
            if buf.len() > GITHUB_JSON_MAX_BYTES {
                return Err(format!(
                    "Update response exceeded {GITHUB_JSON_MAX_BYTES} byte limit"
                ));
            }
            let text = String::from_utf8_lossy(&buf);
            if !status.is_success() {
                return Err(format!("GitHub API returned HTTP {}", status.as_u16()));
            }
            parse_github_release_json(&text, current_version)
        }
        Err(err) => {
            #[cfg(unix)]
            if system_curl_available()
                && let Ok(raw) = curl_get_github_release(
                    GITHUB_RELEASES_LATEST_URL,
                    current_version,
                    GITHUB_JSON_MAX_BYTES,
                )
            {
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
    if !is_newer_version(env!("CARGO_PKG_VERSION"), &clean_ver) {
        return Err(format!(
            "Refusing update: target version v{clean_ver} is not newer than installed v{}.",
            env!("CARGO_PKG_VERSION")
        ));
    }

    let asset_file_name = download_url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Invalid asset filename in download URL.".to_string())?;
    if !asset_matches_host_arch(asset_file_name) {
        return Err(format!(
            "Refusing update: asset '{asset_file_name}' does not match host architecture '{}'.",
            std::env::consts::ARCH
        ));
    }
    let sha256_url = format!("{download_url}.sha256");
    let signature_url = format!("{download_url}.minisig");
    if !is_trusted_github_asset_url(&sha256_url) {
        return Err("Invalid SHA-256 checksum URL.".into());
    }
    if !is_trusted_github_asset_url(&signature_url) {
        return Err("Invalid update signature URL.".into());
    }

    let file_name = if cfg!(target_os = "windows") {
        format!("LibreLinkUp-Setup-{clean_ver}-x64.exe")
    } else {
        format!(
            "librelinkup-rust-desktop-v{clean_ver}-{}.flatpak",
            std::env::consts::ARCH
        )
    };

    let dest_dir = crate::paths::update_staging_dir();
    if dest_dir == std::env::temp_dir() {
        return Err(
            "Refusing update: no private user data directory available for staging.".into(),
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let has_user_base = std::env::var("HOME")
            .ok()
            .is_some_and(|s| !s.trim().is_empty())
            || std::env::var("XDG_DATA_HOME")
                .ok()
                .is_some_and(|s| !s.trim().is_empty())
            || std::env::var("LIBRELINKUP_DATA_DIR")
                .ok()
                .is_some_and(|s| !s.trim().is_empty());
        if !has_user_base {
            return Err(
                "Refusing update: update staging fell back to temporary directory.".into(),
            );
        }
        let meta = std::fs::symlink_metadata(&dest_dir)
            .map_err(|e| format!("Failed inspecting update staging directory: {e}"))?;
        if meta.file_type().is_symlink() || !meta.is_dir() || (meta.mode() & 0o077) != 0 {
            return Err(
                "Refusing update: unsafe permissions on update staging directory.".into(),
            );
        }
    }
    #[cfg(windows)]
    {
        let has_user_base = std::env::var("LOCALAPPDATA")
            .ok()
            .is_some_and(|s| !s.trim().is_empty())
            || std::env::var("USERPROFILE")
                .ok()
                .is_some_and(|s| !s.trim().is_empty())
            || std::env::var("LIBRELINKUP_DATA_DIR")
                .ok()
                .is_some_and(|s| !s.trim().is_empty());
        if !has_user_base {
            return Err(
                "Refusing update: update staging fell back to temporary directory.".into(),
            );
        }
    }
    crate::paths::prune_update_staging_dir(&dest_dir);
    let dest_path = dest_dir.join(&file_name);

    let client = shared_download_client()?;

    // 1. The signature is mandatory and is checked against the compiled-in release key.
    //    It is the only check that survives a compromised repository/token, because the
    //    SHA-256 sidecar is fetched from the very same origin as the artifact.
    let signature_text = fetch_release_text(client, &signature_url)
        .await
        .ok_or_else(|| {
            format!(
                "Refusing to install update: release signature could not be fetched from \
                 {signature_url}. Unsigned updates are never executed."
            )
        })?;

    // 2. Fetch and parse the mandatory .sha256 checksum file (corruption/partial-download guard).
    let sha256_raw = fetch_release_text(client, &sha256_url)
        .await
        .ok_or_else(|| {
            format!("Refusing to install update: could not fetch SHA-256 checksum from {sha256_url}")
        })?;
    let expected_sha256 = parse_sha256_checksum(&sha256_raw, Some(asset_file_name))?;

    // 3. Stream download with bounded maximum size (single buffered copy).
    let mut downloaded: Option<Vec<u8>> = None;
    if let Ok(resp) = client.get(download_url).send().await
        && resp.status().is_success()
            && let Ok(buf) = read_response_bytes_bounded(resp, UPDATE_BUNDLE_MAX_BYTES).await
                && buf.len() > 1024
            {
                downloaded = Some(buf);
            }

    #[cfg(unix)]
    if downloaded.is_none()
        && system_curl_available()
        && let Some(curl_prog) = system_curl_program()
    {
        let url_owned = download_url.to_string();
        let (tmp_dl_path, tmp_dl_file) =
            crate::paths::create_private_temp_file(&dest_dir, ".llu-update-", ".part")?;
        drop(tmp_dl_file);
        let tmp_clone = tmp_dl_path.clone();
        let dl_res = tokio::task::spawn_blocking(move || {
            let max_size_str = UPDATE_BUNDLE_MAX_BYTES.to_string();
            let status = Command::new(curl_prog)
                .args([
                    "-fsSL",
                    "--proto",
                    "=https",
                    "--proto-redir",
                    "=https",
                    "--max-filesize",
                    &max_size_str,
                    "--max-time",
                    "180",
                    "-o",
                ])
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
        let file_len = std::fs::metadata(&tmp_dl_path)
            .map(|m| m.len())
            .unwrap_or(u64::MAX);
        if file_len > UPDATE_BUNDLE_MAX_BYTES as u64 {
            let _ = std::fs::remove_file(&tmp_dl_path);
            return Err(format!(
                "Downloaded update exceeded {UPDATE_BUNDLE_MAX_BYTES} byte limit"
            ));
        }
        let bytes = std::fs::read(&tmp_dl_path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp_dl_path);
            format!("Failed reading downloaded update package: {e}")
        })?;
        let _ = std::fs::remove_file(&tmp_dl_path);
        downloaded = Some(bytes);
    }

    let Some(bytes) = downloaded else {
        return Err(format!("Failed to download update from {download_url}"));
    };

    if let Err(err) = verify_sha256_bytes(&bytes, &expected_sha256) {
        let _ = std::fs::remove_file(&dest_path);
        return Err(format!("Refusing to install update: {err}"));
    }

    // 4. Authenticity: the artifact must be signed by the release key and bound to this filename.
    if let Err(err) = verify_update_signature_for_artifact(&bytes, &signature_text, asset_file_name)
    {
        let _ = std::fs::remove_file(&dest_path);
        return Err(format!("Refusing to install update: {err}"));
    }

    crate::paths::write_file_atomic_0600(&dest_path, &bytes)
        .map_err(|e| format!("Failed writing verified update package: {e}"))?;
    #[cfg(unix)]
    if file_name.ends_with(".flatpak") {
        use std::os::unix::fs::PermissionsExt;
        // Public signed release bundle: allow flatpak-system-helper, Bazaar, and KDE Discover
        // on Bazzite to read the verified bundle when installing into --system.
        let _ = std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(0o644));
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
            let scopes = if crate::paths::is_system_flatpak_install() {
                ["--system", "--user"]
            } else {
                ["--user", "--system"]
            };
            let mut last_err = String::new();
            let mut installed = false;
            for scope in scopes {
                let res = if in_flatpak {
                    // Inside Flatpak sandbox: dest_clone is in paths::update_staging_dir()
                    // (~/.local/share/librelinkup-kde/updates), which exists at the identical path on the host.
                    Command::new("flatpak-spawn")
                        .args([
                            "--host",
                            "flatpak",
                            "install",
                            scope,
                            "-y",
                            "--reinstall",
                            "--",
                        ])
                        .arg(&dest_clone)
                        .output()
                } else {
                    Command::new("flatpak")
                        .args(["install", scope, "-y", "--reinstall", "--"])
                        .arg(&dest_clone)
                        .output()
                };
                match res {
                    Ok(out) if out.status.success() => {
                        installed = true;
                        break;
                    }
                    Ok(out) => {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        last_err = format!(
                            "Flatpak update install ({scope}) failed (status {}): {}",
                            out.status.code().unwrap_or(-1),
                            stderr.trim()
                        );
                    }
                    Err(e) => {
                        last_err = format!("Failed to invoke flatpak installer ({scope}): {e}");
                    }
                }
            }

            if installed {
                if in_flatpak {
                    let _ = Command::new("flatpak-spawn")
                        .args([
                            "--host",
                            "sh",
                            "-c",
                            "(flatpak kill io.github.russellmilliner.LibreLinkUpDesktop 2>/dev/null || true) && nohup flatpak run io.github.russellmilliner.LibreLinkUpDesktop >/dev/null 2>&1 &",
                        ])
                        .spawn();
                }
                return Ok(dest_clone);
            }

            // Fallback (e.g. if org.freedesktop.Flatpak was revoked in Flatseal on Bazzite):
            // open the verified .flatpak bundle in the desktop software center (Bazaar / KDE Discover).
            let opened = if in_flatpak {
                Command::new("flatpak-spawn")
                    .args(["--host", "xdg-open"])
                    .arg(&dest_clone)
                    .spawn()
                    .or_else(|_| Command::new("xdg-open").arg(&dest_clone).spawn())
            } else {
                Command::new("xdg-open").arg(&dest_clone).spawn()
            };
            if opened.is_ok() {
                return Ok(dest_clone);
            }

            Err(if last_err.is_empty() {
                "Flatpak update install failed".into()
            } else {
                last_err
            })
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
    fn verifies_minisign_signatures_and_rejects_tampering() {
        // Fixture generated with a throwaway test key (never the release key).
        const TEST_PUBKEY: &str = "RWRRI0rrvCS5oePZPKf/GuR5nGi7Y9L4V1r026s2IY9Ze0HhrIYHkjy6";
        const TEST_SIG: &str = "untrusted comment: test:update-fixture\n\
RURRI0rrvCS5obXzUzG5SAKLqhmV7rJHLvTbBN9qISUB8hfJ/STjnzWraGunPcopQnoZTwIw3PilJs82T9piNArxmF3ZDyWMKgo=\n\
trusted comment: timestamp:1793326084\n\
CwkbwGp+4TByUUf5t2zRCWvVkwdiTliNUMi8V5+91AenrDh/4eoeBNYWtKhcb/T3RFgv5+Udsj96Y5C+iCcSAA==";
        let payload = b"librelinkup-update-fixture-payload-v1";

        verify_minisign_signature(payload, TEST_SIG, TEST_PUBKEY).expect("valid signature");

        // Tampered payload must fail.
        assert!(verify_minisign_signature(b"tampered", TEST_SIG, TEST_PUBKEY).is_err());
        // Wrong key must fail.
        assert!(verify_minisign_signature(payload, TEST_SIG, UPDATE_SIGNING_PUBLIC_KEY).is_err());
        // Missing artifact binding in trusted comment must fail when artifact verification is requested.
        assert!(
            verify_minisign_signature_for_artifact(
                payload,
                TEST_SIG,
                TEST_PUBKEY,
                Some("librelinkup-rust-desktop-v0.6.0-x86_64.flatpak"),
            )
            .is_err()
        );
        // Missing / empty signature must fail closed.
        assert!(verify_minisign_signature(payload, "", TEST_PUBKEY).is_err());
        assert!(verify_minisign_signature(payload, "   \n", TEST_PUBKEY).is_err());
        // Garbage signature must not panic.
        assert!(verify_minisign_signature(payload, "not a signature", TEST_PUBKEY).is_err());
    }

    #[test]
    fn embedded_release_public_key_is_a_valid_minisign_key() {
        // Guards against a truncated/pasted-wrong trust root: if this fails, every
        // installed copy would refuse all future updates.
        minisign_verify::PublicKey::from_base64(UPDATE_SIGNING_PUBLIC_KEY)
            .expect("embedded update public key must decode");
        assert!(!UPDATE_SIGNING_PUBLIC_KEY.trim().is_empty());
    }

    #[test]
    fn release_json_exposes_checksum_and_signature_urls() {
        let sample = r#"{
            "tag_name": "v0.6.0",
            "html_url": "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.6.0",
            "assets": [
                {
                    "name": "librelinkup-rust-desktop-v0.6.0-x86_64.flatpak",
                    "browser_download_url": "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.6.0/librelinkup-rust-desktop-v0.6.0-x86_64.flatpak"
                },
                {
                    "name": "librelinkup-rust-desktop-v0.6.0-x86_64.flatpak.sha256",
                    "browser_download_url": "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.6.0/librelinkup-rust-desktop-v0.6.0-x86_64.flatpak.sha256"
                },
                {
                    "name": "librelinkup-rust-desktop-v0.6.0-x86_64.flatpak.minisig",
                    "browser_download_url": "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.6.0/librelinkup-rust-desktop-v0.6.0-x86_64.flatpak.minisig"
                }
            ]
        }"#;
        let res = parse_github_release_json(sample, "0.5.0").expect("valid release json");
        if std::env::consts::ARCH == "x86_64" {
            assert!(res.download_url.ends_with(".flatpak"));
            assert!(res.sha256_url.ends_with(".flatpak.sha256"));
            assert!(
                res.signature_url.ends_with(".flatpak.minisig"),
                "signature_url must be discovered from release assets: {}",
                res.signature_url
            );
            assert!(is_trusted_github_asset_url(&res.signature_url));
        }
        assert!(asset_matches_host_arch(&format!(
            "librelinkup-rust-desktop-v0.6.0-{}.flatpak",
            std::env::consts::ARCH
        )));
        let foreign_arch = if std::env::consts::ARCH == "x86_64" {
            "aarch64"
        } else {
            "x86_64"
        };
        assert!(!asset_matches_host_arch(&format!(
            "librelinkup-rust-desktop-v0.6.0-{foreign_arch}.flatpak"
        )));
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
