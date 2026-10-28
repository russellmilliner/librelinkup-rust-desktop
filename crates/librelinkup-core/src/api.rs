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

/// Validates that `url` is a local loopback `http://127.0.0.1:<port>/llu` or `http://localhost:<port>/llu`
/// endpoint used by integration mock tests. External or non-loopback URLs are rejected.
fn is_loopback_test_url(url: &str) -> bool {
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
            if status_u16 == 403 {
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
            if !e.is_timeout() && !e.is_connect() {
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

        // Loopback mock server URL remains allowed for integration tests
        assert_eq!(
            resolve_base_url("http://127.0.0.1:18080/llu"),
            "http://127.0.0.1:18080/llu"
        );

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
}
