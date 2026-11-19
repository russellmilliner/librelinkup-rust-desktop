use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

mod home_assistant;
mod http;
mod libreview;

pub use home_assistant::*;
pub use http::*;
pub use libreview::*;

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

impl Drop for AuthSuccess {
    fn drop(&mut self) {
        self.token.zeroize();
        self.account_id.zeroize();
        self.first_name.zeroize();
        self.last_name.zeroize();
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

impl std::error::Error for ApiError {}

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

    #[test]
    fn ha_entity_id_validation_blocks_query_injection() {
        assert_eq!(
            validate_ha_entity_id("sensor.alex_patient_measurement").unwrap(),
            "sensor.alex_patient_measurement"
        );
        assert_eq!(
            validate_ha_entity_id("  Sensor.Glucose_1  ").unwrap(),
            "sensor.glucose_1"
        );

        for bad in [
            "sensor.foo&bar=1",
            "sensor.foo?x=1",
            "sensor.foo#frag",
            "sensor.foo/bar",
            "sensor..bad",
            "nodot",
            "",
        ] {
            assert!(
                validate_ha_entity_id(bad).is_err(),
                "expected '{bad}' to be rejected"
            );
        }
    }

    #[test]
    fn ha_private_or_local_host_classification() {
        for local in [
            "homeassistant.local:8123",
            "haos.dingo-dory.ts.net",
            "192.168.0.32:8123",
            "100.84.233.88:8123",
            "127.0.0.1:8123",
            "[::1]:8123",
            "localhost:8123",
            "homeassistant:8123",
        ] {
            assert!(
                is_private_or_local_ha_host(local),
                "expected '{local}' to be classified as private/local"
            );
        }

        for remote in [
            "example.com:8123",
            "8.8.8.8:8123",
            "100.128.0.1:8123",
            "134744072:8123",
            "0x08080808:8123",
        ] {
            assert!(
                !is_private_or_local_ha_host(remote),
                "expected '{remote}' to be classified as remote"
            );
        }

        assert!(validate_ha_base_url("https://user:pass@example.com").is_err());
        assert!(validate_ha_base_url("http://example.com?foo=bar").is_err());
        assert_eq!(
            validate_ha_base_url("http://homeassistant.local:8123/api/").unwrap(),
            "http://homeassistant.local:8123"
        );
    }

    #[test]
    fn ssrf_and_url_validation_covers_ipv6_mapped_schemes_and_regions() {
        for bad_url in [
            "file:///etc/passwd",
            "ftp://ha.local:8123",
            "javascript:alert(1)",
            "http://admin:secret@homeassistant.local:8123",
            "https://token@homeassistant.local",
            "https://homeassistant.local#fragment",
            "http://192.168.1.50:8123\nX-Injected: 1",
        ] {
            assert!(
                validate_ha_base_url(bad_url).is_err(),
                "expected URL '{bad_url}' to be rejected"
            );
        }

        // IPv6 loopback, ULA (fc00::/7), link-local (fe80::/10), IPv4-mapped local & link-local,
        // and obfuscated integer/hex/octal loopback forms normalized by URL parser
        for local_ip in [
            "[::1]:8123",
            "[fd12:3456:789a::1]:8123",
            "[fe80::1%eth0]:8123",
            "[::ffff:192.168.1.10]:8123",
            "[::ffff:169.254.169.254]:80",
            "169.254.169.254:80",
            "2130706433:80",
            "0x7f000001:80",
            "0177.0.0.1:80",
        ] {
            assert!(
                is_private_or_local_ha_host(local_ip),
                "expected '{local_ip}' to classify as local/link-local"
            );
        }

        // Public IPv6 and obfuscated public IPv4 forms do not get trusted as local LAN
        for non_local in [
            "[2001:db8::1]:8123",
            "134744072:80",
            "0x08080808:80",
        ] {
            assert!(
                !is_private_or_local_ha_host(non_local),
                "expected '{non_local}' NOT to classify as local"
            );
        }

        // Region allowlist resists smuggling
        for bad_region in [
            "us.attacker.com",
            "https://us.attacker.com",
            "us/../../etc",
            "global@evil.com",
        ] {
            assert_eq!(resolve_base_url(bad_region), "https://api.libreview.io/llu");
        }
    }
}
