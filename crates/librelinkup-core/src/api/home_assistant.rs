use std::sync::Mutex;

use serde_json::Value;

use super::{
    send_ha_json_request, shared_ha_client, ApiError,
};
use crate::models::{
    parse_libreview_timestamp, GlucoseSnapshot, GraphPoint, DEFAULT_TARGET_HIGH_MG_DL,
    DEFAULT_TARGET_LOW_MG_DL,
};

// ---------------------------------------------------------------------------
// Home Assistant LibreLink Integration REST API Support
// ---------------------------------------------------------------------------

/// Validates a Home Assistant `entity_id` against `^[a-z0-9_]+\.[a-z0-9_]+$` and normalizes
/// ASCII uppercase letters to lowercase. Rejects characters that could inject URL query
/// parameters (`&`, `?`, `#`, `/`, whitespace) when querying `/api/history/period`.
pub fn validate_ha_entity_id(raw: &str) -> Result<String, ApiError> {
    let clean = raw.trim().to_ascii_lowercase();
    if clean.is_empty() || clean.len() > 255 {
        return Err(ApiError::ParseError(format!(
            "Invalid Home Assistant entity_id '{raw}'"
        )));
    }
    let Some((domain, object_id)) = clean.split_once('.') else {
        return Err(ApiError::ParseError(format!(
            "Invalid Home Assistant entity_id '{raw}'"
        )));
    };
    let valid_part = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    };
    if !valid_part(domain) || !valid_part(object_id) {
        return Err(ApiError::ParseError(format!(
            "Invalid Home Assistant entity_id '{raw}'"
        )));
    }
    Ok(clean)
}

/// Returns `true` when `host_port` targets localhost, a local LAN hostname/address,
/// RFC1918/ULA private space, or Tailscale (`100.64.0.0/10` / `.ts.net`).
/// Parses through [`reqwest::Url`] first so integer/hex/octal IPv4 forms are normalized.
pub fn is_private_or_local_ha_host(host_port: &str) -> bool {
    let normalized = reqwest::Url::parse(&format!("http://{}", host_port.trim()))
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| host_port.trim().to_string());

    let host = if let Some(after_open) = normalized.strip_prefix('[') {
        after_open.split(']').next().unwrap_or(after_open)
    } else {
        normalized.split(':').next().unwrap_or("")
    }
    .trim()
    .to_ascii_lowercase();

    if host.is_empty() {
        return false;
    }
    if host == "localhost"
        || host.ends_with(".local")
        || host.ends_with(".lan")
        || host.ends_with(".home")
        || host.ends_with(".arpa")
        || host.ends_with(".internal")
        || host.ends_with(".ts.net")
    {
        return true;
    }
    let ip_candidate = host.split('%').next().unwrap_or(&host);
    if let Ok(ip) = ip_candidate.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => {
                let octets = v4.octets();
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || (octets[0] == 100 && (64..=127).contains(&octets[1]))
            }
            std::net::IpAddr::V6(v6) => {
                if let Some(mapped) = v6.to_ipv4_mapped() {
                    let octets = mapped.octets();
                    return mapped.is_loopback()
                        || mapped.is_private()
                        || mapped.is_link_local()
                        || (octets[0] == 100 && (64..=127).contains(&octets[1]));
                }
                let seg = v6.segments();
                v6.is_loopback()
                    || v6.is_unicast_link_local()
                    || (seg[0] & 0xfe00) == 0xfc00
            }
        };
    }
    if host.bytes().all(|b| b.is_ascii_digit())
        || host.starts_with("0x")
        || host.starts_with("0o")
    {
        return false;
    }
    !host.contains('.') && !host.contains(':')
}

