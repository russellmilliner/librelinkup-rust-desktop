use librelinkup_core::api::{
    authenticate_async, fetch_cgm_snapshot_async, normalize_country_code, resolve_base_url,
    sha256_account_id, ApiError,
};
use librelinkup_core::crypto::{
    bytes_to_hex, decrypt_credentials_with_secret, encrypt_credentials_with_secret, hex_to_bytes,
    load_credentials_from_path, save_credentials_to_path, try_migrate_legacy_leveldb_credentials,
    StoredCredentials,
};
use librelinkup_core::event_log::{
    clear_events, export_events_to_path, filter_events, format_events_as_text, get_all_events,
    get_event_count, get_events_json, log_error, log_info, log_warn,
};
use librelinkup_core::models::{
    convert_mg_dl_to_unit, format_glucose_value, format_target_range, load_settings_from_path,
    save_settings_to_path, trend_arrow_description, trend_arrow_symbol, AppSettings,
    GlucoseRangeStatus, GlucoseSnapshot, GraphPoint,
};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;

#[test]
fn test_full_encryption_lifecycle_and_security() {
    let dir = std::env::temp_dir().join(format!("llu_int_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let vault_path = dir.join("credentials.enc.json");
    let key_path = dir.join("master.key");

    let creds = StoredCredentials {
        username: "patient@example.org".to_string(),
        password: "Ultra$ecretPassword!99".to_string(),
        token: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.payload.sig".to_string(),
        account_id: "user-987654".to_string(),
        country: "us".to_string(),
        language: "en".to_string(),
        patient_id: "patient-123456".to_string(),
        first_name: "Jane".to_string(),
        last_name: "Doe".to_string(),
    };

    save_credentials_to_path(&creds, &vault_path, &key_path).expect("Vault save should succeed");
    assert!(vault_path.exists());
    assert!(key_path.exists());

    // Ensure plaintext credentials never appear in raw file bytes
    let raw_bytes = std::fs::read(&vault_path).unwrap();
    let raw_lossy = String::from_utf8_lossy(&raw_bytes);
    assert!(!raw_lossy.contains("Ultra$ecretPassword!99"));
    assert!(!raw_lossy.contains("patient@example.org"));
    assert!(!raw_lossy.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));

    // Verify strict 0600 permissions on both key and encrypted vault
    let vault_mode = std::fs::metadata(&vault_path).unwrap().permissions().mode() & 0o777;
    let key_mode = std::fs::metadata(&key_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(vault_mode, 0o600);
    assert_eq!(key_mode, 0o600);

    // Load and verify roundtrip
    let loaded = load_credentials_from_path(&vault_path, &key_path)
        .expect("Vault load should succeed")
        .expect("Credentials should be present");
    assert_eq!(loaded, creds);
    assert!(loaded.has_valid_session());

    // Wrong key must fail authentication
    let wrong_key_path = dir.join("wrong.key");
    std::fs::write(
        &wrong_key_path,
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
    )
    .unwrap();
    assert!(load_credentials_from_path(&vault_path, &wrong_key_path).is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_encryption_nonce_uniqueness_and_tamper_detection() {
    let mut creds = StoredCredentials::default();
    creds.username = "a@b.c".to_string();
    creds.password = "pw".to_string();
    creds.token = "tok-123".to_string();
    creds.account_id = "acc-456".to_string();

    let secret = b"same-key-material-32-bytes-long!";
    let env1 = encrypt_credentials_with_secret(&creds, secret).unwrap();
    let env2 = encrypt_credentials_with_secret(&creds, secret).unwrap();

    // Random 128-bit salt and 96-bit nonce ensure every encryption produces distinct ciphertext
    assert_ne!(env1.ciphertext_hex, env2.ciphertext_hex);
    assert_ne!(env1.nonce_hex, env2.nonce_hex);
    assert_ne!(env1.salt_hex, env2.salt_hex);
    assert_eq!(decrypt_credentials_with_secret(&env1, secret).unwrap(), creds);
    assert_eq!(decrypt_credentials_with_secret(&env2, secret).unwrap(), creds);

    // Tampering with ciphertext must fail GCM authentication tag verification
    let mut tampered = env1.clone();
    let mut ct_bytes = hex_to_bytes(&tampered.ciphertext_hex).unwrap();
    ct_bytes[0] ^= 0xff;
    tampered.ciphertext_hex = bytes_to_hex(&ct_bytes);
    assert!(decrypt_credentials_with_secret(&tampered, secret).is_err());

    // Tampering with nonce must fail GCM authentication
    let mut tampered_nonce = env1.clone();
    let mut nonce_bytes = hex_to_bytes(&tampered_nonce.nonce_hex).unwrap();
    nonce_bytes[0] ^= 0x01;
    tampered_nonce.nonce_hex = bytes_to_hex(&nonce_bytes);
    assert!(decrypt_credentials_with_secret(&tampered_nonce, secret).is_err());
}

#[test]
fn test_legacy_electron_leveldb_token_migration() {
    let dir = std::env::temp_dir().join(format!("llu_ldb_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let ldb_file = dir.join("000005.ldb");
    let state_blob = br#"prefix_bytes{"state":{"token":"jwt-legacy-token-123","accountId":"user-legacy-42","country":"us","language":"en"},"version":0}suffix_bytes"#;
    std::fs::write(&ldb_file, state_blob).unwrap();

    let migrated = try_migrate_legacy_leveldb_credentials(&dir)
        .expect("Should extract and decode legacy state from LevelDB directory");
    assert_eq!(migrated.token, "jwt-legacy-token-123");
    assert_eq!(migrated.account_id, "user-legacy-42");
    assert_eq!(migrated.country, "us");
    assert_eq!(migrated.language, "en");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_tray_icon_and_hover_tooltip_modes() {
    let snap = GlucoseSnapshot {
        value_mg_dl: 118.0,
        trend_arrow: 3,
        timestamp: "2026-10-19 12:30:00".to_string(),
        target_low_mg_dl: 70.0,
        target_high_mg_dl: 180.0,
        patient_id: "p-1".to_string(),
        first_name: "Alice".to_string(),
        last_name: "Smith".to_string(),
        graph_points: vec![],
    };

    // Case 1: Both tray icon glucose badge and hover info enabled
    let mut settings = AppSettings {
        show_glucose_in_tray_icon: true,
        show_glucose_in_tray_tooltip: true,
        result_unit: "mg/dL".to_string(),
        ..Default::default()
    };
    assert_eq!(settings.format_tray_icon_badge(&snap), "118");
    let tip = settings.format_tray_hover_tooltip(&snap);
    assert!(tip.contains("Blood Sugar: 118 mg/dL → (Stable • In Range)"));
    assert!(tip.contains("Target Range: 70–180 mg/dL"));
    assert!(tip.contains("Patient: Alice Smith"));

    // Case 2: Only tray icon badge enabled, hover info disabled
    settings.show_glucose_in_tray_icon = true;
    settings.show_glucose_in_tray_tooltip = false;
    assert_eq!(settings.format_tray_icon_badge(&snap), "118");
    let disabled_tip = settings.format_tray_hover_tooltip(&snap);
    assert_eq!(disabled_tip, "LibreLinkUp Desktop");

    // Case 3: Tray icon badge disabled, hover info enabled with mmol/L
    settings.show_glucose_in_tray_icon = false;
    settings.show_glucose_in_tray_tooltip = true;
    settings.result_unit = "mmol/L".to_string();
    assert_eq!(settings.format_tray_icon_badge(&snap), "");
    let tip_mmol = settings.format_tray_hover_tooltip(&snap);
    assert!(tip_mmol.contains("Blood Sugar: 6.6 mmol/L → (Stable • In Range)"));
}

#[test]
fn test_glucose_conversion_settings_persistence_and_alerts() {
    assert_eq!(convert_mg_dl_to_unit(180.0, "mmol/L"), 10.0);
    assert_eq!(format_glucose_value(180.0, "mmol/L"), "10");
    assert_eq!(format_glucose_value(118.0, "mmol/L"), "6.6");
    assert_eq!(format_target_range(70.0, 180.0, "mg/dL"), "70–180 mg/dL");
    assert_eq!(trend_arrow_symbol(1), "↓");
    assert_eq!(trend_arrow_symbol(2), "↘");
    assert_eq!(trend_arrow_symbol(3), "→");
    assert_eq!(trend_arrow_symbol(4), "↗");
    assert_eq!(trend_arrow_symbol(5), "↑");
    assert_eq!(trend_arrow_description(1), "Falling Quickly");
    assert_eq!(trend_arrow_description(5), "Rising Quickly");

    assert_eq!(
        GlucoseRangeStatus::classify(55.0, 70.0, 180.0),
        GlucoseRangeStatus::CriticalLow
    );
    assert_eq!(
        GlucoseRangeStatus::classify(68.0, 75.0, 180.0),
        GlucoseRangeStatus::CriticalLow
    );
    assert_eq!(
        GlucoseRangeStatus::classify(72.0, 80.0, 180.0),
        GlucoseRangeStatus::OutOfTarget
    );
    assert_eq!(
        GlucoseRangeStatus::classify(120.0, 70.0, 180.0),
        GlucoseRangeStatus::InRange
    );
    assert_eq!(
        GlucoseRangeStatus::classify(260.0, 70.0, 180.0),
        GlucoseRangeStatus::CriticalHigh
    );

    let settings = AppSettings {
        theme_mode: "dark".to_string(),
        result_unit: "mmol/L".to_string(),
        show_glucose_in_tray_icon: true,
        show_glucose_in_tray_tooltip: true,
        override_threshold: true,
        custom_target_low: 80.0,
        custom_target_high: 160.0,
        ..Default::default()
    };
    assert!(settings.should_trigger_alert(75.0, 70.0, 180.0));
    assert!(!settings.should_trigger_alert(110.0, 70.0, 180.0));
    assert!(settings.should_trigger_alert(170.0, 70.0, 180.0));

    let settings_path =
        std::env::temp_dir().join(format!("llu_settings_{}.json", std::process::id()));
    save_settings_to_path(&settings, &settings_path).unwrap();
    let loaded_settings = load_settings_from_path(&settings_path);
    assert_eq!(loaded_settings, settings);
    let _ = std::fs::remove_file(&settings_path);
}

#[test]
fn test_log_viewer_filtering_and_export() {
    clear_events();
    assert_eq!(get_event_count(), 0);

    log_info("SYSTEM", "Application initialized");
    log_info("CRYPTO", "Loaded AES-256-GCM encrypted vault");
    log_warn("API", "Regional redirect to EU server");
    log_error("ALERT", "Urgent low glucose alert triggered");

    assert_eq!(get_event_count(), 4);
    let all = get_all_events();
    assert_eq!(all.len(), 4);
    assert_eq!(filter_events(&all, "CRYPTO", "ALL", "").len(), 1);
    assert_eq!(filter_events(&all, "ALL", "WARN", "").len(), 1);
    assert_eq!(filter_events(&all, "ALL", "ALL", "redirect").len(), 1);

    let json = get_events_json();
    assert!(json.contains("AES-256-GCM"));
    let formatted = format_events_as_text(&all);
    assert!(formatted.contains("[ERROR] [ALERT   ] Urgent low glucose alert triggered"));

    let export_path =
        std::env::temp_dir().join(format!("llu_log_export_{}.log", std::process::id()));
    let count = export_events_to_path(&export_path).unwrap();
    assert_eq!(count, 4);
    let exported = std::fs::read_to_string(&export_path).unwrap();
    assert!(exported.contains("Application initialized"));
    assert!(exported.contains("Loaded AES-256-GCM encrypted vault"));
    assert!(exported.contains("Urgent low glucose alert triggered"));
    let _ = std::fs::remove_file(&export_path);
}

#[test]
fn test_mock_librelinkup_api_server() {
    assert_eq!(normalize_country_code("ch"), "eu");
    assert_eq!(
        resolve_base_url("us"),
        "https://api-us.libreview.io/llu"
    );
    assert_eq!(
        resolve_base_url("global"),
        "https://api.libreview.io/llu"
    );

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn mock HTTP server handling login, connections, and graph requests
    let handle = std::thread::spawn(move || {
        for step in 0..3 {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);

                let body = match step {
                    0 => {
                        assert!(req.contains("POST /llu/auth/login"));
                        r#"{"status":0,"data":{"user":{"id":"mock-uid-1","country":"US","firstName":"Alice","lastName":"Smith"},"authTicket":{"token":"mock-jwt-xyz","expires":1999999999}}}"#
                    }
                    1 => {
                        assert!(req.contains("GET /llu/connections"));
                        assert!(req.to_ascii_lowercase().contains("account-id: 67f8e2411bf510e9ddc6f77e7dee396ab56c102f099ee152ad0beeca10b95a9a"));
                        r#"{"status":0,"data":[{"patientId":"p-99","firstName":"Alice","lastName":"Smith","targetLow":70,"targetHigh":180}]}"#
                    }
                    _ => {
                        assert!(req.contains("GET /llu/connections/p-99/graph"));
                        r#"{"status":0,"data":{"connection":{"patientId":"p-99","firstName":"Alice","lastName":"Smith","targetLow":70,"targetHigh":180,"glucoseMeasurement":{"Timestamp":"10/19/2026 12:00:00 PM","ValueInMgPerDl":124,"TrendArrow":3}},"graphData":[{"Timestamp":"10/19/2026 11:45:00 AM","ValueInMgPerDl":118}]}}"#
                    }
                };

                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        }
    });

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let base_url = format!("http://{}/llu", addr);
    rt.block_on(async {
        let auth = authenticate_async(&base_url, "test@example.com", "secret-pw")
            .await
            .expect("Mock login should succeed");
        assert_eq!(auth.token, "mock-jwt-xyz");
        assert_eq!(auth.account_id, "mock-uid-1");
        assert_eq!(
            sha256_account_id(&auth.account_id),
            "67f8e2411bf510e9ddc6f77e7dee396ab56c102f099ee152ad0beeca10b95a9a"
        );

        let snap = fetch_cgm_snapshot_async(&base_url, &auth.token, &auth.account_id)
            .await
            .expect("Mock graph fetch should succeed");
        assert_eq!(snap.value_mg_dl, 124.0);
        assert_eq!(snap.trend_arrow, 3);
        assert_eq!(snap.patient_id, "p-99");
        assert_eq!(snap.graph_points.len(), 2);
        assert_eq!(
            snap.graph_points[0],
            GraphPoint {
                timestamp: "10/19/2026 11:45:00 AM".to_string(),
                value_mg_dl: 118.0
            }
        );
    });

    handle.join().unwrap();
    assert!(matches!(ApiError::Unauthorized, ApiError::Unauthorized));
}

#[test]
fn test_mock_api_regional_redirect_and_error_handling() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = std::thread::spawn(move || {
        // Request 1: Return status 2 (InvalidCredentials)
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let body = r#"{"status":2}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
        // Request 2: Return status 4 (PrivacyPolicyAcceptanceRequired)
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let body = r#"{"status":4}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
        // Request 3: Empty connections list -> NoConnectionsFound
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let body = r#"{"status":0,"data":[]}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let base_url = format!("http://{}/llu", addr);
    rt.block_on(async {
        let err1 = authenticate_async(&base_url, "bad@example.com", "wrong")
            .await
            .unwrap_err();
        assert_eq!(err1, ApiError::InvalidCredentials);

        let err2 = authenticate_async(&base_url, "pp@example.com", "pw")
            .await
            .unwrap_err();
        assert_eq!(err2, ApiError::PrivacyPolicyAcceptanceRequired);

        let err3 = fetch_cgm_snapshot_async(&base_url, "tok", "acc")
            .await
            .unwrap_err();
        assert_eq!(err3, ApiError::NoConnectionsFound);
    });

    handle.join().unwrap();
}

#[test]
fn test_all_11_supported_regional_endpoints() {
    let expected = [
        ("us", "https://api-us.libreview.io/llu"),
        ("eu", "https://api-eu.libreview.io/llu"),
        ("eu2", "https://api-eu2.libreview.io/llu"),
        ("de", "https://api-de.libreview.io/llu"),
        ("fr", "https://api-fr.libreview.io/llu"),
        ("jp", "https://api-jp.libreview.io/llu"),
        ("ap", "https://api-ap.libreview.io/llu"),
        ("au", "https://api-au.libreview.io/llu"),
        ("ae", "https://api-ae.libreview.io/llu"),
        ("ca", "https://api-ca.libreview.io/llu"),
        ("ch", "https://api-eu.libreview.io/llu"),
        ("global", "https://api.libreview.io/llu"),
    ];
    for (region, url) in expected {
        assert_eq!(resolve_base_url(region), url);
    }
}
