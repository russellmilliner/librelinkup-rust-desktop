//! Network polling, authentication, Home Assistant sync, and auto-updater tasks.

use std::sync::Arc;

use librelinkup_core::api::{
    authenticate_async, normalize_country_code, publish_ha_medications_state_async,
    publish_ha_snooze_state_async, validate_ha_base_url,
};
use librelinkup_core::crypto::{StoredCredentials, try_load_host_ha_env};
use librelinkup_core::event_log::{log_error, log_info, log_warn};
use librelinkup_core::medications::MedicationDoseLog;
use librelinkup_core::models::{format_glucose_value, save_default_settings, trend_arrow_symbol};
use librelinkup_core::session::{
    apply_ha_sync_from_states, fetch_snapshot_for_source_with_sync_async, reset_reauth_backoff,
};
use librelinkup_core::updater::{
    UpdateCheckResult, check_for_updates_async, download_and_install_update_async,
};
use zeroize::Zeroizing;

use super::{Service, lock, now_epoch};
use crate::alerts;

impl Service {
    pub(super) fn spawn_install_update(&self) {
        let (download_url, latest_version) = {
            let mut s = lock(&self.shared);
            if s.installing_update {
                return;
            }
            let Some((dl, ver)) = s
                .update_info
                .as_ref()
                .map(|i| (i.download_url.clone(), i.latest_version.clone()))
            else {
                return;
            };
            s.installing_update = true;
            (dl, ver)
        };
        self.notifier.state_changed();
        log_info(
            "SYSTEM",
            format!("Downloading and launching installer for {latest_version} ({download_url})..."),
        );

        let shared = self.shared.clone();
        let notifier = self.notifier.clone();
        tokio::spawn(async move {
            let outcome = download_and_install_update_async(&download_url, &latest_version).await;
            match outcome {
                Ok(path) => {
                    log_info(
                        "SYSTEM",
                        format!("Launched update installer: {}", path.display()),
                    );
                    {
                        let mut s = lock(&shared);
                        s.installing_update = false;
                        if let Some(ref mut info) = s.update_info {
                            info.message =
                                format!("Installer launched ({latest_version}) — upgrading...");
                        }
                    }
                    notifier.state_changed();
                }
                Err(err) => {
                    log_error("SYSTEM", format!("Auto-update install failed: {err}"));
                    {
                        let mut s = lock(&shared);
                        s.installing_update = false;
                        if let Some(ref mut info) = s.update_info {
                            info.message = err;
                        }
                    }
                    notifier.state_changed();
                }
            }
        });
    }

    pub(super) fn spawn_update_check(&self, manual: bool) {
        {
            let mut s = lock(&self.shared);
            if s.checking_updates {
                return;
            }
            s.checking_updates = true;
        }
        self.notifier.state_changed();
        if manual {
            log_info(
                "SYSTEM",
                "Checking GitHub Releases for application updates...",
            );
        }

        let shared = self.shared.clone();
        let notifier = self.notifier.clone();
        tokio::spawn(async move {
            let outcome = check_for_updates_async(env!("CARGO_PKG_VERSION")).await;
            match outcome {
                Ok(res) => {
                    let update_available = res.update_available;
                    let latest = res.latest_version.clone();
                    let current = res.current_version.clone();
                    let url = res.release_url.clone();
                    let msg = res.message.clone();
                    let should_notify = {
                        let mut s = lock(&shared);
                        s.checking_updates = false;
                        let already = s.last_notified_update_version == latest;
                        if update_available && (manual || !already) {
                            s.last_notified_update_version = latest.clone();
                            s.update_info = Some(res);
                            true
                        } else {
                            s.update_info = Some(res);
                            false
                        }
                    };
                    notifier.state_changed();
                    if update_available {
                        log_info(
                            "SYSTEM",
                            format!(
                                "Update available: {latest} (current {current}). Release: {url}"
                            ),
                        );
                        if should_notify {
                            alerts::show_notification(
                                "LibreLinkUp Update Available",
                                &format!(
                                    "Version {latest} is available on GitHub Releases (you have {current})."
                                ),
                            );
                        }
                    } else if manual {
                        log_info("SYSTEM", format!("Update check complete: {msg}"));
                        alerts::show_notification("LibreLinkUp Up to Date", &msg);
                    }
                }
                Err(err) => {
                    {
                        let mut s = lock(&shared);
                        s.checking_updates = false;
                        s.update_info = Some(UpdateCheckResult {
                            checked: true,
                            update_available: false,
                            current_version: format!("v{}", env!("CARGO_PKG_VERSION")),
                            latest_version: String::new(),
                            release_url: librelinkup_core::updater::GITHUB_RELEASES_PAGE_URL
                                .to_string(),
                            download_url: librelinkup_core::updater::GITHUB_RELEASES_PAGE_URL
                                .to_string(),
                            sha256_url: String::new(),
                            signature_url: String::new(),
                            release_notes: String::new(),
                            message: err.clone(),
                        });
                    }
                    notifier.state_changed();
                    if manual {
                        log_warn("SYSTEM", format!("Update check failed: {err}"));
                    }
                }
            }
        });
    }

