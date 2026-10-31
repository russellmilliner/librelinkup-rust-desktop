use std::fmt;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, PRAGMA, USER_AGENT,
};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::crypto::bytes_to_hex;
use crate::models::{
    parse_libreview_timestamp, GlucoseSnapshot, GraphPoint, DEFAULT_TARGET_HIGH_MG_DL,
    DEFAULT_TARGET_LOW_MG_DL,
};

pub const LLU_PRODUCT: &str = "llu.android";
pub const LLU_VERSION: &str = "4.16.0";
pub const LLU_USER_AGENT: &str =
    "Dalvik/2.1.0 (Linux; U; Android 14; Pixel 8 Build/UQ1A.240205.004)";

/// Explicit allowlist of canonical LibreView regional endpoint identifiers.
pub const ALLOWED_LIBREVIEW_REGIONS: &[&str] = &[
    "global", "us", "eu", "eu2", "de", "fr", "ca", "au", "ap", "jp", "ae", "la",
];

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthSuccess {
    pub token: String,
    pub account_id: String,
    pub country: String,
    pub first_name: String,
    pub last_name: String,
}

impl fmt::Debug for AuthSuccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthSuccess")
            .field("token", &"[REDACTED]")
            .field("account_id", &"[REDACTED]")
            .field("country", &self.country)
            .field("first_name", &"[REDACTED]")
            .field("last_name", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    InvalidCredentials,
    PrivacyPolicyAcceptanceRequired,
    NoConnectionsFound,
    Unauthorized,
    NetworkError(String),
    ServerError(u16, String),
    ParseError(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCredentials => {
                write!(
                    f,
                    "Invalid credentials — please check your email and password."
                )
            }
            Self::PrivacyPolicyAcceptanceRequired => {
                write!(
                    f,
                    "Terms of Use / Privacy Policy update required — please sign into the LibreLinkUp mobile app once to accept."
                )
            }
            Self::NoConnectionsFound => {
                write!(
                    f,
                    "No LibreLinkUp connections found — please set up a connection in your Libre mobile app."
                )
            }
            Self::Unauthorized => {
                write!(f, "Session expired or unauthorized — please sign in again.")
            }
            Self::NetworkError(msg) => write!(f, "Network error: {msg}"),
            Self::ServerError(code, msg) => write!(f, "Server error ({code}): {msg}"),
            Self::ParseError(msg) => write!(f, "Invalid API response: {msg}"),
        }
    }
}

/// Returns `true` if `region` is one of the 12 canonical LibreView regional endpoint codes.
pub fn is_allowed_region(region: &str) -> bool {
    let lower = region.trim().to_ascii_lowercase();
    ALLOWED_LIBREVIEW_REGIONS.contains(&lower.as_str())
}

/// Whether an insecure `http://127.0.0.1`/`localhost` endpoint may be used as a LibreView
/// base URL. This exists only for the loopback mock-server tests and CI smoke checks; it is
/// compiled out of release builds unless `LIBRELINKUP_ALLOW_INSECURE_TEST_ENDPOINT=1` is set
/// explicitly, so a shipped binary can never be talked into sending a password in cleartext.
pub fn insecure_test_endpoint_allowed() -> bool {
    if cfg!(debug_assertions) {
        return true;
    }
    matches!(
        std::env::var("LIBRELINKUP_ALLOW_INSECURE_TEST_ENDPOINT").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

/// Validates that `url` is a local loopback `http://127.0.0.1:<port>/llu` or
/// `http://localhost:<port>/llu` endpoint. `allow_insecure` is the policy switch (see
/// [`insecure_test_endpoint_allowed`]); when it is `false` every URL is rejected.
fn loopback_url_allowed(url: &str, allow_insecure: bool) -> bool {
    if !allow_insecure {
        return false;
    }
    let trimmed = url.trim();
    if !trimmed.starts_with("http://") {
        return false;
    }
    let Ok(parsed) = reqwest::Url::parse(trimmed) else {
        return false;
    };
    parsed.scheme() == "http"
        && matches!(parsed.host_str(), Some("127.0.0.1") | Some("localhost"))
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.query().is_none()
        && parsed.fragment().is_none()
}

fn is_loopback_test_url(url: &str) -> bool {
    loopback_url_allowed(url, insecure_test_endpoint_allowed())
}

/// Maps a known LibreView region identifier or ISO-3166 country code to its canonical
/// LibreView region in [`ALLOWED_LIBREVIEW_REGIONS`]. Returns `None` for unknown or untrusted strings.
pub fn try_normalize_region_code(country: &str) -> Option<String> {
    let lower = country.trim().to_ascii_lowercase();
    let region = match lower.as_str() {
        // Direct LibreView API regional identifiers
        "global" | "us" | "eu" | "eu2" | "de" | "fr" | "ca" | "au" | "ap" | "jp" | "ae" | "la" => {
            lower.as_str()
        }
        // UK & Ireland route to eu2 (https://api-eu2.libreview.io/llu)
        "gb" | "uk" | "ie" => "eu2",
        // European / EEA / EMEA countries routed to eu (https://api-eu.libreview.io/llu)
        // per Abbott's official /llu/config/country regionalMap
        "at" | "be" | "bg" | "bh" | "ch" | "cy" | "cz" | "dk" | "ee" | "eg" | "es" | "fi"
        | "gr" | "hr" | "hu" | "il" | "is" | "it" | "jo" | "kw" | "lb" | "li" | "lt" | "lu"
        | "lv" | "mt" | "nl" | "no" | "om" | "pl" | "pt" | "qa" | "ro" | "sa" | "se" | "si"
        | "sk" | "tr" | "za" => "eu",
        // Asia-Pacific countries routed to ap (https://api-ap.libreview.io/llu)
        "hk" | "id" | "in" | "kr" | "my" | "ph" | "sg" | "th" | "tw" | "vn" => "ap",
        // Oceania routed to au (https://api-au.libreview.io/llu)
        "nz" => "au",
        // Latin America routed to la (https://api-la.libreview.io/llu)
        "latam" | "ar" | "bo" | "br" | "cl" | "co" | "cr" | "do" | "ec" | "gt" | "mx" | "pa"
        | "pe" | "py" | "uy" => "la",
        _ => return None,
    };
    Some(region.to_string())
}

pub fn normalize_country_code(country: &str) -> String {
    let trimmed = country.trim();
    if trimmed.is_empty() {
        return "global".to_string();
    }
    if is_loopback_test_url(trimmed) {
        return trimmed.trim_end_matches('/').to_string();
    }
    try_normalize_region_code(trimmed).unwrap_or_else(|| "global".to_string())
}

pub fn resolve_base_url(country_code: &str) -> String {
    let norm = normalize_country_code(country_code);
    if is_loopback_test_url(&norm) {
        return norm;
    }
    if norm == "global" || !is_allowed_region(&norm) {
        "https://api.libreview.io/llu".to_string()
    } else {
        format!("https://api-{norm}.libreview.io/llu")
    }
}

/// Validates that a `patient_id` contains only safe path characters (`[A-Za-z0-9_-]`).
pub fn is_valid_patient_id(patient_id: &str) -> bool {
    !patient_id.is_empty()
        && patient_id.len() <= 128
        && patient_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

static SHARED_API_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Returns the process-wide reusable `reqwest::Client` for LibreView API calls.
pub fn shared_api_client() -> Result<&'static reqwest::Client, ApiError> {
    if let Some(client) = SHARED_API_CLIENT.get() {
        return Ok(client);
    }
    let built = reqwest::Client::builder()
        .user_agent(LLU_USER_AGENT)
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| ApiError::NetworkError(e.to_string()))?;
    let _ = SHARED_API_CLIENT.set(built);
    SHARED_API_CLIENT
        .get()
        .ok_or_else(|| ApiError::NetworkError("Failed to initialize HTTP client".into()))
}

pub fn sha256_account_id(account_id: &str) -> String {
    let d = digest(&SHA256, account_id.trim().as_bytes());
    bytes_to_hex(d.as_ref())
}

fn build_common_headers(
    token: Option<&str>,
    account_id: Option<&str>,
) -> Result<HeaderMap, ApiError> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(LLU_USER_AGENT));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("product", HeaderValue::from_static(LLU_PRODUCT));
    headers.insert("version", HeaderValue::from_static(LLU_VERSION));
    headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));

    if let Some(tok) = token {
        let auth_val = format!("Bearer {}", tok.trim());
        let hv = HeaderValue::from_str(&auth_val)
            .map_err(|e| ApiError::ParseError(format!("Invalid authorization token header: {e}")))?;
        headers.insert(AUTHORIZATION, hv);
    }

    if let Some(acc) = account_id {
        let hashed = sha256_account_id(acc);
        let hv = HeaderValue::from_str(&hashed)
            .map_err(|e| ApiError::ParseError(format!("Invalid Account-Id header: {e}")))?;
        headers.insert("Account-Id", hv);
    }

    Ok(headers)
}

