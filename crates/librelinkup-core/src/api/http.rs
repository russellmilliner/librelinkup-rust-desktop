use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::header::{
    ACCEPT, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, HeaderMap, HeaderValue, PRAGMA, USER_AGENT,
};
use ring::digest::{SHA256, digest};
use serde_json::Value;
use zeroize::{Zeroize, Zeroizing};

use super::{
    ApiError, LLU_PRODUCT, LLU_USER_AGENT, LLU_VERSION, insecure_test_endpoint_allowed,
    is_loopback_test_url,
};
use crate::crypto::bytes_to_hex;

pub(super) const API_RESPONSE_MAX_BYTES: usize = 4 * 1024 * 1024;
pub(super) const HA_RESPONSE_MAX_BYTES: usize = 8 * 1024 * 1024;

static SHARED_LIBREVIEW_HTTPS_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static SHARED_LIBREVIEW_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static SHARED_HA_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Returns a reusable `reqwest::Client` configured for LibreView API calls.
/// Enforces HTTPS-only and disables HTTP redirects so bearer tokens are never forwarded.
pub fn shared_libreview_client(allow_http: bool) -> Result<&'static reqwest::Client, ApiError> {
    let slot = if allow_http {
        &SHARED_LIBREVIEW_HTTP_CLIENT
    } else {
        &SHARED_LIBREVIEW_HTTPS_CLIENT
    };
    if let Some(client) = slot.get() {
        return Ok(client);
    }
    let built = reqwest::Client::builder()
        .user_agent(LLU_USER_AGENT)
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .https_only(!allow_http)
        .build()
        .map_err(|e| ApiError::NetworkError(e.to_string()))?;
    let _ = slot.set(built);
    slot.get()
        .ok_or_else(|| ApiError::NetworkError("Failed to initialize HTTP client".into()))
}

/// Returns the process-wide reusable `reqwest::Client` for Home Assistant REST calls.
/// Disables HTTP redirects so the Long-Lived Access Token is never redirected off-origin.
pub fn shared_ha_client() -> Result<&'static reqwest::Client, ApiError> {
    if let Some(client) = SHARED_HA_CLIENT.get() {
        return Ok(client);
    }
    let built = reqwest::Client::builder()
        .user_agent(format!(
            "librelinkup-rust-desktop/{}",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| ApiError::NetworkError(e.to_string()))?;
    let _ = SHARED_HA_CLIENT.set(built);
    SHARED_HA_CLIENT
        .get()
        .ok_or_else(|| ApiError::NetworkError("Failed to initialize HA HTTP client".into()))
}

/// Returns the process-wide reusable `reqwest::Client` for LibreView API calls.
pub fn shared_api_client() -> Result<&'static reqwest::Client, ApiError> {
    shared_libreview_client(insecure_test_endpoint_allowed())
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
        let mut auth_val = format!("Bearer {}", tok.trim());
        let mut hv = HeaderValue::from_str(&auth_val).map_err(|e| {
            ApiError::ParseError(format!("Invalid authorization token header: {e}"))
        })?;
        auth_val.zeroize();
        hv.set_sensitive(true);
        headers.insert(AUTHORIZATION, hv);
    }

    if let Some(acc) = account_id {
        let hashed = sha256_account_id(acc);
        let mut hv = HeaderValue::from_str(&hashed)
            .map_err(|e| ApiError::ParseError(format!("Invalid Account-Id header: {e}")))?;
        hv.set_sensitive(true);
        headers.insert("Account-Id", hv);
    }

    Ok(headers)
}

pub(super) fn build_ha_headers(ha_token: &str) -> Result<HeaderMap, ApiError> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let mut auth_val = format!("Bearer {}", ha_token.trim());
    let mut hv = HeaderValue::from_str(&auth_val)
        .map_err(|e| ApiError::ParseError(format!("Invalid HA authorization header: {e}")))?;
    auth_val.zeroize();
    hv.set_sensitive(true);
    headers.insert(AUTHORIZATION, hv);
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
    let raw_opt = json_payload.map(|b| b.to_string());
    (*build_curl_secret_config_raw(token, account_id, raw_opt.as_deref())).clone()
}

