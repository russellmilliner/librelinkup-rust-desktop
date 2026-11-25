//! Command dispatch handlers for `Service`.

use std::sync::Arc;
use std::time::Duration;

use librelinkup_core::analytics::GlucoseHistoryStore;
use librelinkup_core::api::normalize_country_code;
use librelinkup_core::crypto::clear_credentials_at_path;
use librelinkup_core::event_log::{log_error, log_info};
use librelinkup_core::medications::{MedicationDoseLog, build_demo_medication_store};
use librelinkup_core::models::{
    AppSettings, GlucoseSnapshot, clamp_poll_interval, save_default_settings,
};
use librelinkup_core::paths::credentials_file_path;
use librelinkup_core::session::{build_demo_snapshot, reset_reauth_backoff};
use tokio::time::Instant;

use super::{Command, Service, lock, now_epoch};
use crate::platform;

pub(crate) fn sanitize_settings(mut settings: AppSettings) -> AppSettings {
    settings.country = normalize_country_code(&settings.country);
    settings.poll_interval_seconds = clamp_poll_interval(settings.poll_interval_seconds);
    settings.custom_target_low = settings.custom_target_low.clamp(40.0, 200.0);
    settings.custom_target_high = settings
        .custom_target_high
        .clamp(settings.custom_target_low + 10.0, 400.0);
    settings.urgent_low_mg_dl = settings.urgent_low_mg_dl.clamp(40.0, 100.0);
    settings.alert_repeat_minutes = settings.alert_repeat_minutes.clamp(1, 240);
    settings.stale_after_minutes = settings.stale_after_minutes.clamp(5, 120);
    settings.sensor_expiry_alert_hours = settings.sensor_expiry_alert_hours.clamp(0, 168);
    settings
}