/// Escapes a value for a double-quoted string in a curl `--config` file
/// (curl recognizes `\\`, `\"`, `\t`, `\n`, `\r` and `\v` inside quotes).
pub fn escape_curl_config_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0b}' => out.push_str("\\v"),
            c => out.push(c),
        }
    }
    out
}

/// Builds the curl `--config` document carrying all sensitive request parts
/// (bearer token, hashed account id, JSON body incl. password). It is written to
/// curl's stdin so secrets never appear on the process command line (`ps`, `/proc/*/cmdline`).
pub fn build_curl_secret_config(
    token: Option<&str>,
    account_id: Option<&str>,
    json_payload: Option<&Value>,
) -> String {
    let mut cfg = String::new();
    if let Some(tok) = token {
        let header = format!("Authorization: Bearer {}", tok.trim());
        cfg.push_str(&format!("header = \"{}\"\n", escape_curl_config_value(&header)));
    }
    if let Some(acc) = account_id {
        let header = format!("Account-Id: {}", sha256_account_id(acc));
        cfg.push_str(&format!("header = \"{}\"\n", escape_curl_config_value(&header)));
    }
    if let Some(body) = json_payload {
        cfg.push_str(&format!(
            "data-raw = \"{}\"\n",
            escape_curl_config_value(&body.to_string())
        ));
    }
    cfg
}