    pub(super) async fn login(
        &mut self,
        username: String,
        mut password: Zeroizing<String>,
        country: String,
    ) -> bool {
        let username = username.trim().to_string();
        if username.len() < 2 || password.len() < 2 {
            self.update(|s| {
                s.error = "Please enter both your LibreLinkUp email and password.".into()
            });
            return false;
        }
        let country = if country.trim().is_empty() {
            "us".to_string()
        } else {
            normalize_country_code(&country)
        };
        reset_reauth_backoff();
        log_info(
            "AUTH",
            format!("Attempting LibreLinkUp login (region: {country})"),
        );
        self.update(|s| {
            s.busy = true;
            s.error.clear();
            s.status = "Signing in to LibreLinkUp...".into();
        });
        let result = authenticate_async(&country, &username, &password).await;
        match result {
            Ok(mut auth) => {
                let remember_pw = lock(&self.shared).settings.remember_password;
                let existing_ha_token = self
                    .creds
                    .as_ref()
                    .map(|c| c.ha_token.clone())
                    .unwrap_or_default();
                let auth_country = std::mem::take(&mut auth.country);
                let creds = StoredCredentials {
                    username: username.clone(),
                    password: std::mem::take(&mut *password),
                    token: std::mem::take(&mut auth.token),
                    account_id: std::mem::take(&mut auth.account_id),
                    country: auth_country.clone(),
                    language: "en".into(),
                    patient_id: String::new(),
                    first_name: std::mem::take(&mut auth.first_name),
                    last_name: std::mem::take(&mut auth.last_name),
                    ha_token: existing_ha_token,
                }
                .sanitized_for_storage(remember_pw);
                let has_ha_tok = creds.has_valid_ha_session();
                self.save_vault(&creds);
                self.creds = Some(creds);
                log_info(
                    "AUTH",
                    format!("Login succeeded (region '{auth_country}')."),
                );
                let country_for_settings = auth_country;
                self.update_restore_demo(|s| {
                    s.busy = false;
                    s.logged_in = true;
                    s.demo = false;
                    s.vault_ok = true;
                    s.ha_token_configured = has_ha_tok;
                    s.username = username;
                    s.settings.data_source = "librelinkup".into();
                    s.settings.country = country_for_settings;
                    s.status = "Signed in — fetching glucose data...".into();
                });
                let settings = lock(&self.shared).settings.clone();
                let _ = save_default_settings(&settings);
                true
            }
            Err(err) => {
                let msg = err.to_string();
                log_error("AUTH", format!("Login failed: {msg}"));
                self.update(|s| {
                    s.busy = false;
                    s.error = msg;
                    s.status = "Authentication failed".into();
                });
                false
            }
        }
    }