/// Validates and normalizes a Home Assistant base URL (e.g. `"http://homeassistant.local:8123"`
/// or `"https://homeassistant.example.com"`). Strips any trailing `/` or `/api`.
pub fn validate_ha_base_url(raw_url: &str) -> Result<String, ApiError> {
    if raw_url.chars().any(|c| c.is_control()) {
        return Err(ApiError::ParseError(
            "Invalid control characters in Home Assistant URL".to_string(),
        ));
    }
    let mut trimmed = raw_url.trim().trim_end_matches('/');
    if let Some(stripped) = trimmed.strip_suffix("/api") {
        trimmed = stripped.trim_end_matches('/');
    }
    if trimmed.is_empty() {
        return Err(ApiError::ParseError(
            "Home Assistant URL is empty. Enter your Home Assistant URL (e.g. http://homeassistant.local:8123)."
                .to_string(),
        ));
    }
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return Err(ApiError::ParseError(
            "Home Assistant URL must start with https:// or http://".to_string(),
        ));
    }
    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|e| ApiError::ParseError(format!("Invalid Home Assistant URL: {e}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ApiError::ParseError(
            "Home Assistant URL must use http:// or https://".to_string(),
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(ApiError::ParseError(
            "Credentials are not allowed in the Home Assistant URL".to_string(),
        ));
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(ApiError::ParseError(
            "Query strings and fragments are not allowed in the Home Assistant URL".to_string(),
        ));
    }
    let Some(host_str) = parsed.host_str() else {
        return Err(ApiError::ParseError(
            "Home Assistant URL is missing a host".to_string(),
        ));
    };
    let host_port = match parsed.port() {
        Some(p) => format!("{host_str}:{p}"),
        None => host_str.to_string(),
    };

    if parsed.scheme() == "http" && !is_private_or_local_ha_host(&host_port) {
        static LAST_WARNED_HTTP_HOST: Mutex<String> = Mutex::new(String::new());
        let should_warn = LAST_WARNED_HTTP_HOST
            .lock()
            .map(|mut prev| {
                if prev.as_str() != host_port {
                    *prev = host_port.clone();
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);
        if should_warn {
            crate::event_log::log_warn(
                "SECURITY",
                format!(
                    "Home Assistant URL ({host_port}) uses unencrypted http:// over a non-local network; prefer https:// or Tailscale to protect your Long-Lived Access Token."
                ),
            );
        }
    }
    Ok(parsed.as_str().trim_end_matches('/').to_string())
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
    if let Some(epoch) = parse_libreview_timestamp(trimmed, true)
        && let Some(dt) = chrono::DateTime::from_timestamp(epoch, 0) {
            let local_str = dt
                .with_timezone(&chrono::Local)
                .format("%m/%d/%Y %I:%M:%S %p")
                .to_string();
            return Some((epoch, local_str));
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
        if patient_name.is_empty()
            && let Some(friendly) = attrs
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
        let hint = validate_ha_entity_id(hint)?;
        if let Some(found) = states_arr.iter().find(|item| {
            item.get("entity_id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| id.eq_ignore_ascii_case(&hint))
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
    parse_ha_states_slice_and_history(states_arr, history_body, entity_id_hint)
}

fn parse_ha_states_slice_and_history(
    states_arr: &[Value],
    history_body: Option<&Value>,
    entity_id_hint: &str,
) -> Result<GlucoseSnapshot, ApiError> {

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
    if !value_mg_dl.is_finite() || value_mg_dl <= 0.0 || value_mg_dl > 2000.0 {
        return Err(ApiError::ParseError(format!(
            "Home Assistant glucose sensor '{entity_id}' reported out-of-range reading"
        )));
    }

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
    })
        && let Some(st) = ent.get("state").and_then(|v| v.as_str()) {
            trend_arrow = parse_ha_trend_str(st);
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
                if let Some(arr_str) = ent_attrs.get("trend_arrow").and_then(|v| v.as_str())
                    && let Some(code) = parse_ha_trend_str(arr_str) {
                        trend_arrow = Some(code);
                        break;
                    }
                if let Some(cat_str) = ent_attrs.get("trend_category").and_then(|v| v.as_str())
                    && let Some(code) = parse_ha_trend_str(cat_str) {
                        trend_arrow = Some(code);
                        break;
                    }
            }
            if let Some(st) = ent.get("state").and_then(|v| v.as_str())
                && let Some(code) = parse_ha_trend_str(st) {
                    trend_arrow = Some(code);
                    break;
                }
        }
    }

    // Fallback to primary sensor's own attributes (`trend_arrow` or `icon`)
    if trend_arrow.is_none()
        && let Some(a) = attrs {
            if let Some(s) = a.get("trend_arrow").and_then(|v| v.as_str()) {
                trend_arrow = parse_ha_trend_str(s);
            }
            if trend_arrow.is_none()
                && let Some(icon) = a.get("icon").and_then(|v| v.as_str()) {
                    trend_arrow = parse_ha_trend_str(icon);
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
        if let Some(st) = ent.get("state").and_then(|v| v.as_str())
            && let Some(parsed) = format_rfc3339_as_local_libreview(st) {
                ts_pair = Some(parsed);
                break;
            }
    }
    if ts_pair.is_none()
        && let Some(lu) = primary
            .get("last_updated")
            .and_then(|v| v.as_str())
            .or_else(|| primary.get("last_changed").and_then(|v| v.as_str()))
        {
            ts_pair = format_rfc3339_as_local_libreview(lu);
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
            if patient_id.is_empty()
                && let Some(pid) = a.get("Patient ID").and_then(|v| v.as_str()) {
                    patient_id = pid.trim().to_string();
                }
            if full_name.is_empty()
                && let Some(pname) = a.get("Patient").and_then(|v| v.as_str()) {
                    full_name = pname.trim().to_string();
                }
            if sensor_serial.is_empty()
                && let Some(sn) = a.get("Serial number").and_then(|v| v.as_str()) {
                    sensor_serial = sn.trim().to_string();
                }
            if sensor_activated_at_epoch.is_none()
                && let Some(act_str) = a.get("Activation date").and_then(|v| v.as_str()) {
                    sensor_activated_at_epoch = parse_libreview_timestamp(act_str, true);
                }
            if sensor_lifespan_days <= 0
                && let Some(days) = a.get("Sensor lifespan days").and_then(|v| v.as_i64()) {
                    sensor_lifespan_days = days.clamp(1, 60) as i32;
                }
            if let Some(plus) = a.get("Sensor plus model").and_then(|v| v.as_bool()) {
                sensor_plus_model = plus;
            }
        }
    }
    if sensor_expires_at_epoch.is_none()
        && let Some(act) = sensor_activated_at_epoch {
            let days = if sensor_lifespan_days > 0 {
                i64::from(sensor_lifespan_days)
            } else if sensor_plus_model {
                15
            } else {
                14
            };
            sensor_expires_at_epoch = Some(act + days * 86_400);
        }
    if full_name.is_empty()
        && let Some(friendly) = attrs
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
            if let Some(low) = a.get("target_low_mgdl").and_then(|v| v.as_f64())
                && low > 0.0 {
                    target_low_mg_dl = low;
                }
            if let Some(high) = a.get("target_high_mgdl").and_then(|v| v.as_f64())
                && high > 0.0 {
                    target_high_mg_dl = high;
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
            if !mg_dl.is_finite() || mg_dl <= 0.0 || mg_dl > 2000.0 {
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
    let client = shared_ha_client()?;
    let states_url = format!("{base}/api/states");
    let states_body = send_ha_json_request(
        client,
        reqwest::Method::GET,
        &states_url,
        token,
        None,
    )
    .await?;
    if let Value::Array(arr) = states_body {
        Ok(arr)
    } else {
        Err(ApiError::ParseError(
            "Home Assistant /api/states did not return a JSON array".to_string(),
        ))
    }
}

async fn fetch_optional_ha_entity_state(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    entity_id: &str,
) -> Result<Option<Value>, ApiError> {
    let url = format!("{base}/api/states/{entity_id}");
    match send_ha_json_request(client, reqwest::Method::GET, &url, token, None).await {
        Ok(val) => Ok(Some(val)),
        Err(ApiError::ServerError(404, _)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Fetches ONLY the specific Home Assistant entities required for cross-device medications and/or
/// alert snooze synchronization instead of downloading the entire `/api/states` array.
pub async fn fetch_ha_sync_states_async(
    ha_url: &str,
    ha_token: &str,
    sync_meds: bool,
    sync_snooze: bool,
) -> Result<Vec<Value>, ApiError> {
    let base = validate_ha_base_url(ha_url)?;
    let token = ha_token.trim();
    if token.is_empty() {
        return Err(ApiError::InvalidCredentials);
    }
    if !sync_meds && !sync_snooze {
        return Ok(Vec::new());
    }
    let client = shared_ha_client()?;
    let meds_fut = async {
        if sync_meds {
            fetch_optional_ha_entity_state(client, &base, token, "sensor.librelinkup_medications")
                .await
        } else {
            Ok(None)
        }
    };
    let snooze_sensor_fut = async {
        if sync_snooze {
            fetch_optional_ha_entity_state(
                client,
                &base,
                token,
                "sensor.librelinkup_alert_snooze",
            )
            .await
        } else {
            Ok(None)
        }
    };
    let snooze_dt_fut = async {
        if sync_snooze {
            fetch_optional_ha_entity_state(
                client,
                &base,
                token,
                "input_datetime.librelinkup_snooze_until",
            )
            .await
        } else {
            Ok(None)
        }
    };

    let (meds_res, snooze_res, dt_res) = tokio::join!(meds_fut, snooze_sensor_fut, snooze_dt_fut);
    let mut out = Vec::with_capacity(3);
    for res in [meds_res, snooze_res, dt_res] {
        match res {
            Ok(Some(v)) => out.push(v),
            Ok(None) => {}
            Err(ApiError::Unauthorized) => return Err(ApiError::Unauthorized),
            Err(_) => {}
        }
    }
    Ok(out)
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
    let client = shared_ha_client()?;
    let state_url = format!("{base}/api/states/sensor.librelinkup_alert_snooze");
    send_ha_json_request(
        client,
        reqwest::Method::POST,
        &state_url,
        token,
        Some(&payload),
    )
    .await?;

    let event_url = format!("{base}/api/events/librelinkup_alert_snooze_changed");
    let event_payload = serde_json::json!({
        "snoozed": snooze_until_epoch > now,
        "snooze_until_epoch": snooze_until_epoch.max(0),
        "updated_at_epoch": updated_at_epoch,
    });
    let _ = send_ha_json_request(
        client,
        reqwest::Method::POST,
        &event_url,
        token,
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
    let client = shared_ha_client()?;
    let state_url = format!("{base}/api/states/sensor.librelinkup_medications");
    send_ha_json_request(
        client,
        reqwest::Method::POST,
        &state_url,
        token,
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
        let _ = send_ha_json_request(
            client,
            reqwest::Method::POST,
            &event_url,
            token,
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

    let client = shared_ha_client()?;
    let states_url = format!("{base}/api/states");
    let states_body = send_ha_json_request(
        client,
        reqwest::Method::GET,
        &states_url,
        token,
        None,
    )
    .await?;

    let Value::Array(states_arr) = states_body else {
        return Err(ApiError::ParseError(
            "Home Assistant /api/states did not return a JSON array".to_string(),
        ));
    };
    let primary = find_ha_glucose_entity(&states_arr, entity_id_hint)?;
    let matched_entity_id = primary
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let history_body = if let Ok(safe_entity_id) = validate_ha_entity_id(&matched_entity_id) {
        let start_iso = (chrono::Utc::now() - chrono::Duration::hours(12))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        let hist_url = format!(
            "{base}/api/history/period/{start_iso}?filter_entity_id={safe_entity_id}&minimal_response&no_attributes"
        );
        send_ha_json_request(
            client,
            reqwest::Method::GET,
            &hist_url,
            token,
            None,
        )
        .await
        .ok()
    } else {
        None
    };

    let snap = parse_ha_states_slice_and_history(&states_arr, history_body.as_ref(), entity_id_hint)?;
    Ok((snap, states_arr))
}