impl Service {
    /// Returns true if an immediate refresh should follow.
    pub(super) async fn handle(&mut self, cmd: Command) -> bool {
        match cmd {
            Command::Login {
                username,
                password,
                country,
            } => self.login(username, password, country).await,
            Command::SaveHomeAssistantConfig {
                data_source,
                ha_url,
                ha_token,
                ha_sensor_entity_id,
            } => self.save_ha_config(data_source, ha_url, ha_token, ha_sensor_entity_id),
            Command::AutoFillHomeAssistantFromEnv => self.auto_fill_ha_env(),
            Command::SelectHaPatient(entity_id) => {
                let clean = entity_id.trim().to_string();
                let should_refresh = {
                    let mut s = lock(&self.shared);
                    s.settings.ha_sensor_entity_id = clean.clone();
                    let _ = save_default_settings(&s.settings);
                    s.settings.data_source == "home_assistant" && s.logged_in && !s.demo
                };
                log_info(
                    "API",
                    format!(
                        "Selected Home Assistant patient sensor entity: '{}'",
                        if clean.is_empty() {
                            "auto-discover"
                        } else {
                            &clean
                        }
                    ),
                );
                self.notifier.state_changed();
                should_refresh
            }
            Command::Logout => {
                let now = now_epoch();
                let _ = clear_credentials_at_path(&credentials_file_path());
                self.creds = None;
                reset_reauth_backoff();
                log_info("AUTH", "Signed out; encrypted vault deleted.");
                self.update_restore_demo(|s| {
                    if !s.glucose_history.samples.is_empty() {
                        Self::save_history_if_safe(s, now, true);
                    }
                    s.logged_in = false;
                    s.demo = false;
                    s.vault_ok = false;
                    s.ha_token_configured = false;
                    s.username.clear();
                    s.snapshot = Arc::new(GlucoseSnapshot::default());
                    s.last_alert = None;
                    s.last_successful_fetch_epoch = None;
                    s.error.clear();
                    s.status = "Signed out — encrypted vault cleared.".into();
                });
                false
            }
            Command::RefreshNow => true,
            Command::SaveSettings(settings) => {
                let settings = sanitize_settings(*settings);
                let previous = lock(&self.shared).settings.clone();
                if previous.autostart_enabled != settings.autostart_enabled
                    || previous.start_minimized != settings.start_minimized
                {
                    let enabled = settings.autostart_enabled;
                    match platform::set_autostart(enabled, settings.start_minimized) {
                        Ok(()) => log_info("SYSTEM", format!("Start at login set to {enabled}.")),
                        Err(e) => log_error("SYSTEM", format!("Failed to update autostart: {e}")),
                    }
                }
                let mut country_changed = false;
                let remember_changed = previous.remember_password != settings.remember_password;
                if let Some(ref mut creds) = self.creds {
                    if creds.country != settings.country {
                        log_info(
                            "AUTH",
                            format!(
                                "Updated active regional endpoint '{}' -> '{}'",
                                creds.country, settings.country
                            ),
                        );
                        creds.country = settings.country.clone();
                        country_changed = true;
                    }
                    if !settings.remember_password && !creds.password.is_empty() {
                        *creds = creds.sanitized_for_storage(false);
                    }
                }
                self.update(|s| s.settings = settings.clone());
                self.pending_settings_save =
                    Some((Instant::now() + Duration::from_millis(400), settings));
                if (country_changed || remember_changed)
                    && let Some(ref creds) = self.creds
                {
                    self.save_vault(creds);
                }
                self.evaluate_alerts();
                country_changed
            }
            Command::Snooze(minutes) => {
                let now = now_epoch();
                let until = if minutes > 0 {
                    now + i64::from(minutes.min(1440)) * 60
                } else {
                    0
                };
                let settings = {
                    let mut s = lock(&self.shared);
                    s.settings.snooze_until_epoch = until;
                    s.settings.snooze_updated_at_epoch = now;
                    if until == 0 {
                        s.last_alert = None; // re-arm
                    }
                    s.settings.clone()
                };
                let _ = save_default_settings(&settings);
                self.spawn_ha_snooze_push(until, now);
                if until > 0 {
                    log_info(
                        "ALERT",
                        format!("Alerts snoozed for {minutes} min (urgent low still alerts)."),
                    );
                } else {
                    log_info("ALERT", "Alert snooze cancelled.");
                }
                self.notifier.state_changed();
                self.evaluate_alerts();
                false
            }
            Command::TestAlert => {
                log_info("ALERT", "Triggered manual alert test.");
                let settings = lock(&self.shared).settings.clone();
                self.fire(
                    &settings,
                    "LibreLinkUp alert test",
                    "This is how out-of-range glucose alerts will look and sound.",
                    true,
                );
                false
            }
            Command::LoadDemo => {
                let now = now_epoch();
                let snap = build_demo_snapshot(now);
                {
                    let mut s = lock(&self.shared);
                    if !s.demo && self.demo_backup.is_none() {
                        Self::save_history_if_safe(&mut s, now, false);
                        self.demo_backup = Some((
                            Arc::clone(&s.glucose_history),
                            Arc::clone(&s.medication_store),
                        ));
                        s.glucose_history = Arc::new(GlucoseHistoryStore::default());
                        s.medication_store = Arc::new(build_demo_medication_store(now));
                    }
                    s.demo = true;
                    s.logged_in = true;
                    s.status = "Sample CGM telemetry loaded".into();
                    Arc::make_mut(&mut s.glucose_history).ingest_snapshot(&snap, now);
                    if s.medication_store.medications.is_empty() {
                        s.medication_store = Arc::new(build_demo_medication_store(now));
                    }
                    s.snapshot = Arc::new(snap);
                    s.data_rev = s.data_rev.wrapping_add(1);
                }
                self.notifier.state_changed();
                false
            }
            Command::CheckUpdates { manual } => {
                self.spawn_update_check(manual);
                false
            }
            Command::InstallUpdate => {
                self.spawn_install_update();
                false
            }
            Command::SetStatsWindow(win) => {
                log_info(
                    "GLUCOSE",
                    format!("Selected glycemic stats window: {}", win.label()),
                );
                self.update(|s| {
                    s.stats_window = win;
                    s.settings.stats_window = win.code().to_string();
                    let _ = save_default_settings(&s.settings);
                });
                false
            }
            Command::UpsertMedication {
                id,
                name,
                dosage,
                schedule,
                scheduled_time,
                weekday,
                reminder_enabled,
                notes,
            } => {
                let mut changed = false;
                let mut push_ha = false;
                self.update(|s| {
                    if Arc::make_mut(&mut s.medication_store)
                        .upsert_medication(
                            &id,
                            &name,
                            &dosage,
                            schedule,
                            &scheduled_time,
                            weekday,
                            reminder_enabled,
                            &notes,
                        )
                        .is_ok()
                    {
                        Self::save_meds_if_safe(s, false);
                        log_info(
                            "MEDS",
                            format!("Saved scheduled medication ({}).", schedule.label()),
                        );
                        changed = true;
                        push_ha = !s.demo;
                    }
                });
                if changed && push_ha {
                    self.spawn_ha_medications_push(None);
                }
                false
            }
            Command::RemoveMedication(id) => {
                let mut changed = false;
                let mut push_ha = false;
                self.update(|s| {
                    if Arc::make_mut(&mut s.medication_store).remove_medication(&id) {
                        Self::save_meds_if_safe(s, false);
                        log_info("MEDS", "Removed scheduled medication.");
                        changed = true;
                        push_ha = !s.demo;
                    }
                });
                if changed && push_ha {
                    self.spawn_ha_medications_push(None);
                }
                false
            }
            Command::LogMedicationDose {
                medication_id,
                custom_dosage,
                notes,
            } => {
                let now = now_epoch();
                let mut logged_entry: Option<MedicationDoseLog> = None;
                let mut push_ha = false;
                self.update(|s| {
                    let g = (s.snapshot.value_mg_dl > 0.0).then_some(s.snapshot.value_mg_dl);
                    let cd = (!custom_dosage.trim().is_empty()).then_some(custom_dosage.as_str());
                    if let Ok(entry) = Arc::make_mut(&mut s.medication_store).log_dose(
                        &medication_id,
                        cd,
                        now,
                        g,
                        &notes,
                    ) {
                        Self::save_meds_if_safe(s, false);
                        log_info(
                            "MEDS",
                            format!("Logged medication dose at {}.", entry.taken_at_display),
                        );
                        push_ha = !s.demo;
                        logged_entry = Some(entry);
                    }
                });
                if push_ha && logged_entry.is_some() {
                    self.spawn_ha_medications_push(logged_entry);
                }
                false
            }
            Command::SkipMedicationDose {
                medication_id,
                notes,
            } => {
                let now = now_epoch();
                let mut skipped_entry: Option<MedicationDoseLog> = None;
                let mut push_ha = false;
                self.update(|s| {
                    if let Ok(entry) = Arc::make_mut(&mut s.medication_store).skip_dose(
                        &medication_id,
                        now,
                        &notes,
                    ) {
                        Self::save_meds_if_safe(s, false);
                        log_info(
                            "MEDS",
                            format!("Skipped medication dose at {}.", entry.taken_at_display),
                        );
                        push_ha = !s.demo;
                        skipped_entry = Some(entry);
                    }
                });
                if push_ha && skipped_entry.is_some() {
                    self.spawn_ha_medications_push(skipped_entry);
                }
                false
            }
            Command::RemoveMedicationDose(dose_id) => {
                let mut changed = false;
                let mut push_ha = false;
                self.update(|s| {
                    if Arc::make_mut(&mut s.medication_store).remove_dose_log(&dose_id) {
                        Self::save_meds_if_safe(s, false);
                        log_info("MEDS", "Removed logged medication dose entry.");
                        changed = true;
                        push_ha = !s.demo;
                    }
                });
                if changed && push_ha {
                    self.spawn_ha_medications_push(None);
                }
                false
            }
        }
    }
}