fn build_curl_secret_config_raw(
    token: Option<&str>,
    account_id: Option<&str>,
    raw_json_body: Option<&str>,
) -> Zeroizing<String> {
    let mut cfg = String::new();
    if let Some(tok) = token {
        let mut header = format!("Authorization: Bearer {}", tok.trim());
        cfg.push_str(&format!(
            "header = \"{}\"\n",
            escape_curl_config_value(&header)
        ));
        header.zeroize();
    }
    if let Some(acc) = account_id {
        let header = format!("Account-Id: {}", sha256_account_id(acc));
        cfg.push_str(&format!(
            "header = \"{}\"\n",
            escape_curl_config_value(&header)
        ));
    }
    if let Some(body) = raw_json_body {
        let mut escaped = escape_curl_config_value(body);
        cfg.push_str("data-raw = \"");
        cfg.push_str(&escaped);
        cfg.push_str("\"\n");
        escaped.zeroize();
    }
    Zeroizing::new(cfg)
}

pub(super) async fn read_json_response_text_bounded(
    mut resp: reqwest::Response,
    max_bytes: usize,
) -> Result<String, ApiError> {
    if let Some(len) = resp.content_length()
        && len > max_bytes as u64
    {
        return Err(ApiError::ParseError(format!(
            "API response too large ({len} bytes > {max_bytes} byte limit)"
        )));
    }
    let cap = resp
        .content_length()
        .and_then(|l| usize::try_from(l).ok())
        .map_or(0, |l| l.min(max_bytes));
    let mut buf = Vec::with_capacity(cap);
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| ApiError::NetworkError(format!("Failed reading API response stream: {e}")))?
    {
        if buf.len().saturating_add(chunk.len()) > max_bytes {
            return Err(ApiError::ParseError(format!(
                "API response stream exceeded {max_bytes} byte limit"
            )));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8(buf)
        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

fn run_curl_blocking(method: &str, url: &str, secret_config: &str) -> Result<String, ApiError> {
    use std::io::Write;
    use std::process::Stdio;

    let curl_prog = crate::updater::system_curl_program()
        .ok_or_else(|| ApiError::NetworkError("System curl is unavailable".into()))?;
    let mut cmd = Command::new(curl_prog);
    #[cfg(windows)]
    {
        // GUI app: don't flash a console window for the curl fallback.
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let proto_spec = if is_loopback_test_url(url) {
        "=https,http"
    } else {
        "=https"
    };
    let max_size_str = API_RESPONSE_MAX_BYTES.to_string();
    let mut child = cmd
        .arg("-s")
        .arg("-S")
        .arg("--proto")
        .arg(proto_spec)
        .arg("--proto-redir")
        .arg("=https")
        .arg("--max-filesize")
        .arg(&max_size_str)
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

    if let Some(mut stdin) = child.stdin.take()
        && let Err(e) = stdin.write_all(secret_config.as_bytes())
    {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ApiError::NetworkError(format!(
            "curl fallback stdin error: {e}"
        )));
    }
    // stdin dropped here -> EOF, curl proceeds with the request.

    let output = child
        .wait_with_output()
        .map_err(|e| ApiError::NetworkError(format!("curl fallback failed: {e}")))?;
    if output.stdout.len() > API_RESPONSE_MAX_BYTES + 64 {
        return Err(ApiError::ParseError(format!(
            "curl API response exceeded {API_RESPONSE_MAX_BYTES} byte limit"
        )));
    }
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
    raw_json_body: Option<&str>,
) -> Result<(u16, Value), ApiError> {
    let method_owned = method.to_string();
    let url_owned = url.to_string();
    let mut secret_config = build_curl_secret_config_raw(token, account_id, raw_json_body);

    let stdout = tokio::task::spawn_blocking(move || {
        let res = run_curl_blocking(&method_owned, &url_owned, &secret_config);
        secret_config.zeroize();
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
        return Err(if token.is_some() {
            ApiError::Unauthorized
        } else {
            ApiError::InvalidCredentials
        });
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

pub(super) async fn send_json_request_raw(
    client: &reqwest::Client,
    method: reqwest::Method,
    url: &str,
    token: Option<&str>,
    account_id: Option<&str>,
    raw_json_body: Option<&str>,
) -> Result<Value, ApiError> {
    let headers = build_common_headers(token, account_id)?;
    let mut req = client.request(method.clone(), url).headers(headers);
    if let Some(body) = raw_json_body {
        req = req.body(body.as_bytes().to_vec());
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let status_u16 = status.as_u16();
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(if token.is_some() {
                    ApiError::Unauthorized
                } else {
                    ApiError::InvalidCredentials
                });
            }

            let text = read_json_response_text_bounded(resp, API_RESPONSE_MAX_BYTES)
                .await
                .unwrap_or_default();

            if let Ok(json_val) = serde_json::from_str::<Value>(&text)
                && (status.is_success() || json_val.get("status").is_some())
            {
                return Ok(json_val);
            }

            // Cloudflare WAF returned 403 Forbidden without a LibreView JSON body
            // (e.g. TLS JA3/JA4 block on rustls): retry once via system curl (OpenSSL + HTTP/2).
            if status_u16 == 403
                && crate::updater::system_curl_available()
                && let Ok((code, val)) =
                    curl_fallback_json(method.as_str(), url, token, account_id, raw_json_body).await
                && ((200..300).contains(&code) || val.get("status").is_some())
            {
                return Ok(val);
            }

            Err(ApiError::ServerError(status_u16, status.to_string()))
        }
        Err(e) => {
            // Timeouts and connection failures (offline, DNS) would fail identically via
            // curl and only add another 15 s of waiting, so skip the fallback for them.
            if !e.is_timeout()
                && !e.is_connect()
                && crate::updater::system_curl_available()
                && let Ok((code, val)) =
                    curl_fallback_json(method.as_str(), url, token, account_id, raw_json_body).await
                && ((200..300).contains(&code) || val.get("status").is_some())
            {
                return Ok(val);
            }
            Err(ApiError::NetworkError(e.to_string()))
        }
    }
}

pub(super) async fn send_json_request(
    client: &reqwest::Client,
    method: reqwest::Method,
    url: &str,
    token: Option<&str>,
    account_id: Option<&str>,
    json_payload: Option<&Value>,
) -> Result<Value, ApiError> {
    let raw_body = json_payload.map(|p| p.to_string());
    send_json_request_raw(client, method, url, token, account_id, raw_body.as_deref()).await
}

pub(super) async fn send_ha_json_request(
    client: &reqwest::Client,
    method: reqwest::Method,
    url: &str,
    ha_token: &str,
    json_payload: Option<&Value>,
) -> Result<Value, ApiError> {
    let headers = build_ha_headers(ha_token)?;
    let mut req = client.request(method, url).headers(headers);
    if let Some(payload) = json_payload {
        req = req.json(payload);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| ApiError::NetworkError(e.to_string()))?;
    let status = resp.status();
    let status_u16 = status.as_u16();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(ApiError::Unauthorized);
    }
    let text = read_json_response_text_bounded(resp, HA_RESPONSE_MAX_BYTES).await?;
    if !status.is_success() {
        return Err(ApiError::ServerError(status_u16, status.to_string()));
    }
    serde_json::from_str::<Value>(&text)
        .map_err(|e| ApiError::ParseError(format!("Invalid Home Assistant JSON response: {e}")))
}