fn run_curl_blocking(method: &str, url: &str, secret_config: &str) -> Result<String, ApiError> {
    use std::io::Write;
    use std::process::Stdio;

    let mut cmd = Command::new("curl");
    #[cfg(windows)]
    {
        // GUI app: don't flash a console window for the curl fallback.
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd
        .arg("-s")
        .arg("-S")
        .arg("--proto")
        .arg("=https,http")
        .arg("--max-time")
        .arg("15")
        .arg("-X")
        .arg(method)
        .arg("-H")
        .arg(format!("User-Agent: {LLU_USER_AGENT}"))
        .arg("-H")
        .arg("Accept: application/json")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg(format!("product: {LLU_PRODUCT}"))
        .arg("-H")
        .arg(format!("version: {LLU_VERSION}"))
        .arg("-H")
        .arg("Pragma: no-cache")
        .arg("-H")
        .arg("Cache-Control: no-cache")
        .arg("-w")
        .arg("\n__HTTP_STATUS__:%{http_code}")
        // Sensitive headers/body are read from stdin (see build_curl_secret_config).
        .arg("--config")
        .arg("-")
        .arg("--url")
        .arg(url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ApiError::NetworkError(format!("curl fallback failed to start: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(secret_config.as_bytes()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ApiError::NetworkError(format!("curl fallback stdin error: {e}")));
        }
        // stdin dropped here -> EOF, curl proceeds with the request.
    }

    let output = child
        .wait_with_output()
        .map_err(|e| ApiError::NetworkError(format!("curl fallback failed: {e}")))?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Executes an HTTP request via system `curl` (OpenSSL + HTTP/2 ALPN) as a fallback
/// when Cloudflare WAF blocks a `rustls` TLS fingerprint with 403 Forbidden.
/// Runs on tokio's blocking pool so it never stalls async worker threads.
async fn curl_fallback_json(
    method: &str,
    url: &str,
    token: Option<&str>,
    account_id: Option<&str>,
    json_payload: Option<&Value>,
) -> Result<(u16, Value), ApiError> {
    let method_owned = method.to_string();
    let url_owned = url.to_string();
    let mut secret_config = build_curl_secret_config(token, account_id, json_payload);

    let stdout = tokio::task::spawn_blocking(move || {
        let res = run_curl_blocking(&method_owned, &url_owned, &secret_config);
        zeroize::Zeroize::zeroize(&mut secret_config);
        res
    })
    .await
    .map_err(|e| ApiError::NetworkError(format!("curl fallback task failed: {e}")))??;

    let (body_str, status_code) = if let Some((b, s)) = stdout.rsplit_once("\n__HTTP_STATUS__:") {
        let code = s.trim().parse::<u16>().unwrap_or(500);
        (b.trim(), code)
    } else {
        (stdout.trim(), 500)
    };

    if status_code == 401 {
        return Err(ApiError::Unauthorized);
    }

    if let Ok(json_val) = serde_json::from_str::<Value>(body_str) {
        return Ok((status_code, json_val));
    }

    if !(200..300).contains(&status_code) {
        return Err(ApiError::ServerError(
            status_code,
            format!("HTTP {status_code}"),
        ));
    }

    Err(ApiError::ParseError(
        "Failed to parse JSON response from LibreView server".to_string(),
    ))
}

async fn send_json_request(
    client: &reqwest::Client,
    method: reqwest::Method,
    url: &str,
    token: Option<&str>,
    account_id: Option<&str>,
    json_payload: Option<&Value>,
) -> Result<Value, ApiError> {
    let headers = build_common_headers(token, account_id)?;
    let mut req = client.request(method.clone(), url).headers(headers);
    if let Some(payload) = json_payload {
        req = req.json(payload);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let status_u16 = status.as_u16();
            let text = resp.text().await.unwrap_or_default();

            if let Ok(json_val) = serde_json::from_str::<Value>(&text) {
                if status.is_success() || json_val.get("status").is_some() {
                    return Ok(json_val);
                }
            }

            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(if token.is_some() {
                    ApiError::Unauthorized
                } else {
                    ApiError::InvalidCredentials
                });
            }

            // Cloudflare WAF returned 403 Forbidden without a LibreView JSON body
            // (e.g. TLS JA3/JA4 block on rustls): retry once via system curl (OpenSSL + HTTP/2).
            if status_u16 == 403 && crate::updater::system_curl_available() {
                if let Ok((code, val)) =
                    curl_fallback_json(method.as_str(), url, token, account_id, json_payload).await
                {
                    if (200..300).contains(&code) || val.get("status").is_some() {
                        return Ok(val);
                    }
                }
            }

            Err(ApiError::ServerError(status_u16, status.to_string()))
        }
        Err(e) => {
            // Timeouts and connection failures (offline, DNS) would fail identically via
            // curl and only add another 15 s of waiting, so skip the fallback for them.
            if !e.is_timeout() && !e.is_connect() && crate::updater::system_curl_available() {
                if let Ok((code, val)) =
                    curl_fallback_json(method.as_str(), url, token, account_id, json_payload).await
                {
                    if (200..300).contains(&code) || val.get("status").is_some() {
                        return Ok(val);
                    }
                }
            }
            Err(ApiError::NetworkError(e.to_string()))
        }
    }
}

pub fn parse_login_response(
    body: &Value,
    requested_country: &str,
) -> Result<(Option<String>, Option<AuthSuccess>), ApiError> {
    let status = body.get("status").and_then(|v| v.as_i64()).unwrap_or(-1);
    if status == 2 {
        return Err(ApiError::InvalidCredentials);
    }
    if status == 4 {
        return Err(ApiError::PrivacyPolicyAcceptanceRequired);
    }
    if status != 0 {
        let err_msg = body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("Authentication failed");
        return Err(ApiError::ServerError(
            status.clamp(0, 65535) as u16,
            format!("{err_msg} (status {status})"),
        ));
    }

    let data = body
        .get("data")
        .ok_or_else(|| ApiError::ParseError("Missing 'data' field in login response".to_string()))?;

    let redirect = data
        .get("redirect")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let raw_region_redirect = data
        .get("region")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let region_redirect = match raw_region_redirect {
        Some(raw) => match try_normalize_region_code(raw) {
            Some(valid) => Some(valid),
            None => {
                return Err(ApiError::ParseError(format!(
                    "Refusing redirect to untrusted LibreView region: '{raw}'"
                )));
            }
        },
        None => None,
    };
    let user_country = data
        .get("user")
        .and_then(|u| u.get("country"))
        .and_then(|c| c.as_str())
        .filter(|s| !s.trim().is_empty())
        .and_then(try_normalize_region_code);

    let token = data
        .get("authTicket")
        .and_then(|t| t.get("token"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let account_id = data
        .get("user")
        .and_then(|u| u.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let first_name = data
        .get("user")
        .and_then(|u| u.get("firstName"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let last_name = data
        .get("user")
        .and_then(|u| u.get("lastName"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if redirect || token.is_empty() || account_id.is_empty() {
        let target_region = region_redirect
            .or(user_country)
            .unwrap_or_else(|| normalize_country_code(requested_country));
        return Ok((Some(target_region), None));
    }

    // When login succeeds without redirect, preserve the regional endpoint that
    // issued the authTicket (`requested_country`, or explicit `data.region` if present).
    // Do NOT overwrite the chosen API region with `data.user.country` (which is the
    // user's ISO residence country such as "FI", not an API server subdomain).
    let final_country =
        region_redirect.unwrap_or_else(|| normalize_country_code(requested_country));

    Ok((
        None,
        Some(AuthSuccess {
            token,
            account_id,
            country: final_country,
            first_name,
            last_name,
        }),
    ))
}

pub fn parse_connections_response(
    body: &Value,
) -> Result<(String, String, String, f64, f64), ApiError> {
    let data = body
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            ApiError::ParseError("Missing 'data' array in /connections response".to_string())
        })?;

    let first = data.first().ok_or(ApiError::NoConnectionsFound)?;
    let patient_id = first
        .get("patientId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if patient_id.is_empty() {
        return Err(ApiError::NoConnectionsFound);
    }
    if !is_valid_patient_id(&patient_id) {
        return Err(ApiError::ParseError(
            "Invalid patientId in /connections response".to_string(),
        ));
    }
    let first_name = first
        .get("firstName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let last_name = first
        .get("lastName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let target_low = first
        .get("targetLow")
        .and_then(|v| v.as_f64())
        .unwrap_or(DEFAULT_TARGET_LOW_MG_DL);
    let target_high = first
        .get("targetHigh")
        .and_then(|v| v.as_f64())
        .unwrap_or(DEFAULT_TARGET_HIGH_MG_DL);

    Ok((patient_id, first_name, last_name, target_low, target_high))
}

pub fn parse_graph_response(
    body: &Value,
    fallback_patient_id: &str,
    fallback_first_name: &str,
    fallback_last_name: &str,
) -> Result<GlucoseSnapshot, ApiError> {
    let data = body
        .get("data")
        .ok_or_else(|| ApiError::ParseError("Missing 'data' object in /graph response".to_string()))?;
    let conn = data.get("connection").ok_or_else(|| {
        ApiError::ParseError("Missing 'data.connection' in /graph response".to_string())
    })?;

    let patient_id = conn
        .get("patientId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_patient_id)
        .to_string();
    let first_name = conn
        .get("firstName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_first_name)
        .to_string();
    let last_name = conn
        .get("lastName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_last_name)
        .to_string();

    let target_low_mg_dl = conn
        .get("targetLow")
        .and_then(|v| v.as_f64())
        .unwrap_or(DEFAULT_TARGET_LOW_MG_DL);
    let target_high_mg_dl = conn
        .get("targetHigh")
        .and_then(|v| v.as_f64())
        .unwrap_or(DEFAULT_TARGET_HIGH_MG_DL);

    let gm = conn
        .get("glucoseMeasurement")
        .or_else(|| conn.get("glucoseItem"));

    let value_mg_dl = gm
        .and_then(|g| {
            g.get("ValueInMgPerDl")
                .and_then(|v| v.as_f64())
                .or_else(|| g.get("Value").and_then(|v| v.as_f64()))
        })
        .unwrap_or(0.0);

    let trend_arrow = gm
        .and_then(|g| g.get("TrendArrow").and_then(|v| v.as_i64()))
        .unwrap_or(3) as i32;

    let timestamp = gm
        .and_then(|g| {
            g.get("Timestamp")
                .and_then(|v| v.as_str())
                .or_else(|| g.get("FactoryTimestamp").and_then(|v| v.as_str()))
        })
        .unwrap_or("")
        .to_string();

    // FactoryTimestamp is UTC; Timestamp is the sensor phone's local time (fallback).
    let measured_at_epoch = gm.and_then(|g| {
        g.get("FactoryTimestamp")
            .and_then(|v| v.as_str())
            .and_then(|t| parse_libreview_timestamp(t, true))
            .or_else(|| {
                g.get("Timestamp")
                    .and_then(|v| v.as_str())
                    .and_then(|t| parse_libreview_timestamp(t, false))
            })
    });

    let mut graph_points = Vec::new();
    if let Some(arr) = data.get("graphData").and_then(|v| v.as_array()) {
        for item in arr {
            let val = item
                .get("ValueInMgPerDl")
                .and_then(|v| v.as_f64())
                .or_else(|| item.get("Value").and_then(|v| v.as_f64()))
                .unwrap_or(0.0);
            let ts = item
                .get("Timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if val > 0.0 {
                graph_points.push(GraphPoint {
                    timestamp: ts,
                    value_mg_dl: val,
                });
            }
        }
    }
    if value_mg_dl > 0.0 {
        graph_points.push(GraphPoint {
            timestamp: timestamp.clone(),
            value_mg_dl,
        });
    }

    let sensor_obj = conn.get("sensor").or_else(|| data.get("sensor"));
    let sensor_serial = sensor_obj
        .and_then(|s| s.get("sn").or_else(|| s.get("serialNumber")))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let sensor_activated_at_epoch = sensor_obj
        .and_then(|s| s.get("a").and_then(|v| v.as_i64()))
        .filter(|&ep| ep > 1_000_000_000);
    let sensor_pt = sensor_obj
        .and_then(|s| s.get("pt").and_then(|v| v.as_i64()))
        .unwrap_or(0);
    let sensor_plus_model = sensor_obj
        .and_then(|s| s.get("plus").and_then(|v| v.as_bool()))
        .unwrap_or(sensor_pt == 4);
    let sensor_lifespan_days = if sensor_plus_model {
        15
    } else if sensor_activated_at_epoch.is_some() {
        14
    } else {
        0
    };
    let sensor_expires_at_epoch =
        sensor_activated_at_epoch.map(|act| act + i64::from(sensor_lifespan_days.max(14)) * 86_400);

    Ok(GlucoseSnapshot {
        value_mg_dl,
        trend_arrow,
        timestamp,
        target_low_mg_dl,
        target_high_mg_dl,
        patient_id,
        first_name,
        last_name,
        graph_points,
        measured_at_epoch,
        sensor_serial,
        sensor_activated_at_epoch,
        sensor_expires_at_epoch,
        sensor_lifespan_days,
        sensor_plus_model,
    })
}

pub async fn authenticate_async(
    country: &str,
    username: &str,
    password: &str,
) -> Result<AuthSuccess, ApiError> {
    let client = shared_api_client()?;

    let initial_base = resolve_base_url(country);
    let login_url = format!("{initial_base}/auth/login");

    let payload = serde_json::json!({
        "email": username.trim(),
        "password": password,
    });

    let body = send_json_request(
        client,
        reqwest::Method::POST,
        &login_url,
        None,
        None,
        Some(&payload),
    )
    .await?;

    let (redirect_region, maybe_auth) = parse_login_response(&body, country)?;
    if let Some(auth) = maybe_auth {
        return Ok(auth);
    }

    if let Some(region) = redirect_region {
        let region_base = resolve_base_url(&region);
        let region_url = format!("{region_base}/auth/login");
        let body2 = send_json_request(
            client,
            reqwest::Method::POST,
            &region_url,
            None,
            None,
            Some(&payload),
        )
        .await?;
        let (_, maybe_auth2) = parse_login_response(&body2, &region)?;
        if let Some(auth2) = maybe_auth2 {
            return Ok(auth2);
        }
    }

    Err(ApiError::ParseError(
        "Authentication response did not include an authTicket token".to_string(),
    ))
}

pub async fn fetch_cgm_snapshot_async(
    country: &str,
    token: &str,
    account_id: &str,
) -> Result<GlucoseSnapshot, ApiError> {
    let client = shared_api_client()?;

    let base = resolve_base_url(country);

    let conn_url = format!("{base}/connections");
    let conn_body = send_json_request(
        client,
        reqwest::Method::GET,
        &conn_url,
        Some(token),
        Some(account_id),
        None,
    )
    .await?;

    let (patient_id, first_name, last_name, _, _) = parse_connections_response(&conn_body)?;

    let graph_url = format!("{base}/connections/{patient_id}/graph");
    let graph_body = send_json_request(
        client,
        reqwest::Method::GET,
        &graph_url,
        Some(token),
        Some(account_id),
        None,
    )
    .await?;

    parse_graph_response(&graph_body, &patient_id, &first_name, &last_name)
}

// ---------------------------------------------------------------------------
// Home Assistant LibreLink Integration REST API Support
// ---------------------------------------------------------------------------

/// Validates and normalizes a Home Assistant base URL (e.g. `"http://homeassistant.local:8123"`
/// or `"https://homeassistant.example.com"`). Strips any trailing `/` or `/api`.
pub fn validate_ha_base_url(raw_url: &str) -> Result<String, ApiError> {
    let trimmed = raw_url.trim().trim_end_matches('/');
    let trimmed = trimmed.strip_suffix("/api").unwrap_or(trimmed).trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(ApiError::ParseError(
            "Home Assistant URL is empty. Enter your Home Assistant URL (e.g. http://homeassistant.local:8123)."
                .to_string(),
        ));
    }
    let rest = if let Some(r) = trimmed.strip_prefix("https://") {
        r
    } else if let Some(r) = trimmed.strip_prefix("http://") {
        r
    } else {
        return Err(ApiError::ParseError(
            "Home Assistant URL must start with https:// or http://".to_string(),
        ));
    };
    if rest.is_empty()
        || rest.contains('@')
        || rest.contains('?')
        || rest.contains('#')
        || rest.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(ApiError::ParseError(
            "Invalid characters in Home Assistant URL".to_string(),
        ));
    }
    let host_port = rest.split('/').next().unwrap_or("");
    if host_port.is_empty()
        || !host_port
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
    {
        return Err(ApiError::ParseError(
            "Invalid hostname or port in Home Assistant URL".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn parse_ha_trend_str(raw: &str) -> Option<i32> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    match s {
        "↓" | "⇊" | "1" | "mdi:arrow-down-bold" | "mdi:arrow-down" | "mdi:chevron-double-down" => {
            Some(1)
        }
        "↘" | "2" | "mdi:arrow-bottom-right" | "mdi:arrow-bottom-right-thick" => Some(2),
        "→" | "3" | "mdi:arrow-right" | "mdi:arrow-right-thick" => Some(3),
        "↗" | "4" | "mdi:arrow-top-right" | "mdi:arrow-top-right-thick" => Some(4),
        "↑" | "⇈" | "5" | "mdi:arrow-up-bold" | "mdi:arrow-up" | "mdi:chevron-double-up" => {
            Some(5)
        }
        _ => {
            let up = s.to_ascii_uppercase().replace(['-', '_'], " ");
            match up.as_str() {
                "FALLING QUICKLY" | "RAPIDLY FALLING" => Some(1),
                "FALLING" | "SLOWLY FALLING" => Some(2),
                "STABLE" | "STEADY" | "NORMAL" => Some(3),
                "RISING" | "SLOWLY RISING" => Some(4),
                "RISING QUICKLY" | "RAPIDLY RISING" => Some(5),
                _ => None,
            }
        }
    }
}

fn format_rfc3339_as_local_libreview(iso_raw: &str) -> Option<(i64, String)> {
    let trimmed = iso_raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        let epoch = dt.timestamp();
        let local_str = dt
            .with_timezone(&chrono::Local)
            .format("%m/%d/%Y %I:%M:%S %p")
            .to_string();
        return Some((epoch, local_str));
    }
    if let Some(epoch) = parse_libreview_timestamp(trimmed, true) {
        if let Some(dt) = chrono::DateTime::from_timestamp(epoch, 0) {
            let local_str = dt
                .with_timezone(&chrono::Local)
                .format("%m/%d/%Y %I:%M:%S %p")
                .to_string();
            return Some((epoch, local_str));
        }
    }
    None
}

/// A discovered patient / blood-glucose sensor entity from Home Assistant `/api/states`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct HaPatientOption {
    /// Home Assistant sensor entity ID (e.g. `"sensor.alex_patient_measurement"`).
    pub entity_id: String,
    /// Human-friendly patient name (e.g. `"Alex Patient"`).
    pub patient_name: String,
    /// Current reading string with unit (e.g. `"105 mg/dL"` or `"unavailable"`).
    pub reading_display: String,
    /// Combined display label for UI dropdown selectors.
    pub label: String,
}

fn is_ha_primary_glucose_sensor(item: &Value) -> bool {
    let id = item
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !id.starts_with("sensor.")
        || id.contains("_delta_")
        || id.ends_with("_rate_of_change")
        || id.contains("_time_in_range")
    {
        return false;
    }
    let attrs = item.get("attributes");
    let dev_class = attrs
        .and_then(|a| a.get("device_class"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let attribution = attrs
        .and_then(|a| a.get("attribution"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    dev_class.eq_ignore_ascii_case("blood_glucose_concentration")
        || (id.ends_with("_measurement")
            && attribution.to_ascii_lowercase().contains("libreview"))
}

/// Discovers all patient glucose measurement sensors present in a Home Assistant `/api/states` array.
pub fn discover_ha_patients(states_arr: &[Value]) -> Vec<HaPatientOption> {
    let mut options: Vec<(bool, HaPatientOption)> = Vec::new();

    for item in states_arr.iter().filter(|i| is_ha_primary_glucose_sensor(i)) {
        let Some(entity_id) = item.get("entity_id").and_then(|v| v.as_str()) else {
            continue;
        };
        let without_domain = entity_id.strip_prefix("sensor.").unwrap_or(entity_id);
        let prefix = without_domain
            .strip_suffix("_measurement")
            .or_else(|| without_domain.strip_suffix("_glucose_measurement"))
            .or_else(|| without_domain.strip_suffix("_glucose"))
            .unwrap_or(without_domain);

        let mut patient_name = String::new();
        for ent in states_arr.iter().filter(|e| {
            e.get("entity_id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| id.starts_with(&format!("sensor.{prefix}_application_timestamp")))
        }) {
            if let Some(pname) = ent
                .get("attributes")
                .and_then(|a| a.get("Patient"))
                .and_then(|v| v.as_str())
            {
                let trimmed = pname.trim();
                if !trimmed.is_empty() {
                    patient_name = trimmed.to_string();
                    break;
                }
            }
        }

        let attrs = item.get("attributes");
        if patient_name.is_empty() {
            if let Some(friendly) = attrs
                .and_then(|a| a.get("friendly_name"))
                .and_then(|v| v.as_str())
            {
                let cleaned = friendly
                    .trim()
                    .strip_suffix(" Measurement")
                    .or_else(|| friendly.trim().strip_suffix(" Glucose"))
                    .or_else(|| friendly.trim().strip_suffix(" Blood Glucose"))
                    .unwrap_or(friendly.trim());
                patient_name = cleaned.trim().to_string();
            }
        }
        if patient_name.is_empty() {
            patient_name = prefix.replace('_', " ");
        }

        let raw_state = item
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let numeric_ok = raw_state.parse::<f64>().is_ok_and(|v| v > 0.0);
        let unit = attrs
            .and_then(|a| a.get("unit_of_measurement"))
            .and_then(|v| v.as_str())
            .unwrap_or("mg/dL")
            .trim();
        let reading_display = if numeric_ok {
            format!("{raw_state} {unit}")
        } else if raw_state.is_empty() {
            "unavailable".to_string()
        } else {
            raw_state.to_string()
        };
        let label = format!("{patient_name} ({reading_display}) — {entity_id}");
        options.push((
            numeric_ok,
            HaPatientOption {
                entity_id: entity_id.to_string(),
                patient_name,
                reading_display,
                label,
            },
        ));
    }

    options.sort_by(|(num_a, opt_a), (num_b, opt_b)| {
        num_b
            .cmp(num_a)
            .then_with(|| opt_a.patient_name.cmp(&opt_b.patient_name))
            .then_with(|| opt_a.entity_id.cmp(&opt_b.entity_id))
    });

    options.into_iter().map(|(_, opt)| opt).collect()
}

/// Finds the primary glucose measurement entity object inside a Home Assistant `/api/states` array.
pub fn find_ha_glucose_entity<'a>(
    states_arr: &'a [Value],
    entity_id_hint: &str,
) -> Result<&'a Value, ApiError> {
    let hint = entity_id_hint.trim();
    if !hint.is_empty() {
        if let Some(found) = states_arr.iter().find(|item| {
            item.get("entity_id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| id.eq_ignore_ascii_case(hint))
        }) {
            return Ok(found);
        }
        return Err(ApiError::ParseError(format!(
            "Home Assistant entity '{hint}' was not found in /api/states"
        )));
    }

    let is_glucose_sensor = is_ha_primary_glucose_sensor;

    let has_numeric_state = |item: &Value| -> bool {
        item.get("state")
            .and_then(|v| v.as_str())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .is_some_and(|v| v > 0.0)
    };

    // 1. Prefer `sensor.*_measurement` with a live numeric reading.
    if let Some(found) = states_arr.iter().find(|item| {
        let id = item
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        id.ends_with("_measurement") && is_glucose_sensor(item) && has_numeric_state(item)
    }) {
        return Ok(found);
    }

    // 2. Any blood_glucose_concentration sensor with a live numeric reading.
    if let Some(found) = states_arr
        .iter()
        .find(|item| is_glucose_sensor(item) && has_numeric_state(item))
    {
        return Ok(found);
    }

    // 3. Fallback to any matching glucose sensor even if its state is currently unavailable.
    if let Some(found) = states_arr.iter().find(|item| is_glucose_sensor(item)) {
        return Ok(found);
    }

    Err(ApiError::NoConnectionsFound)
}

/// Parses Home Assistant `/api/states` and optional `/api/history/period/...` payloads into a
/// [`GlucoseSnapshot`].
pub fn parse_ha_states_and_history(
    states_body: &Value,
    history_body: Option<&Value>,
    entity_id_hint: &str,
) -> Result<GlucoseSnapshot, ApiError> {
    let states_arr = states_body.as_array().ok_or_else(|| {
        ApiError::ParseError("Home Assistant /api/states did not return a JSON array".to_string())
    })?;

    let primary = find_ha_glucose_entity(states_arr, entity_id_hint)?;
    let entity_id = primary
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("sensor.librelink_measurement");

    let raw_state = primary
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let raw_val = raw_state.parse::<f64>().map_err(|_| {
        ApiError::ParseError(format!(
            "Home Assistant glucose sensor '{entity_id}' has non-numeric state '{raw_state}'"
        ))
    })?;

    let attrs = primary.get("attributes");
    let unit_attr = attrs
        .and_then(|a| a.get("unit_of_measurement"))
        .and_then(|v| v.as_str())
        .unwrap_or("mg/dL");
    let is_mmol = unit_attr.to_ascii_lowercase().contains("mmol");
    let to_mg_dl = |val: f64| -> f64 {
        if is_mmol {
            val * 18.0182
        } else {
            val
        }
    };
    let value_mg_dl = to_mg_dl(raw_val);

    // Derive the LibreLink entity prefix (e.g. "sensor.patient_glucose_measurement" -> "patient_glucose")
    let without_domain = entity_id.strip_prefix("sensor.").unwrap_or(entity_id);
    let prefix = without_domain
        .strip_suffix("_measurement")
        .or_else(|| without_domain.strip_suffix("_glucose_measurement"))
        .or_else(|| without_domain.strip_suffix("_glucose"))
        .unwrap_or(without_domain);

    // 1. Trend arrow discovery across sibling entities and primary attributes
    let mut trend_arrow: Option<i32> = None;

    // Check `sensor.<prefix>_glucose_trend_arrow`
    let arrow_entity_id = format!("sensor.{prefix}_glucose_trend_arrow");
    if let Some(ent) = states_arr.iter().find(|e| {
        e.get("entity_id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| id == arrow_entity_id)
    }) {
        if let Some(st) = ent.get("state").and_then(|v| v.as_str()) {
            trend_arrow = parse_ha_trend_str(st);
        }
    }

    // Check `sensor.<prefix>_trend` or `sensor.<prefix>_trend_2`
    if trend_arrow.is_none() {
        let trend_prefix = format!("sensor.{prefix}_trend");
        for ent in states_arr.iter().filter(|e| {
            e.get("entity_id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| id == trend_prefix || id.starts_with(&format!("{trend_prefix}_")))
        }) {
            if let Some(ent_attrs) = ent.get("attributes") {
                if let Some(arr_str) = ent_attrs.get("trend_arrow").and_then(|v| v.as_str()) {
                    if let Some(code) = parse_ha_trend_str(arr_str) {
                        trend_arrow = Some(code);
                        break;
                    }
                }
                if let Some(cat_str) = ent_attrs.get("trend_category").and_then(|v| v.as_str()) {
                    if let Some(code) = parse_ha_trend_str(cat_str) {
                        trend_arrow = Some(code);
                        break;
                    }
                }
            }
            if let Some(st) = ent.get("state").and_then(|v| v.as_str()) {
                if let Some(code) = parse_ha_trend_str(st) {
                    trend_arrow = Some(code);
                    break;
                }
            }
        }
    }

    // Fallback to primary sensor's own attributes (`trend_arrow` or `icon`)
    if trend_arrow.is_none() {
        if let Some(a) = attrs {
            if let Some(s) = a.get("trend_arrow").and_then(|v| v.as_str()) {
                trend_arrow = parse_ha_trend_str(s);
            }
            if trend_arrow.is_none() {
                if let Some(icon) = a.get("icon").and_then(|v| v.as_str()) {
                    trend_arrow = parse_ha_trend_str(icon);
                }
            }
        }
    }
    let trend_arrow = trend_arrow.unwrap_or(3);

    // 2. Timestamp & measured_at_epoch discovery
    let mut ts_pair: Option<(i64, String)> = None;
    let ts_prefix = format!("sensor.{prefix}_last_measurement_timestamp");
    for ent in states_arr.iter().filter(|e| {
        e.get("entity_id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| id == ts_prefix || id.starts_with(&format!("{ts_prefix}_")))
    }) {
        if let Some(st) = ent.get("state").and_then(|v| v.as_str()) {
            if let Some(parsed) = format_rfc3339_as_local_libreview(st) {
                ts_pair = Some(parsed);
                break;
            }
        }
    }
    if ts_pair.is_none() {
        if let Some(lu) = primary
            .get("last_updated")
            .and_then(|v| v.as_str())
            .or_else(|| primary.get("last_changed").and_then(|v| v.as_str()))
        {
            ts_pair = format_rfc3339_as_local_libreview(lu);
        }
    }
    let (measured_at_epoch, timestamp) = match ts_pair {
        Some((ep, s)) => (Some(ep), s),
        None => (None, String::new()),
    };

    // 3. Patient name, Patient ID & Sensor Expiry/Warmup metadata from application/expiration timestamp entities
    let mut patient_id = String::new();
    let mut full_name = String::new();
    let mut sensor_serial = String::new();
    let mut sensor_activated_at_epoch: Option<i64> = None;
    let mut sensor_expires_at_epoch: Option<i64> = None;
    let mut sensor_lifespan_days: i32 = 0;
    let mut sensor_plus_model = false;

    for ent in states_arr.iter().filter(|e| {
        e.get("entity_id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| {
                id.starts_with(&format!("sensor.{prefix}_application_timestamp"))
                    || id.starts_with(&format!("sensor.{prefix}_expiration_timestamp"))
            })
    }) {
        let ent_id = ent
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if let Some(st) = ent.get("state").and_then(|v| v.as_str()) {
            if ent_id.starts_with(&format!("sensor.{prefix}_application_timestamp"))
                && sensor_activated_at_epoch.is_none()
            {
                sensor_activated_at_epoch = parse_libreview_timestamp(st, true);
            } else if ent_id.starts_with(&format!("sensor.{prefix}_expiration_timestamp"))
                && sensor_expires_at_epoch.is_none()
            {
                sensor_expires_at_epoch = parse_libreview_timestamp(st, true);
            }
        }
        if let Some(a) = ent.get("attributes") {
            if patient_id.is_empty() {
                if let Some(pid) = a.get("Patient ID").and_then(|v| v.as_str()) {
                    patient_id = pid.trim().to_string();
                }
            }
            if full_name.is_empty() {
                if let Some(pname) = a.get("Patient").and_then(|v| v.as_str()) {
                    full_name = pname.trim().to_string();
                }
            }
            if sensor_serial.is_empty() {
                if let Some(sn) = a.get("Serial number").and_then(|v| v.as_str()) {
                    sensor_serial = sn.trim().to_string();
                }
            }
            if sensor_activated_at_epoch.is_none() {
                if let Some(act_str) = a.get("Activation date").and_then(|v| v.as_str()) {
                    sensor_activated_at_epoch = parse_libreview_timestamp(act_str, true);
                }
            }
            if sensor_lifespan_days <= 0 {
                if let Some(days) = a.get("Sensor lifespan days").and_then(|v| v.as_i64()) {
                    sensor_lifespan_days = days.clamp(1, 60) as i32;
                }
            }
            if let Some(plus) = a.get("Sensor plus model").and_then(|v| v.as_bool()) {
                sensor_plus_model = plus;
            }
        }
    }
    if sensor_expires_at_epoch.is_none() {
        if let Some(act) = sensor_activated_at_epoch {
            let days = if sensor_lifespan_days > 0 {
                i64::from(sensor_lifespan_days)
            } else if sensor_plus_model {
                15
            } else {
                14
            };
            sensor_expires_at_epoch = Some(act + days * 86_400);
        }
    }
    if full_name.is_empty() {
        if let Some(friendly) = attrs
            .and_then(|a| a.get("friendly_name"))
            .and_then(|v| v.as_str())
        {
            let cleaned = friendly
                .trim()
                .strip_suffix(" Measurement")
                .or_else(|| friendly.trim().strip_suffix(" Glucose"))
                .unwrap_or(friendly.trim());
            full_name = cleaned.trim().to_string();
        }
    }
    let (first_name, last_name) = match full_name.split_once(' ') {
        Some((f, l)) => (f.trim().to_string(), l.trim().to_string()),
        None => (full_name, String::new()),
    };
    if patient_id.is_empty() {
        patient_id = format!("ha-{prefix}");
    }

    // 4. Target range discovery from any time_in_range sensor for this prefix
    let mut target_low_mg_dl = DEFAULT_TARGET_LOW_MG_DL;
    let mut target_high_mg_dl = DEFAULT_TARGET_HIGH_MG_DL;
    for ent in states_arr.iter().filter(|e| {
        e.get("entity_id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| id.contains(prefix) && id.contains("time_in_range"))
    }) {
        if let Some(a) = ent.get("attributes") {
            if let Some(low) = a.get("target_low_mgdl").and_then(|v| v.as_f64()) {
                if low > 0.0 {
                    target_low_mg_dl = low;
                }
            }
            if let Some(high) = a.get("target_high_mgdl").and_then(|v| v.as_f64()) {
                if high > 0.0 {
                    target_high_mg_dl = high;
                }
            }
        }
    }

    // 5. Historical 12-hour series from `/api/history/period/...`
    let mut graph_points = Vec::new();
    if let Some(hist) = history_body.and_then(|v| v.as_array()) {
        let entries: Vec<&Value> = if let Some(first_arr) = hist.first().and_then(|v| v.as_array())
        {
            first_arr.iter().collect()
        } else {
            hist.iter().collect()
        };
        for item in entries {
            let Some(st_str) = item.get("state").and_then(|v| v.as_str()) else {
                continue;
            };
            let Ok(raw_v) = st_str.trim().parse::<f64>() else {
                continue;
            };
            let mg_dl = to_mg_dl(raw_v);
            if mg_dl <= 0.0 || mg_dl.is_nan() {
                continue;
            }
            let ts_iso = item
                .get("last_changed")
                .and_then(|v| v.as_str())
                .or_else(|| item.get("last_updated").and_then(|v| v.as_str()))
                .unwrap_or("");
            let formatted_ts = format_rfc3339_as_local_libreview(ts_iso)
                .map(|(_, s)| s)
                .unwrap_or_else(|| ts_iso.to_string());
            if !formatted_ts.is_empty() {
                graph_points.push(GraphPoint {
                    timestamp: formatted_ts,
                    value_mg_dl: mg_dl,
                });
            }
        }
    }

    if value_mg_dl > 0.0 && !timestamp.is_empty() {
        let already_last = graph_points
            .last()
            .is_some_and(|last| last.timestamp == timestamp && (last.value_mg_dl - value_mg_dl).abs() < 0.5);
        if !already_last {
            graph_points.push(GraphPoint {
                timestamp: timestamp.clone(),
                value_mg_dl,
            });
        }
    }

    Ok(GlucoseSnapshot {
        value_mg_dl,
        trend_arrow,
        timestamp,
        target_low_mg_dl,
        target_high_mg_dl,
        patient_id,
        first_name,
        last_name,
        graph_points,
        measured_at_epoch,
        sensor_serial,
        sensor_activated_at_epoch,
        sensor_expires_at_epoch,
        sensor_lifespan_days,
        sensor_plus_model,
    })
}

/// Builds the Home Assistant `/api/states/sensor.librelinkup_alert_snooze` payload for
/// cross-device alert snooze synchronization.
pub fn build_ha_snooze_payload(
    snooze_until_epoch: i64,
    updated_at_epoch: i64,
    now_epoch: i64,
) -> Value {
    let active = snooze_until_epoch > now_epoch;
    let iso_until = if active {
        chrono::DateTime::from_timestamp(snooze_until_epoch, 0)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default()
    } else {
        String::new()
    };
    serde_json::json!({
        "state": if active { "snoozed" } else { "active" },
        "attributes": {
            "friendly_name": "LibreLinkUp Alert Snooze",
            "icon": if active { "mdi:bell-sleep" } else { "mdi:bell-ring-outline" },
            "snoozed": active,
            "snooze_until_epoch": if active { snooze_until_epoch } else { 0 },
            "snooze_until_iso": iso_until,
            "updated_at_epoch": updated_at_epoch,
        }
    })
}

/// Inspects a Home Assistant `/api/states` array for `sensor.librelinkup_alert_snooze` (or
/// `input_datetime.librelinkup_snooze_until`) and returns `Some((remote_snooze_until_epoch, remote_updated_at_epoch))`
/// when the remote state was updated more recently than `local_updated_at_epoch`.
pub fn parse_ha_remote_snooze(
    states_arr: &[Value],
    local_updated_at_epoch: i64,
) -> Option<(i64, i64)> {
    // 1. Primary cross-device entity: `sensor.librelinkup_alert_snooze`
    if let Some(ent) = states_arr.iter().find(|e| {
        e.get("entity_id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| id == "sensor.librelinkup_alert_snooze")
    }) {
        let attrs = ent.get("attributes");
        let remote_updated = attrs
            .and_then(|a| a.get("updated_at_epoch"))
            .and_then(|v| v.as_i64())
            .or_else(|| {
                ent.get("last_updated")
                    .and_then(|v| v.as_str())
                    .and_then(|s| parse_libreview_timestamp(s, true))
            })
            .unwrap_or(0);
        if remote_updated > local_updated_at_epoch {
            let st_str = ent
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let remote_until = if st_str.eq_ignore_ascii_case("active")
                || st_str.eq_ignore_ascii_case("off")
                || st_str == "0"
            {
                0
            } else {
                attrs
                    .and_then(|a| a.get("snooze_until_epoch"))
                    .and_then(|v| v.as_i64())
                    .or_else(|| {
                        attrs
                            .and_then(|a| a.get("snooze_until_iso"))
                            .and_then(|v| v.as_str())
                            .and_then(|s| parse_libreview_timestamp(s, true))
                    })
                    .unwrap_or(0)
            };
            return Some((remote_until.max(0), remote_updated));
        }
    }

    // 2. Optional Home Assistant helper: `input_datetime.librelinkup_snooze_until`
    if let Some(ent) = states_arr.iter().find(|e| {
        e.get("entity_id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| id == "input_datetime.librelinkup_snooze_until")
    }) {
        let remote_updated = ent
            .get("last_updated")
            .and_then(|v| v.as_str())
            .and_then(|s| parse_libreview_timestamp(s, true))
            .unwrap_or(0);
        if remote_updated > local_updated_at_epoch {
            let attrs = ent.get("attributes");
            let remote_until = attrs
                .and_then(|a| a.get("timestamp"))
                .and_then(|v| v.as_f64())
                .map(|t| t as i64)
                .or_else(|| {
                    ent.get("state")
                        .and_then(|v| v.as_str())
                        .and_then(|s| parse_libreview_timestamp(s, false))
                })
                .unwrap_or(0);
            return Some((remote_until.max(0), remote_updated));
        }
    }

    None
}

/// Fetches the full `/api/states` array from Home Assistant.
pub async fn fetch_ha_states_array_async(
    ha_url: &str,
    ha_token: &str,
) -> Result<Vec<Value>, ApiError> {
    let base = validate_ha_base_url(ha_url)?;
    let token = ha_token.trim();
    if token.is_empty() {
        return Err(ApiError::InvalidCredentials);
    }
    let client = shared_api_client()?;
    let states_url = format!("{base}/api/states");
    let states_body = send_json_request(
        client,
        reqwest::Method::GET,
        &states_url,
        Some(token),
        None,
        None,
    )
    .await?;
    states_body.as_array().cloned().ok_or_else(|| {
        ApiError::ParseError("Home Assistant /api/states did not return a JSON array".to_string())
    })
}

/// Publishes the current alert snooze state to Home Assistant
/// (`POST /api/states/sensor.librelinkup_alert_snooze` and
/// `POST /api/events/librelinkup_alert_snooze_changed`).
pub async fn publish_ha_snooze_state_async(
    ha_url: &str,
    ha_token: &str,
    snooze_until_epoch: i64,
    updated_at_epoch: i64,
) -> Result<(), ApiError> {
    let base = validate_ha_base_url(ha_url)?;
    let token = ha_token.trim();
    if token.is_empty() {
        return Err(ApiError::InvalidCredentials);
    }
    let now = chrono::Utc::now().timestamp();
    let payload = build_ha_snooze_payload(snooze_until_epoch, updated_at_epoch, now);
    let client = shared_api_client()?;
    let state_url = format!("{base}/api/states/sensor.librelinkup_alert_snooze");
    send_json_request(
        client,
        reqwest::Method::POST,
        &state_url,
        Some(token),
        None,
        Some(&payload),
    )
    .await?;

    let event_url = format!("{base}/api/events/librelinkup_alert_snooze_changed");
    let event_payload = serde_json::json!({
        "snoozed": snooze_until_epoch > now,
        "snooze_until_epoch": snooze_until_epoch.max(0),
        "updated_at_epoch": updated_at_epoch,
    });
    let _ = send_json_request(
        client,
        reqwest::Method::POST,
        &event_url,
        Some(token),
        None,
        Some(&event_payload),
    )
    .await;
    Ok(())
}

/// Publishes the current [`crate::medications::MedicationStore`] state to Home Assistant
/// (`POST /api/states/sensor.librelinkup_medications`) and optionally fires
/// `POST /api/events/librelinkup_medication_dose_logged` when a dose is logged or skipped.
pub async fn publish_ha_medications_state_async(
    ha_url: &str,
    ha_token: &str,
    store: &crate::medications::MedicationStore,
    logged_dose: Option<&crate::medications::MedicationDoseLog>,
) -> Result<(), ApiError> {
    let base = validate_ha_base_url(ha_url)?;
    let token = ha_token.trim();
    if token.is_empty() {
        return Err(ApiError::InvalidCredentials);
    }
    let now = chrono::Utc::now().timestamp();
    let payload = store.to_ha_sensor_payload(now);
    let client = shared_api_client()?;
    let state_url = format!("{base}/api/states/sensor.librelinkup_medications");
    send_json_request(
        client,
        reqwest::Method::POST,
        &state_url,
        Some(token),
        None,
        Some(&payload),
    )
    .await?;

    if let Some(dose) = logged_dose {
        let event_url = format!("{base}/api/events/librelinkup_medication_dose_logged");
        let event_payload = serde_json::json!({
            "id": dose.id,
            "medication_id": dose.medication_id,
            "medication_name": dose.medication_name,
            "dosage": dose.dosage,
            "taken_at_epoch": dose.taken_at_epoch,
            "taken_at_display": dose.taken_at_display,
            "glucose_mg_dl_at_dose": dose.glucose_mg_dl_at_dose,
            "skipped": dose.skipped,
            "notes": dose.notes,
        });
        let _ = send_json_request(
            client,
            reqwest::Method::POST,
            &event_url,
            Some(token),
            None,
            Some(&event_payload),
        )
        .await;
    }
    Ok(())
}

/// Fetches the current CGM reading, trend arrow, patient metadata, 12-hour history, and the
/// full `/api/states` array from a Home Assistant instance running the LibreLink integration.
pub async fn fetch_ha_cgm_snapshot_and_states_async(
    ha_url: &str,
    ha_token: &str,
    entity_id_hint: &str,
) -> Result<(GlucoseSnapshot, Vec<Value>), ApiError> {
    let base = validate_ha_base_url(ha_url)?;
    let token = ha_token.trim();
    if token.is_empty() {
        return Err(ApiError::InvalidCredentials);
    }

    let client = shared_api_client()?;
    let states_url = format!("{base}/api/states");
    let states_body = send_json_request(
        client,
        reqwest::Method::GET,
        &states_url,
        Some(token),
        None,
        None,
    )
    .await?;

    let states_arr = states_body.as_array().ok_or_else(|| {
        ApiError::ParseError("Home Assistant /api/states did not return a JSON array".to_string())
    })?;
    let primary = find_ha_glucose_entity(states_arr, entity_id_hint)?;
    let matched_entity_id = primary
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let history_body = if !matched_entity_id.is_empty() {
        let start_iso = (chrono::Utc::now() - chrono::Duration::hours(12))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        let hist_url = format!(
            "{base}/api/history/period/{start_iso}?filter_entity_id={matched_entity_id}&minimal_response&no_attributes"
        );
        send_json_request(
            client,
            reqwest::Method::GET,
            &hist_url,
            Some(token),
            None,
            None,
        )
        .await
        .ok()
    } else {
        None
    };

    let snap = parse_ha_states_and_history(&states_body, history_body.as_ref(), entity_id_hint)?;
    Ok((snap, states_arr.clone()))
}

/// Fetches the current CGM reading, trend arrow, patient metadata, and 12-hour history from
/// a Home Assistant instance running the LibreLink integration.
pub async fn fetch_ha_cgm_snapshot_async(
    ha_url: &str,
    ha_token: &str,
    entity_id_hint: &str,
) -> Result<GlucoseSnapshot, ApiError> {
    let (snap, _) = fetch_ha_cgm_snapshot_and_states_async(ha_url, ha_token, entity_id_hint).await?;
    Ok(snap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curl_config_escaping_handles_quotes_backslashes_and_controls() {
        assert_eq!(escape_curl_config_value(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(escape_curl_config_value("x\ny\rz\t"), "x\\ny\\rz\\t");
        assert_eq!(escape_curl_config_value("plain-token.123"), "plain-token.123");
    }

    #[test]
    fn curl_secret_config_carries_token_account_and_body() {
        let body = serde_json::json!({"email": "me@example.com", "password": "p\"w\\d"});
        let cfg = build_curl_secret_config(Some(" tok123 "), Some("acc-1"), Some(&body));
        let lines: Vec<&str> = cfg.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "header = \"Authorization: Bearer tok123\"");
        assert_eq!(
            lines[1],
            format!("header = \"Account-Id: {}\"", sha256_account_id("acc-1"))
        );
        assert!(lines[2].starts_with("data-raw = \""));
        // Password quote/backslash must be escaped twice (JSON, then curl config).
        assert!(lines[2].contains(r#"p\\\"w\\\\d"#), "{}", lines[2]);
    }

    #[test]
    fn curl_secret_config_empty_without_secrets() {
        assert!(build_curl_secret_config(None, None, None).is_empty());
    }

    #[test]
    fn login_preserves_selected_region_when_user_country_differs() {
        let body = serde_json::json!({
            "status": 0,
            "data": {
                "user": {
                    "id": "u-fi-1",
                    "firstName": "Matti",
                    "lastName": "Meikalainen",
                    "country": "FI"
                },
                "authTicket": {
                    "token": "jwt-eu-tok",
                    "expires": 1999999999
                }
            }
        });
        let (redirect, auth) = parse_login_response(&body, "eu").expect("parse login");
        assert!(redirect.is_none());
        let auth = auth.expect("auth present");
        assert_eq!(auth.country, "eu");
        assert_eq!(
            resolve_base_url(&auth.country),
            "https://api-eu.libreview.io/llu"
        );
    }

    #[test]
    fn normalize_iso_countries_to_libreview_regions() {
        for eu_country in ["FI", "fi", "SE", "NO", "DK", "NL", "BE", "AT", "ES", "IT", "PL", "PT", "CH", "ZA"] {
            assert_eq!(normalize_country_code(eu_country), "eu", "failed for {eu_country}");
            assert_eq!(
                resolve_base_url(eu_country),
                "https://api-eu.libreview.io/llu"
            );
        }
        for eu2_country in ["GB", "uk", "IE"] {
            assert_eq!(normalize_country_code(eu2_country), "eu2", "failed for {eu2_country}");
            assert_eq!(
                resolve_base_url(eu2_country),
                "https://api-eu2.libreview.io/llu"
            );
        }
        assert_eq!(normalize_country_code("NZ"), "au");
        assert_eq!(normalize_country_code("BR"), "la");
        assert_eq!(normalize_country_code("SG"), "ap");
    }

    #[test]
    fn rejects_untrusted_regions_external_urls_and_path_traversal() {
        // Unknown region or external URL falls back safely to "global" -> https://api.libreview.io/llu
        for untrusted in [
            "https://evil.example.com/llu",
            "http://evil.example.com/llu",
            "evil.example.com#",
            "us.evil.com/",
            "unknown-region",
        ] {
            assert_eq!(normalize_country_code(untrusted), "global");
            assert_eq!(resolve_base_url(untrusted), "https://api.libreview.io/llu");
        }

        // Loopback mock server URLs are allowed only when the insecure test endpoint gate is
        // open (debug builds / LIBRELINKUP_ALLOW_INSECURE_TEST_ENDPOINT=1). Release builds
        // - including `cargo test --release` - must fall back to the safe default.
        let loopback = "http://127.0.0.1:18080/llu";
        if insecure_test_endpoint_allowed() {
            assert_eq!(resolve_base_url(loopback), loopback);
        } else {
            assert_eq!(resolve_base_url(loopback), "https://api.libreview.io/llu");
        }

        // Spoofed login redirect to an untrusted region or URL is rejected with an error
        let spoofed_redirect = serde_json::json!({
            "status": 0,
            "data": {
                "redirect": true,
                "region": "https://evil.example.com"
            }
        });
        assert!(parse_login_response(&spoofed_redirect, "us").is_err());

        // Path traversal in patientId is rejected
        let bad_conn = serde_json::json!({
            "status": 0,
            "data": [{
                "patientId": "../../admin/secret",
                "firstName": "Test",
                "lastName": "User"
            }]
        });
        assert!(parse_connections_response(&bad_conn).is_err());
    }

    #[test]
    fn loopback_endpoint_requires_explicit_opt_in() {
        let url = "http://127.0.0.1:18080/llu";
        // Allowed only when the insecure policy switch is on (debug builds / test flag).
        assert!(loopback_url_allowed(url, true));
        assert!(!loopback_url_allowed(url, false), "release builds must refuse plaintext loopback");
        assert!(!loopback_url_allowed("http://localhost:1234/llu?x=1", true));
        assert!(!loopback_url_allowed("http://user:pw@127.0.0.1:1/llu", true));
        assert!(!loopback_url_allowed("https://api.libreview.io/llu", true));
        // With the switch off, a stored loopback "region" normalizes back to the safe default.
        assert_eq!(
            normalize_country_code(url),
            if insecure_test_endpoint_allowed() { url } else { "global" }
        );
    }

    #[test]
    fn auth_success_debug_redacts_secrets_and_pii() {
        let auth = AuthSuccess {
            token: "super-secret-jwt-token".into(),
            account_id: "acc-secret-999".into(),
            country: "eu".into(),
            first_name: "Alice".into(),
            last_name: "Patient".into(),
        };
        let dbg = format!("{auth:?}");
        assert!(!dbg.contains("super-secret-jwt-token"));
        assert!(!dbg.contains("acc-secret-999"));
        assert!(!dbg.contains("Alice"));
        assert!(!dbg.contains("Patient"));
        assert!(dbg.contains("[REDACTED]"));
        assert!(dbg.contains("eu"));
    }

    #[test]
    fn parses_home_assistant_librelink_states_and_history() {
        let states = serde_json::json!([
            {
                "entity_id": "sensor.alex_patient_measurement",
                "state": "105",
                "last_updated": "2026-10-31T15:39:25.298895+00:00",
                "attributes": {
                    "unit_of_measurement": "mg/dL",
                    "device_class": "blood_glucose_concentration",
                    "friendly_name": "Alex Patient Measurement",
                    "icon": "mdi:arrow-right",
                    "attribution": "Data provided by https://libreview.com"
                }
            },
            {
                "entity_id": "sensor.alex_patient_trend_2",
                "state": "Rising",
                "attributes": {
                    "trend_arrow": "↗",
                    "trend_category": "RISING"
                }
            },
            {
                "entity_id": "sensor.alex_patient_last_measurement_timestamp_2",
                "state": "2026-10-31T15:39:25+00:00",
                "attributes": {
                    "device_class": "timestamp"
                }
            },
            {
                "entity_id": "sensor.alex_patient_application_timestamp_2",
                "state": "2026-10-30T02:31:26+00:00",
                "attributes": {
                    "Patient ID": "84ab9cce-7a4f-11ed-9dfe-0242ac110006",
                    "Patient": "Alex Patient",
                    "Serial number": "40TUAM8CTE0",
                    "Activation date": "2026-10-30T02:31:26+00:00",
                    "Sensor lifespan days": 15,
                    "Sensor plus model": true
                }
            },
            {
                "entity_id": "sensor.alex_patient_expiration_timestamp_2",
                "state": "2026-11-14T02:31:26+00:00",
                "attributes": {
                    "device_class": "timestamp"
                }
            },
            {
                "entity_id": "sensor.main_bed_alex_patient_time_in_range_24h",
                "state": "99.3",
                "attributes": {
                    "target_low_mgdl": 72,
                    "target_high_mgdl": 175
                }
            },
            {
                "entity_id": "sensor.librelinkup_alert_snooze",
                "state": "snoozed",
                "attributes": {
                    "snooze_until_epoch": 1793500000,
                    "updated_at_epoch": 1793496400
                }
            },
            {
                "entity_id": "sensor.sam_patient_measurement",
                "state": "142",
                "last_updated": "2026-10-31T15:40:00.000000+00:00",
                "attributes": {
                    "unit_of_measurement": "mg/dL",
                    "device_class": "blood_glucose_concentration",
                    "friendly_name": "Sam Patient Measurement",
                    "attribution": "Data provided by https://libreview.com"
                }
            },
            {
                "entity_id": "sensor.sam_patient_application_timestamp",
                "state": "2026-10-28T10:00:00+00:00",
                "attributes": {
                    "Patient ID": "99bb1122-3344-5566-7788-99aabbccddee",
                    "Patient": "Sam Patient",
                    "Serial number": "SAM998877",
                    "Sensor lifespan days": 14
                }
            }
        ]);

        let history = serde_json::json!([[
            {
                "entity_id": "sensor.alex_patient_measurement",
                "state": "98",
                "last_changed": "2026-10-31T15:24:25+00:00"
            },
            {
                "entity_id": "sensor.alex_patient_measurement",
                "state": "102",
                "last_changed": "2026-10-31T15:34:25+00:00"
            }
        ]]);

        let snap = parse_ha_states_and_history(&states, Some(&history), "").expect("parse HA states");
        assert_eq!(snap.value_mg_dl, 105.0);
        assert_eq!(snap.trend_arrow, 4); // ↗ Rising
        assert_eq!(snap.first_name, "Alex");
        assert_eq!(snap.last_name, "Patient");
        assert_eq!(snap.patient_id, "84ab9cce-7a4f-11ed-9dfe-0242ac110006");
        assert_eq!(snap.target_low_mg_dl, 72.0);
        assert_eq!(snap.target_high_mg_dl, 175.0);
        assert_eq!(snap.sensor_serial, "40TUAM8CTE0");
        assert_eq!(snap.sensor_lifespan_days, 15);
        assert!(snap.sensor_plus_model);
        assert!(snap.sensor_activated_at_epoch.is_some());
        assert!(snap.sensor_expires_at_epoch.is_some());
        assert_eq!(snap.graph_points.len(), 3);
        assert!(snap.measured_at_epoch.is_some());

        // Multi-patient discovery & selection
        let states_arr = states.as_array().unwrap();
        let patients = discover_ha_patients(states_arr);
        assert_eq!(patients.len(), 2);
        assert_eq!(patients[0].entity_id, "sensor.alex_patient_measurement");
        assert_eq!(patients[0].patient_name, "Alex Patient");
        assert_eq!(patients[0].reading_display, "105 mg/dL");
        assert_eq!(patients[1].entity_id, "sensor.sam_patient_measurement");
        assert_eq!(patients[1].patient_name, "Sam Patient");
        assert_eq!(patients[1].reading_display, "142 mg/dL");

        let sam_snap = parse_ha_states_and_history(&states, None, "sensor.sam_patient_measurement")
            .expect("select Sam Patient");
        assert_eq!(sam_snap.value_mg_dl, 142.0);
        assert_eq!(sam_snap.first_name, "Sam");
        assert_eq!(sam_snap.last_name, "Patient");
        assert_eq!(sam_snap.sensor_serial, "SAM998877");

        // Cross-device alert snooze parsing
        let remote_snooze = parse_ha_remote_snooze(states_arr, 1793400000);
        assert_eq!(remote_snooze, Some((1793500000, 1793496400)));
        // When local is newer, remote is ignored
        assert_eq!(parse_ha_remote_snooze(states_arr, 1793500000), None);
    }
}