    pub(super) fn save_ha_config(
        &mut self,
        data_source: String,
        ha_url: String,
        ha_token: Zeroizing<String>,
        ha_sensor_entity_id: String,
    ) -> bool {
        let clean_source = if data_source.trim().eq_ignore_ascii_case("home_assistant") {
            "home_assistant".to_string()
        } else {
            "librelinkup".to_string()
        };
        let clean_url = ha_url.trim().trim_end_matches('/').to_string();
        let clean_entity = ha_sensor_entity_id.trim().to_string();
        let clean_token = Zeroizing::new(ha_token.trim().to_string());

        if clean_source == "home_assistant"
            && !clean_url.is_empty()
            && let Err(e) = validate_ha_base_url(&clean_url)
        {
            self.update(|s| s.error = e.to_string());
            return false;
        }

        let mut creds = self.creds.clone().unwrap_or_default();
        if !clean_token.is_empty() {
            creds.ha_token = (*clean_token).clone();
        }
        let has_ha_tok = creds.has_valid_ha_session();
        if clean_source == "home_assistant" && (clean_url.is_empty() || !has_ha_tok) {
            self.update(|s| {
                s.settings.data_source = clean_source.clone();
                s.settings.ha_url = clean_url.clone();
                s.settings.ha_sensor_entity_id = clean_entity.clone();
                s.error =
                    "Please enter both your Home Assistant URL and Long-Lived Access Token.".into();
            });
            return false;
        }

        self.save_vault(&creds);
        let valid_for_source = creds.has_valid_session_for(&clean_source, &clean_url);
        let has_any = creds.has_valid_llu_session() || has_ha_tok;
        self.creds = Some(creds);
        reset_reauth_backoff();

        let settings = {
            let mut s = lock(&self.shared);
            Self::restore_demo_backup_from(&mut self.demo_backup, &mut s);
            s.settings.data_source = clean_source.clone();
            s.settings.ha_url = clean_url;
            s.settings.ha_sensor_entity_id = clean_entity;
            s.ha_token_configured = has_ha_tok;
            s.vault_ok = has_any;
            s.logged_in = valid_for_source;
            s.demo = false;
            s.error.clear();
            s.status = if clean_source == "home_assistant" {
                "Connected to Home Assistant — fetching LibreLink telemetry...".into()
            } else if valid_for_source {
                "LibreLinkUp Cloud active".into()
            } else {
                "Please sign in to LibreLinkUp".into()
            };
            s.data_rev = s.data_rev.wrapping_add(1);
            s.settings.clone()
        };
        let _ = save_default_settings(&settings);
        self.notifier.state_changed();
        valid_for_source
    }

    pub(super) fn auto_fill_ha_env(&mut self) -> bool {
        let Some((env_url, raw_token)) = try_load_host_ha_env() else {
            self.update(|s| {
                s.error =
                    "Could not find HA_URL and HA_TOKEN in ~/.config/home-assistant.env".into();
            });
            return false;
        };
        let env_token = Zeroizing::new(raw_token);
        let entity = lock(&self.shared).settings.ha_sensor_entity_id.clone();
        log_info(
            "SYSTEM",
            format!(
                "Imported Home Assistant URL ({env_url}) and token from ~/.config/home-assistant.env"
            ),
        );
        self.save_ha_config("home_assistant".into(), env_url, env_token, entity)
    }

    pub(super) fn spawn_ha_medications_push(&self, last_logged_dose: Option<MedicationDoseLog>) {
        let Some(creds) = self.creds.clone() else {
            return;
        };
        if !creds.has_valid_ha_session() {
            return;
        }
        let (ha_url, sync_enabled, store) = {
            let s = lock(&self.shared);
            (
                s.settings.ha_url.trim().to_string(),
                s.settings.ha_sync_medications,
                s.medication_store.clone(),
            )
        };
        if !sync_enabled || ha_url.is_empty() {
            return;
        }
        tokio::spawn(async move {
            if let Err(err) = publish_ha_medications_state_async(
                &ha_url,
                &creds.ha_token,
                &store,
                last_logged_dose.as_ref(),
            )
            .await
            {
                log_warn(
                    "MEDS",
                    format!("Failed to sync medications to Home Assistant: {err}"),
                );
            } else {
                log_info(
                    "MEDS",
                    "Synced medication schedule & dose log to Home Assistant",
                );
            }
        });
    }

    pub(super) fn spawn_ha_snooze_push(&self, snooze_until_epoch: i64, updated_at_epoch: i64) {
        let Some(creds) = self.creds.clone() else {
            return;
        };
        if !creds.has_valid_ha_session() {
            return;
        }
        let (ha_url, sync_enabled) = {
            let s = lock(&self.shared);
            (
                s.settings.ha_url.trim().to_string(),
                s.settings.ha_sync_snooze,
            )
        };
        if !sync_enabled || ha_url.is_empty() {
            return;
        }
        tokio::spawn(async move {
            if let Err(err) = publish_ha_snooze_state_async(
                &ha_url,
                &creds.ha_token,
                snooze_until_epoch,
                updated_at_epoch,
            )
            .await
            {
                log_warn(
                    "ALERT",
                    format!("Failed to sync alert snooze to Home Assistant: {err}"),
                );
            } else {
                log_info(
                    "ALERT",
                    format!(
                        "Synced alert snooze state to Home Assistant (until={snooze_until_epoch})"
                    ),
                );
            }
        });
    }

