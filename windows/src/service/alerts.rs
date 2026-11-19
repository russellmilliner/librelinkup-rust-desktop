//! Alert cycle evaluation and desktop audio/toast dispatch for Windows.

use std::sync::Arc;

use librelinkup_core::event_log::{log_info, log_warn};
use librelinkup_core::models::AppSettings;
use librelinkup_core::session::evaluate_full_alert_cycle;

use super::{lock, now_epoch, Service};
use crate::alerts;

impl Service {
    /// Evaluates medication reminders, sensor expiry, and glucose/staleness thresholds using
    /// `librelinkup_core::session::evaluate_full_alert_cycle`.
    pub(super) fn evaluate_alerts(&mut self) {
        let now = now_epoch();
        let (res, settings) = {
            let mut s = lock(&self.shared);
            let settings = s.settings.clone();
            let snapshot = Arc::clone(&s.snapshot);
            let last_glucose_alert = s.last_alert;
            let last_fetch = s.last_successful_fetch_epoch;
            let logged_in = s.logged_in;
            let demo = s.demo;
            let res = evaluate_full_alert_cycle(
                &settings,
                &snapshot,
                Arc::make_mut(&mut s.medication_store),
                &mut self.medication_last_notified,
                last_glucose_alert,
                self.sensor_expiry_last_notified_epoch,
                last_fetch,
                logged_in,
                demo,
                now,
            );
            if res.meds_slots_recorded {
                Self::save_meds_if_safe(&s, false);
            }
            if res.clear_last_glucose_alert {
                s.last_alert = None;
            }
            if res.sensor_expiry_message.is_some() {
                self.sensor_expiry_last_notified_epoch = now;
            }
            if let Some(ref d) = res.glucose_alert {
                s.last_alert = Some((d.kind, now));
                s.data_rev = s.data_rev.wrapping_add(1);
            }
            (res, settings)
        };

        for r in res.due_medication_reminders {
            log_info("MEDS", "Triggered due medication reminder.");
            alerts::show_notification(&r.title, &r.body);
        }

        if res.clear_last_glucose_alert {
            log_info("ALERT", "Glucose back within alert thresholds.");
        }

        if let Some(status_str) = res.sensor_expiry_message {
            let body = format!("CGM Sensor {status_str} — prepare a replacement sensor.");
            log_warn("ALERT", format!("CGM Sensor Expiring Soon: {body}"));
            alerts::show_notification("CGM Sensor Expiring Soon", &body);
        }

        if let Some(d) = res.glucose_alert {
            log_warn("ALERT", format!("{}: {}", d.title, d.body));
            self.fire(&settings, d.title, &d.body, d.critical);
            self.notifier.state_changed();
        }
    }

