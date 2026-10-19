use std::process::Command;
use std::time::Duration;

use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, PRAGMA, USER_AGENT,
};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::crypto::bytes_to_hex;
use crate::models::{
    GlucoseSnapshot, GraphPoint, DEFAULT_TARGET_HIGH_MG_DL, DEFAULT_TARGET_LOW_MG_DL,
};

pub const LLU_PRODUCT: &str = "llu.android";
pub const LLU_VERSION: &str = "4.16.0";
pub const LLU_USER_AGENT: &str =
    "Dalvik/2.1.0 (Linux; U; Android 14; Pixel 8 Build/UQ1A.240205.004)";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthSuccess {
    pub token: String,
    pub account_id: String,
    pub country: String,
    pub first_name: String,
    pub last_name: String,
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

pub fn normalize_country_code(country: &str) -> String {
    let lower = country.trim().to_lowercase();
    if lower.is_empty() {
        return "global".to_string();
    }
    if lower == "ch" {
        return "eu".to_string();
    }
    lower
}

pub fn resolve_base_url(country_code: &str) -> String {
    let norm = normalize_country_code(country_code);
    if norm.starts_with("http://") || norm.starts_with("https://") {
        return norm.trim_end_matches('/').to_string();
    }
    if norm == "global" {
        "https://api.libreview.io/llu".to_string()
    } else {
        format!("https://api-{norm}.libreview.io/llu")
    }
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

/// Executes an HTTP request via system `curl` (OpenSSL + HTTP/2 ALPN) as a fallback
/// when Cloudflare WAF blocks a `rustls` TLS fingerprint with 403 Forbidden.
fn curl_fallback_json(
    method: &str,
    url: &str,
    token: Option<&str>,
    account_id: Option<&str>,
    json_payload: Option<&Value>,
) -> Result<(u16, Value), ApiError> {
    let mut cmd = Command::new("curl");
    cmd.arg("-s")
        .arg("-S")
        .arg("--max-time")
        .arg("15")
        .arg("-X")
        .arg(method)
        .arg(url)
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
        .arg("\n__HTTP_STATUS__:%{http_code}");

    if let Some(tok) = token {
        cmd.arg("-H").arg(format!("Authorization: Bearer {}", tok.trim()));
    }
    if let Some(acc) = account_id {
        let hashed = sha256_account_id(acc);
        cmd.arg("-H").arg(format!("Account-Id: {hashed}"));
    }
    if let Some(body) = json_payload {
        cmd.arg("--data-raw").arg(body.to_string());
    }

    let output = cmd
        .output()
        .map_err(|e| ApiError::NetworkError(format!("curl fallback failed: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
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

            // If Cloudflare WAF returned 403 Forbidden (e.g. TLS JA3/JA4 block on rustls),
            // retry transparently using system curl (OpenSSL + HTTP/2).
            if status_u16 == 403 && !text.trim_start().starts_with('{') {
                if let Ok((code, val)) =
                    curl_fallback_json(method.as_str(), url, token, account_id, json_payload)
                {
                    if (200..300).contains(&code) || val.get("status").is_some() {
                        return Ok(val);
                    }
                }
            }

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

            // Try curl fallback one last time on 403 even if body was empty or non-LibreView JSON
            if status_u16 == 403 {
                if let Ok((code, val)) =
                    curl_fallback_json(method.as_str(), url, token, account_id, json_payload)
                {
                    if (200..300).contains(&code) || val.get("status").is_some() {
                        return Ok(val);
                    }
                }
            }

            Err(ApiError::ServerError(status_u16, status.to_string()))
        }
        Err(e) => {
            if let Ok((code, val)) =
                curl_fallback_json(method.as_str(), url, token, account_id, json_payload)
            {
                if (200..300).contains(&code) || val.get("status").is_some() {
                    return Ok(val);
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
    let region_redirect = data
        .get("region")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(normalize_country_code);
    let user_country = data
        .get("user")
        .and_then(|u| u.get("country"))
        .and_then(|c| c.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(normalize_country_code);

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

    let final_country = region_redirect
        .or(user_country)
        .unwrap_or_else(|| normalize_country_code(requested_country));

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
    })
}

pub async fn authenticate_async(
    country: &str,
    username: &str,
    password: &str,
) -> Result<AuthSuccess, ApiError> {
    let client = reqwest::Client::builder()
        .user_agent(LLU_USER_AGENT)
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| ApiError::NetworkError(e.to_string()))?;

    let initial_base = resolve_base_url(country);
    let login_url = format!("{initial_base}/auth/login");

    let payload = serde_json::json!({
        "email": username.trim(),
        "password": password,
    });

    let body = send_json_request(
        &client,
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
            &client,
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
    let client = reqwest::Client::builder()
        .user_agent(LLU_USER_AGENT)
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| ApiError::NetworkError(e.to_string()))?;

    let base = resolve_base_url(country);

    let conn_url = format!("{base}/connections");
    let conn_body = send_json_request(
        &client,
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
        &client,
        reqwest::Method::GET,
        &graph_url,
        Some(token),
        Some(account_id),
        None,
    )
    .await?;

    parse_graph_response(&graph_body, &patient_id, &first_name, &last_name)
}