    pub(super) async fn refresh(&mut self) {
        let Some(creds) = self.creds.clone() else {
            return;
        };
        let (data_source, ha_url, ha_entity_id, sync_meds, sync_snooze) = {
            let s = lock(&self.shared);
            (
                s.settings.data_source.clone(),
                s.settings.ha_url.clone(),
                s.settings.ha_sensor_entity_id.clone(),
                s.settings.ha_sync_medications,
                s.settings.ha_sync_snooze,
            )
        };
        if !creds.has_valid_session_for(&data_source, &ha_url) {
            return;
        }
        self.update(|s| s.busy = true);

        match fetch_snapshot_for_source_with_sync_async(
            &data_source,
            &ha_url,
            &ha_entity_id,
            &creds,
            sync_meds,
            sync_snooze,
        )
        .await
        {
            Ok(mut outcome) => {
                if let Some(updated_creds) = outcome.updated_credentials.take() {
                    self.save_vault(&updated_creds);
                    self.creds = Some(updated_creds);
                }
                let snap = outcome.snapshot;
                let ha_states = outcome.ha_states;
                let unit = lock(&self.shared).settings.result_unit.clone();
                log_info(
                    "GLUCOSE",
                    format!(
                        "Fetched glucose reading: {} {} {} ({} points)",
                        format_glucose_value(snap.value_mg_dl, &unit),
                        unit,
                        trend_arrow_symbol(snap.trend_arrow),
                        snap.graph_points.len()
                    ),
                );
                let now = now_epoch();
                let mut push_meds = false;
                self.update_restore_demo(|s| {
                    s.busy = false;
                    s.error.clear();
                    s.demo = false;
                    s.last_successful_fetch_epoch = Some(now);
                    if Arc::make_mut(&mut s.glucose_history).ingest_snapshot(&snap, now)
                        && !s.history_vault_load_failed
                        && now - s.last_history_persist_epoch
                            >= librelinkup_core::analytics::HISTORY_PERSIST_MIN_INTERVAL_SECS
                    {
                        Self::save_history_if_safe(s, now, false);
                    }
                    if let Some(ref states) = ha_states {
                        let ha_sync = apply_ha_sync_from_states(
                            states,
                            Arc::make_mut(&mut s.medication_store),
                            s.settings.ha_sync_medications,
                            s.settings.ha_sync_snooze,
                            s.settings.snooze_until_epoch,
                            s.settings.snooze_updated_at_epoch,
                            now,
                        );
                        s.discovered_ha_patients = ha_sync.discovered_patients;
                        if ha_sync.meds_merged {
                            Self::save_meds_if_safe(s, false);
                            log_info(
                                "MEDS",
                                "Merged remote medication schedule/dose updates from Home Assistant",
                            );
                        }
                        push_meds = ha_sync.should_push_meds;
                        if let Some((effective_until, remote_updated_at)) = ha_sync.updated_snooze {
                            s.settings.snooze_until_epoch = effective_until;
                            s.settings.snooze_updated_at_epoch = remote_updated_at;
                            if effective_until == 0 {
                                s.last_alert = None;
                            }
                            let _ = save_default_settings(&s.settings);
                            log_info(
                                "ALERT",
                                format!(
                                    "Applied cross-device alert snooze from Home Assistant (until={})",
                                    s.settings.snooze_until_epoch
                                ),
                            );
                        }
                    }
                    s.snapshot = Arc::new(snap);
                    s.status = "Live CGM telemetry active".into();
                });
                if push_meds {
                    self.spawn_ha_medications_push(None);
                }
            }
            Err(err) => {
                let msg = err.to_string();
                log_warn("API", format!("Failed to fetch CGM data: {msg}"));
                self.update(|s| {
                    s.busy = false;
                    s.error = msg;
                });
            }
        }
        self.evaluate_alerts();
    }
}