    pub(super) fn fire(&self, settings: &AppSettings, title: &str, body: &str, critical: bool) {
        if settings.flash_window_enabled || settings.bring_to_front_enabled {
            self.notifier
                .attention(settings.flash_window_enabled, settings.bring_to_front_enabled);
        }
        if settings.audio_alert_enabled {
            let custom = settings.custom_sound_path.trim();
            let custom = (settings.use_custom_sound && !custom.is_empty())
                .then(|| std::path::PathBuf::from(custom))
                .filter(|p| p.is_file());
            alerts::play_alert_sound(custom);
        }
        let tx = self.tx.clone();
        alerts::show_alert_notification(title, body, critical, move |minutes| {
            let _ = tx.send(super::Command::Snooze(minutes));
        });
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use librelinkup_core::analytics::{GlucoseHistoryStore, HistorySample};
    use librelinkup_core::medications::{MedicationSchedule, MedicationStore};
    use librelinkup_core::models::{AlertKind, AppSettings, GlucoseSnapshot};
    use librelinkup_core::session::evaluate_full_alert_cycle;

    use crate::service::commands::sanitize_settings;
    use crate::service::{Service, Shared};

    #[test]
    fn demo_backup_restores_real_history_and_medications_cleanly() {
        let mut real_hist = GlucoseHistoryStore::default();
        real_hist.samples.push(HistorySample {
            epoch: 1_700_000_000,
            value_mg_dl: 112.0,
        });
        let mut real_meds = MedicationStore::default();
        real_meds
            .upsert_medication(
                "",
                "RealMed",
                "10 mg",
                MedicationSchedule::Daily,
                "08:00",
                0,
                true,
                "",
            )
            .expect("insert real med");

        let mut demo_meds = MedicationStore::default();
        demo_meds
            .upsert_medication(
                "",
                "DemoMed",
                "5 units",
                MedicationSchedule::Daily,
                "09:00",
                0,
                false,
                "",
            )
            .expect("insert demo med");

        let mut shared = Shared {
            glucose_history: Arc::new(GlucoseHistoryStore::default()),
            medication_store: Arc::new(demo_meds),
            demo: true,
            ..Default::default()
        };
        let mut backup = Some((Arc::new(real_hist), Arc::new(real_meds)));

        Service::restore_demo_backup_from(&mut backup, &mut shared);
        assert!(backup.is_none());
        assert_eq!(shared.glucose_history.samples.len(), 1);
        assert_eq!(shared.glucose_history.samples[0].value_mg_dl, 112.0);
        assert_eq!(shared.medication_store.medications[0].name, "RealMed");
    }

    #[test]
    fn sanitize_settings_clamps_poll_and_alert_thresholds() {
        let dirty = AppSettings {
            country: "FI".to_string(),
            poll_interval_seconds: 10,
            custom_target_low: 195.0,
            custom_target_high: 120.0, // invalid: below custom_target_low
            urgent_low_mg_dl: 15.0,
            alert_repeat_minutes: 0,
            stale_after_minutes: 999,
            sensor_expiry_alert_hours: 500,
            ..Default::default()
        };
        let clean = sanitize_settings(dirty);
        assert_eq!(clean.country, "eu");
        assert_eq!(clean.poll_interval_seconds, 60);
        assert_eq!(clean.custom_target_low, 195.0);
        assert!(clean.custom_target_high >= clean.custom_target_low + 10.0);
        assert_eq!(clean.urgent_low_mg_dl, 40.0);
        assert_eq!(clean.alert_repeat_minutes, 1);
        assert_eq!(clean.stale_after_minutes, 120);
        assert_eq!(clean.sensor_expiry_alert_hours, 168);
    }

    #[test]
    fn snooze_suppresses_high_low_stale_and_expiry_but_never_urgent_low() {
        let now = 1_700_000_000i64;
        let settings = AppSettings {
            snooze_until_epoch: now + 1800,
            urgent_low_mg_dl: 55.0,
            override_threshold: true,
            custom_target_low: 70.0,
            custom_target_high: 180.0,
            sensor_expiry_alert_hours: 24,
            ..Default::default()
        };
        let mut meds = MedicationStore::default();
        let mut last_meds = HashMap::new();

        // High glucose while snoozed -> suppressed (also sensor expiring -> suppressed)
        let high_snap = GlucoseSnapshot {
            value_mg_dl: 260.0,
            measured_at_epoch: Some(now - 60),
            sensor_expires_at_epoch: Some(now + 3600),
            ..Default::default()
        };
        let res_high = evaluate_full_alert_cycle(
            &settings,
            &high_snap,
            &mut meds,
            &mut last_meds,
            None,
            0,
            Some(now - 60),
            true,
            false,
            now,
        );
        assert!(res_high.glucose_alert.is_none());
        assert!(res_high.sensor_expiry_message.is_none());

        // Urgent low while snoozed -> MUST still alert
        let urgent_snap = GlucoseSnapshot {
            value_mg_dl: 52.0,
            measured_at_epoch: Some(now - 30),
            ..Default::default()
        };
        let res_urgent = evaluate_full_alert_cycle(
            &settings,
            &urgent_snap,
            &mut meds,
            &mut last_meds,
            None,
            0,
            Some(now - 30),
            true,
            false,
            now,
        );
        let alert = res_urgent.glucose_alert.expect("urgent low must bypass snooze");
        assert_eq!(alert.kind, AlertKind::UrgentLow);
        assert!(alert.critical);
    }
}
