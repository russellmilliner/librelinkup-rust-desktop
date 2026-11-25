use serde::Serialize;
use serde_json::Value;
use zeroize::Zeroizing;

use super::http::send_json_request_raw;
use super::{
    ApiError, AuthSuccess, is_valid_patient_id, normalize_country_code, resolve_base_url,
    send_json_request, shared_libreview_client, try_normalize_region_code,
};
use crate::models::{
    DEFAULT_TARGET_HIGH_MG_DL, DEFAULT_TARGET_LOW_MG_DL, GlucoseSnapshot, GraphPoint,
    parse_libreview_timestamp,
};

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

    let data = body.get("data").ok_or_else(|| {
        ApiError::ParseError("Missing 'data' field in login response".to_string())
    })?;

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
    let data = body.get("data").and_then(|v| v.as_array()).ok_or_else(|| {
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
    let data = body.get("data").ok_or_else(|| {
        ApiError::ParseError("Missing 'data' object in /graph response".to_string())
    })?;
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
            if val.is_finite() && val > 0.0 && val <= 2000.0 {
                graph_points.push(GraphPoint {
                    timestamp: ts,
                    value_mg_dl: val,
                });
            }
        }
    }
    if value_mg_dl.is_finite() && value_mg_dl > 0.0 && value_mg_dl <= 2000.0 {
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
    #[derive(Serialize)]
    struct LoginPayload<'a> {
        email: &'a str,
        password: &'a str,
    }

    let initial_base = resolve_base_url(country);
    let client = shared_libreview_client(initial_base.starts_with("http://"))?;
    let login_url = format!("{initial_base}/auth/login");

    let login_json = Zeroizing::new(
        serde_json::to_string(&LoginPayload {
            email: username.trim(),
            password,
        })
        .map_err(|e| ApiError::ParseError(format!("Failed encoding login request: {e}")))?,
    );

    let body = send_json_request_raw(
        client,
        reqwest::Method::POST,
        &login_url,
        None,
        None,
        Some(&login_json),
    )
    .await?;

    let (redirect_region, maybe_auth) = parse_login_response(&body, country)?;
    if let Some(auth) = maybe_auth {
        return Ok(auth);
    }

    if let Some(region) = redirect_region {
        let region_base = resolve_base_url(&region);
        let region_client = shared_libreview_client(region_base.starts_with("http://"))?;
        let region_url = format!("{region_base}/auth/login");
        let body2 = send_json_request_raw(
            region_client,
            reqwest::Method::POST,
            &region_url,
            None,
            None,
            Some(&login_json),
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

fn is_safe_patient_id_hint(raw: &str) -> bool {
    !raw.is_empty()
        && raw.len() <= 128
        && raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub async fn fetch_cgm_snapshot_async(
    country: &str,
    token: &str,
    account_id: &str,
) -> Result<GlucoseSnapshot, ApiError> {
    fetch_cgm_snapshot_with_patient_hint_async(country, token, account_id, None).await
}

/// Fetches the current CGM snapshot, skipping `/llu/connections` when a valid `patient_id_hint`
/// succeeds on `/llu/connections/{id}/graph` directly (falling back cleanly if invalid/empty).
pub async fn fetch_cgm_snapshot_with_patient_hint_async(
    country: &str,
    token: &str,
    account_id: &str,
    patient_id_hint: Option<&str>,
) -> Result<GlucoseSnapshot, ApiError> {
    let base = resolve_base_url(country);
    let client = shared_libreview_client(base.starts_with("http://"))?;

    if let Some(hint) = patient_id_hint
        .map(str::trim)
        .filter(|h| is_safe_patient_id_hint(h))
    {
        let graph_url = format!("{base}/connections/{hint}/graph");
        match send_json_request(
            client,
            reqwest::Method::GET,
            &graph_url,
            Some(token),
            Some(account_id),
            None,
        )
        .await
        {
            Ok(graph_body) => {
                if let Ok(snap) = parse_graph_response(&graph_body, hint, "", "")
                    && (snap.value_mg_dl > 0.0 || !snap.graph_points.is_empty())
                {
                    return Ok(snap);
                }
            }
            Err(ApiError::Unauthorized) => return Err(ApiError::Unauthorized),
            Err(_) => {}
        }
    }

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
