//! Medication schedule definitions, dose logging, 12-hour graph markers, and
//! due/overdue desktop reminder evaluation shared by KDE Plasma and Windows.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;

use chrono::{Datelike, Local, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};

use crate::crypto::{
    decrypt_bytes_with_secret, encrypt_bytes_with_secret, phi_secret, AAD_LABEL_MEDICATIONS,
};
use crate::event_log::{log_error, log_info};
use crate::models::{parse_libreview_timestamp, GlucoseSnapshot};
use crate::paths::{
    legacy_medications_file_path, medications_file_path, write_file_atomic_0600,
};

/// Minimum seconds between repeated desktop reminder notifications for the same medication.
pub const MEDICATION_REMINDER_REPEAT_SECONDS: i64 = 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MedicationSchedule {
    #[default]
    Daily,
    TwiceDaily,
    Weekly,
    AsNeeded,
}

impl MedicationSchedule {
    pub fn from_code(code: &str) -> Self {
        match code.trim().to_ascii_lowercase().as_str() {
            "daily" | "once_daily" | "1x" => Self::Daily,
            "twicedaily" | "twice_daily" | "twice daily" | "2x" | "bid" => Self::TwiceDaily,
            "weekly" | "once_weekly" | "1w" => Self::Weekly,
            "asneeded" | "as_needed" | "as needed" | "prn" => Self::AsNeeded,
            _ => Self::Daily,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::TwiceDaily => "twice_daily",
            Self::Weekly => "weekly",
            Self::AsNeeded => "as_needed",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::TwiceDaily => "Twice Daily",
            Self::Weekly => "Weekly",
            Self::AsNeeded => "As Needed",
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MedicationDef {
    pub id: String,
    pub name: String,
    pub dosage: String,
    pub schedule: MedicationSchedule,
    /// Scheduled 24-hour time(s), e.g. `"08:00"` for Daily/Weekly or `"08:00,20:00"` for TwiceDaily.
    pub scheduled_time: String,
    /// Day of week (`0` = Monday .. `6` = Sunday) when `schedule == Weekly`.
    pub weekday: u8,
    pub reminder_enabled: bool,
    pub notes: String,
    pub enabled: bool,
}

impl Default for MedicationDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            dosage: String::new(),
            schedule: MedicationSchedule::Daily,
            scheduled_time: "08:00".to_string(),
            weekday: 0,
            reminder_enabled: true,
            notes: String::new(),
            enabled: true,
        }
    }
}

impl fmt::Debug for MedicationDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MedicationDef")
            .field("id", &self.id)
            .field("name", &"[REDACTED]")
            .field("dosage", &"[REDACTED]")
            .field("schedule", &self.schedule)
            .field("scheduled_time", &self.scheduled_time)
            .finish()
    }
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MedicationDoseLog {
    pub id: String,
    pub medication_id: String,
    pub medication_name: String,
    pub dosage: String,
    pub taken_at_epoch: i64,
    pub taken_at_display: String,
    pub glucose_mg_dl_at_dose: Option<f64>,
    pub notes: String,
    #[serde(default)]
    pub skipped: bool,
}

impl fmt::Debug for MedicationDoseLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MedicationDoseLog")
            .field("id", &self.id)
            .field("medication_id", &self.medication_id)
            .field("medication_name", &"[REDACTED]")
            .field("dosage", &"[REDACTED]")
            .field("taken_at_epoch", &self.taken_at_epoch)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MedicationReminder {
    pub medication_id: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoseStatusKind {
    Taken,
    Skipped,
    DueNow,
    Upcoming,
    AsNeeded,
}

impl DoseStatusKind {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Taken => "taken",
            Self::Skipped => "skipped",
            Self::DueNow => "due",
            Self::Upcoming => "upcoming",
            Self::AsNeeded => "as_needed",
        }
    }
}

/// Pre-positioned medication dose marker for rendering on the 12-hour glucose trend chart.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphDoseMarker {
    pub id: String,
    pub medication_name: String,
    pub dosage: String,
    pub taken_at_display: String,
    pub point_index: f64,
    pub value_mg_dl: f64,
}

/// View-model item serialized to JSON for QML / UI medication lists.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MedicationViewItem {
    pub id: String,
    pub name: String,
    pub dosage: String,
    pub schedule_code: String,
    pub schedule_label: String,
    pub scheduled_time: String,
    pub weekday: u8,
    pub weekday_label: String,
    pub reminder_enabled: bool,
    pub notes: String,
    pub status_code: String,
    pub status_label: String,
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MedicationStore {
    #[serde(default)]
    pub medications: Vec<MedicationDef>,
    #[serde(default)]
    pub dose_logs: Vec<MedicationDoseLog>,
}

impl fmt::Debug for MedicationStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MedicationStore")
            .field("medications_len", &self.medications.len())
            .field("dose_logs_len", &self.dose_logs.len())
            .finish()
    }
}

pub fn format_epoch_short(epoch: i64) -> String {
    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|dt| dt.format("%m/%d %-I:%M %p").to_string())
        .unwrap_or_else(|| epoch.to_string())
}

pub fn weekday_short_name(weekday: u8) -> &'static str {
    match weekday % 7 {
        0 => "Mon",
        1 => "Tue",
        2 => "Wed",
        3 => "Thu",
        4 => "Fri",
        5 => "Sat",
        _ => "Sun",
    }
}

pub fn weekday_full_name(weekday: u8) -> &'static str {
    match weekday % 7 {
        0 => "Monday",
        1 => "Tuesday",
        2 => "Wednesday",
        3 => "Thursday",
        4 => "Friday",
        5 => "Saturday",
        _ => "Sunday",
    }
}

/// Parses `"HH:MM"` slots from `scheduled_time` (e.g. `"08:00"` or `"08:00,20:00"`).
pub fn parse_scheduled_slots(raw: &str, schedule: MedicationSchedule) -> Vec<NaiveTime> {
    let mut slots: Vec<NaiveTime> = raw
        .split([',', ';', '/'])
        .filter_map(|part| {
            let trimmed = part.trim();
            NaiveTime::parse_from_str(trimmed, "%H:%M")
                .or_else(|_| NaiveTime::parse_from_str(trimmed, "%H:%M:%S"))
                .ok()
        })
        .collect();

    if slots.is_empty() {
        if let Some(t) = NaiveTime::from_hms_opt(8, 0, 0) {
            slots.push(t);
        }
    }
    if schedule == MedicationSchedule::TwiceDaily && slots.len() == 1 {
        if let Some(t2) = NaiveTime::from_hms_opt(20, 0, 0) {
            if slots[0] != t2 {
                slots.push(t2);
            }
        }
    }
    slots.sort();
    slots.dedup();
    slots
}

impl MedicationStore {
    /// Adds or updates a medication definition. Returns its assigned `id`.
    ///
    /// The flat argument list mirrors the Qt/egui form fields one-to-one; a parameter struct
    /// would add boilerplate at all four call sites without improving clarity.
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_medication(
        &mut self,
        id: &str,
        name: &str,
        dosage: &str,
        schedule: MedicationSchedule,
        scheduled_time: &str,
        weekday: u8,
        reminder_enabled: bool,
        notes: &str,
    ) -> Result<String, String> {
        let clean_name = name.trim();
        if clean_name.is_empty() {
            return Err("Medication name is required.".to_string());
        }
        let clean_dosage = dosage.trim();
        let clean_time = if scheduled_time.trim().is_empty() {
            match schedule {
                MedicationSchedule::TwiceDaily => "08:00,20:00".to_string(),
                _ => "08:00".to_string(),
            }
        } else {
            scheduled_time.trim().to_string()
        };

        let assigned_id = if id.trim().is_empty() {
            format!(
                "med-{}-{}",
                chrono::Utc::now().timestamp_millis(),
                self.medications.len() + 1
            )
        } else {
            id.trim().to_string()
        };

        if let Some(existing) = self.medications.iter_mut().find(|m| m.id == assigned_id) {
            existing.name = clean_name.to_string();
            existing.dosage = clean_dosage.to_string();
            existing.schedule = schedule;
            existing.scheduled_time = clean_time;
            existing.weekday = weekday % 7;
            existing.reminder_enabled = reminder_enabled;
            existing.notes = notes.trim().to_string();
        } else {
            self.medications.push(MedicationDef {
                id: assigned_id.clone(),
                name: clean_name.to_string(),
                dosage: clean_dosage.to_string(),
                schedule,
                scheduled_time: clean_time,
                weekday: weekday % 7,
                reminder_enabled,
                notes: notes.trim().to_string(),
                enabled: true,
            });
        }
        Ok(assigned_id)
    }

    pub fn remove_medication(&mut self, medication_id: &str) -> bool {
        let before = self.medications.len();
        self.medications.retain(|m| m.id != medication_id.trim());
        self.medications.len() != before
    }

    /// Logs a dose for `medication_id` (or an ad-hoc medication name) at `taken_at_epoch`.
    pub fn log_dose(
        &mut self,
        medication_id: &str,
        custom_dosage: Option<&str>,
        taken_at_epoch: i64,
        glucose_mg_dl: Option<f64>,
        notes: &str,
    ) -> Result<MedicationDoseLog, String> {
        let med = self
            .medications
            .iter()
            .find(|m| m.id == medication_id.trim())
            .cloned();

        let (med_id, med_name, med_dosage) = if let Some(m) = med {
            let d = custom_dosage
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(&m.dosage)
                .to_string();
            (m.id, m.name, d)
        } else {
            let trimmed = medication_id.trim();
            if trimmed.is_empty() {
                return Err("Select or enter a medication to log.".to_string());
            }
            (
                "adhoc".to_string(),
                trimmed.to_string(),
                custom_dosage.unwrap_or("").trim().to_string(),
            )
        };

        let entry = MedicationDoseLog {
            id: format!("dose-{taken_at_epoch}-{}", self.dose_logs.len() + 1),
            medication_id: med_id,
            medication_name: med_name,
            dosage: med_dosage,
            taken_at_epoch,
            taken_at_display: format_epoch_short(taken_at_epoch),
            glucose_mg_dl_at_dose: glucose_mg_dl.filter(|v| *v > 0.0 && !v.is_nan()),
            notes: notes.trim().to_string(),
            skipped: false,
        };

        self.dose_logs.insert(0, entry.clone());
        self.dose_logs
            .sort_by_key(|d| std::cmp::Reverse(d.taken_at_epoch));

        // Retain 180 days / up to 2,000 dose entries.
        let cutoff = taken_at_epoch - 180 * 86_400;
        self.dose_logs.retain(|d| d.taken_at_epoch >= cutoff);
        if self.dose_logs.len() > 2_000 {
            self.dose_logs.truncate(2_000);
        }

        Ok(entry)
    }

    /// Records a skipped dose for `medication_id` at `skipped_at_epoch` so the current
    /// scheduled window is satisfied and reminders do not repeat for this slot.
    pub fn skip_dose(
        &mut self,
        medication_id: &str,
        skipped_at_epoch: i64,
        notes: &str,
    ) -> Result<MedicationDoseLog, String> {
        let med = self
            .medications
            .iter()
            .find(|m| m.id == medication_id.trim())
            .cloned()
            .ok_or_else(|| "Select a scheduled medication to skip.".to_string())?;

        let entry = MedicationDoseLog {
            id: format!("skip-{skipped_at_epoch}-{}", self.dose_logs.len() + 1),
            medication_id: med.id,
            medication_name: med.name,
            dosage: med.dosage,
            taken_at_epoch: skipped_at_epoch,
            taken_at_display: format_epoch_short(skipped_at_epoch),
            glucose_mg_dl_at_dose: None,
            notes: if notes.trim().is_empty() {
                "Skipped".to_string()
            } else {
                notes.trim().to_string()
            },
            skipped: true,
        };

        self.dose_logs.insert(0, entry.clone());
        self.dose_logs
            .sort_by_key(|d| std::cmp::Reverse(d.taken_at_epoch));

        let cutoff = skipped_at_epoch - 180 * 86_400;
        self.dose_logs.retain(|d| d.taken_at_epoch >= cutoff);
        if self.dose_logs.len() > 2_000 {
            self.dose_logs.truncate(2_000);
        }

        Ok(entry)
    }

    pub fn remove_dose_log(&mut self, dose_id: &str) -> bool {
        let before = self.dose_logs.len();
        self.dose_logs.retain(|d| d.id != dose_id.trim());
        self.dose_logs.len() != before
    }

    /// Evaluates the current status of a medication (`Taken`, `DueNow`, `Upcoming`, `AsNeeded`)
    /// and a concise human-readable status string at `now_epoch`.
    pub fn medication_status(&self, med: &MedicationDef, now_epoch: i64) -> (DoseStatusKind, String) {
        let last_dose = self
            .dose_logs
            .iter()
            .find(|d| d.medication_id == med.id);

        if med.schedule == MedicationSchedule::AsNeeded {
            let label = if let Some(d) = last_dose {
                format!("As needed • Last taken {}", d.taken_at_display)
            } else {
                "As needed • No doses logged yet".to_string()
            };
            return (DoseStatusKind::AsNeeded, label);
        }

        let Some(now_dt) = Local.timestamp_opt(now_epoch, 0).single() else {
            return (DoseStatusKind::Upcoming, med.scheduled_time.clone());
        };
        let today = now_dt.date_naive();
        let slots = parse_scheduled_slots(&med.scheduled_time, med.schedule);

        if med.schedule == MedicationSchedule::Weekly {
            let today_wd = today.weekday().num_days_from_monday() as u8;
            // Start of the current local week (Monday 00:00), computed from the calendar so
            // DST transitions and non-UTC offsets are handled correctly.
            let week_start_epoch = today
                .checked_sub_days(chrono::Days::new(today_wd as u64))
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .and_then(|ndt| Local.from_local_datetime(&ndt).earliest())
                .map(|dt| dt.timestamp())
                .unwrap_or(now_epoch - (today_wd as i64) * 86_400);
            if let Some(d) = last_dose.filter(|d| d.taken_at_epoch >= week_start_epoch) {
                if d.skipped {
                    return (
                        DoseStatusKind::Skipped,
                        format!("⏭ Skipped {}", d.taken_at_display),
                    );
                }
                return (
                    DoseStatusKind::Taken,
                    format!("✓ Taken {}", d.taken_at_display),
                );
            }
            if today_wd == med.weekday % 7 {
                if let Some(slot) = slots.first() {
                    if now_dt.time() >= *slot {
                        return (
                            DoseStatusKind::DueNow,
                            format!(
                                "⏰ Due today ({} {})",
                                weekday_short_name(med.weekday),
                                slot.format("%H:%M")
                            ),
                        );
                    }
                }
            }
            return (
                DoseStatusKind::Upcoming,
                format!(
                    "Scheduled {} {}",
                    weekday_short_name(med.weekday),
                    med.scheduled_time
                ),
            );
        }

        // Daily or TwiceDaily: check today's scheduled slots
        let day_start_epoch = today
            .and_hms_opt(0, 0, 0)
            .and_then(|ndt| Local.from_local_datetime(&ndt).earliest())
            .map(|dt| dt.timestamp())
            .unwrap_or(now_epoch - 43_200);

        let doses_today: Vec<&MedicationDoseLog> = self
            .dose_logs
            .iter()
            .filter(|d| d.medication_id == med.id && d.taken_at_epoch >= day_start_epoch)
            .collect();

        if doses_today.len() >= slots.len() {
            if let Some(latest) = doses_today.first() {
                if latest.skipped {
                    return (
                        DoseStatusKind::Skipped,
                        format!("⏭ Skipped ({})", latest.taken_at_display),
                    );
                }
                return (
                    DoseStatusKind::Taken,
                    format!("✓ Taken ({})", latest.taken_at_display),
                );
            }
        }

        // Check if any slot today is due and not yet satisfied
        for (idx, slot) in slots.iter().enumerate() {
            if now_dt.time() >= *slot && doses_today.len() <= idx {
                return (
                    DoseStatusKind::DueNow,
                    format!("⏰ Due now (scheduled {})", slot.format("%H:%M")),
                );
            }
        }

        if let Some(next_slot) = slots.iter().find(|&&s| s > now_dt.time()).or_else(|| slots.first())
        {
            return (
                DoseStatusKind::Upcoming,
                format!("Next dose at {}", next_slot.format("%H:%M")),
            );
        }

        (DoseStatusKind::Upcoming, format!("Scheduled {}", med.scheduled_time))
    }

    /// Evaluates all enabled medications with `reminder_enabled` and returns any due/overdue
    /// desktop notifications that have not been notified within `MEDICATION_REMINDER_REPEAT_SECONDS`.
    pub fn evaluate_due_reminders(
        &self,
        now_epoch: i64,
        last_notified: &mut HashMap<String, i64>,
    ) -> Vec<MedicationReminder> {
        let mut reminders = Vec::new();
        for med in &self.medications {
            if !med.enabled || !med.reminder_enabled {
                continue;
            }
            let (status, detail) = self.medication_status(med, now_epoch);
            if status != DoseStatusKind::DueNow {
                continue;
            }
            if let Some(&last_ts) = last_notified.get(&med.id) {
                if now_epoch - last_ts < MEDICATION_REMINDER_REPEAT_SECONDS {
                    continue;
                }
            }
            last_notified.insert(med.id.clone(), now_epoch);
            let dose_part = if med.dosage.trim().is_empty() {
                med.name.clone()
            } else {
                format!("{} ({})", med.name, med.dosage)
            };
            reminders.push(MedicationReminder {
                medication_id: med.id.clone(),
                title: format!("Medication Reminder: {}", med.name),
                body: format!("{dose_part} — {detail}"),
            });
        }
        reminders
    }

    /// Returns all logged doses within `[start_epoch, end_epoch]` sorted chronologically
    /// for rendering markers on the 12-hour glucose trend graph.
    pub fn doses_in_window(&self, start_epoch: i64, end_epoch: i64) -> Vec<MedicationDoseLog> {
        let mut list: Vec<MedicationDoseLog> = self
            .dose_logs
            .iter()
            .filter(|d| d.taken_at_epoch >= start_epoch && d.taken_at_epoch <= end_epoch)
            .cloned()
            .collect();
        list.sort_by_key(|d| d.taken_at_epoch);
        list
    }

    /// Computes pre-positioned [`GraphDoseMarker`] items for `snapshot.graph_points`.
    pub fn graph_markers_for_snapshot(
        &self,
        snapshot: &GlucoseSnapshot,
        now_epoch: i64,
    ) -> Vec<GraphDoseMarker> {
        let pts = &snapshot.graph_points;
        if pts.is_empty() || self.dose_logs.is_empty() {
            return Vec::new();
        }

        let point_epochs: Vec<(i64, f64)> = pts
            .iter()
            .enumerate()
            .map(|(idx, pt)| {
                let ep = parse_libreview_timestamp(&pt.timestamp, false).unwrap_or_else(|| {
                    let rem = pts.len().saturating_sub(1 + idx) as i64;
                    now_epoch - rem * 15 * 60
                });
                (ep, pt.value_mg_dl)
            })
            .collect();

        let min_ep = point_epochs
            .iter()
            .map(|(e, _)| *e)
            .min()
            .unwrap_or(now_epoch - 12 * 3600)
            - 900;
        let max_ep = point_epochs
            .iter()
            .map(|(e, _)| *e)
            .max()
            .unwrap_or(now_epoch)
            + 900;

        let mut markers = Vec::new();
        for dose in &self.dose_logs {
            if dose.skipped || dose.taken_at_epoch < min_ep || dose.taken_at_epoch > max_ep {
                continue;
            }
            // Find nearest graph point by timestamp
            let mut best_idx = 0usize;
            let mut best_diff = i64::MAX;
            for (idx, (ep, _)) in point_epochs.iter().enumerate() {
                let diff = (dose.taken_at_epoch - *ep).abs();
                if diff < best_diff {
                    best_diff = diff;
                    best_idx = idx;
                }
            }
            let val = dose
                .glucose_mg_dl_at_dose
                .filter(|v| *v > 0.0)
                .unwrap_or(point_epochs[best_idx].1);
            markers.push(GraphDoseMarker {
                id: dose.id.clone(),
                medication_name: dose.medication_name.clone(),
                dosage: dose.dosage.clone(),
                taken_at_display: dose.taken_at_display.clone(),
                point_index: best_idx as f64,
                value_mg_dl: val,
            });
        }
        markers.sort_by(|a, b| {
            a.point_index
                .partial_cmp(&b.point_index)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        markers
    }

    /// Builds [`MedicationViewItem`] entries with live status strings at `now_epoch`.
    pub fn view_items(&self, now_epoch: i64) -> Vec<MedicationViewItem> {
        self.medications
            .iter()
            .map(|m| {
                let (kind, label) = self.medication_status(m, now_epoch);
                MedicationViewItem {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    dosage: m.dosage.clone(),
                    schedule_code: m.schedule.code().to_string(),
                    schedule_label: m.schedule.label().to_string(),
                    scheduled_time: m.scheduled_time.clone(),
                    weekday: m.weekday,
                    weekday_label: weekday_full_name(m.weekday).to_string(),
                    reminder_enabled: m.reminder_enabled,
                    notes: m.notes.clone(),
                    status_code: kind.code().to_string(),
                    status_label: label,
                }
            })
            .collect()
    }

    /// Summary line for the Dashboard & Medications header.
    pub fn summary_line(&self, now_epoch: i64) -> String {
        if self.medications.is_empty() {
            return "No medications configured".to_string();
        }
        let mut due_count = 0usize;
        let mut taken_count = 0usize;
        for med in self.medications.iter().filter(|m| m.enabled) {
            match self.medication_status(med, now_epoch).0 {
                DoseStatusKind::DueNow => due_count += 1,
                DoseStatusKind::Taken => taken_count += 1,
                _ => {}
            }
        }
        if due_count > 0 {
            format!(
                "⏰ {} medication{} due now • {} taken today",
                due_count,
                if due_count == 1 { "" } else { "s" },
                taken_count
            )
        } else {
            format!(
                "✓ All scheduled doses up to date ({} taken today, {} total)",
                taken_count,
                self.medications.len()
            )
        }
    }

    /// Builds the Home Assistant `/api/states/sensor.librelinkup_medications` JSON payload for
    /// two-way medication and dose synchronization.
    pub fn to_ha_sensor_payload(&self, now_epoch: i64) -> serde_json::Value {
        let mut due_now_count = 0usize;
        let mut taken_today_count = 0usize;
        for med in self.medications.iter().filter(|m| m.enabled) {
            match self.medication_status(med, now_epoch).0 {
                DoseStatusKind::DueNow => due_now_count += 1,
                DoseStatusKind::Taken => taken_today_count += 1,
                _ => {}
            }
        }
        let latest = self.dose_logs.first();
        let recent_doses: Vec<&MedicationDoseLog> = self.dose_logs.iter().take(25).collect();
        serde_json::json!({
            "state": self.summary_line(now_epoch),
            "attributes": {
                "friendly_name": "LibreLinkUp Medications & Dose Tracker",
                "icon": "mdi:pill",
                "updated_at_epoch": now_epoch,
                "due_now_count": due_now_count,
                "taken_today_count": taken_today_count,
                "scheduled_count": self.medications.len(),
                "last_dose_medication": latest.map(|d| d.medication_name.as_str()).unwrap_or(""),
                "last_dose_dosage": latest.map(|d| d.dosage.as_str()).unwrap_or(""),
                "last_dose_at_epoch": latest.map(|d| d.taken_at_epoch).unwrap_or(0),
                "last_dose_at_display": latest.map(|d| d.taken_at_display.as_str()).unwrap_or(""),
                "last_dose_glucose_mg_dl": latest.and_then(|d| d.glucose_mg_dl_at_dose),
                "last_dose_skipped": latest.map(|d| d.skipped).unwrap_or(false),
                "medications": self.medications,
                "recent_doses": recent_doses,
            }
        })
    }

    /// Merges remote medications and dose logs published to Home Assistant's
    /// `sensor.librelinkup_medications` entity (from another desktop client or a Home Assistant
    /// automation/script/button). Returns `true` if any medication or dose log was imported.
    pub fn merge_from_ha_states(&mut self, states_arr: &[serde_json::Value]) -> bool {
        let Some(ent) = states_arr.iter().find(|e| {
            e.get("entity_id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| id == "sensor.librelinkup_medications")
        }) else {
            return false;
        };
        let Some(attrs) = ent.get("attributes") else {
            return false;
        };

        let mut changed = false;

        // 1. Merge remote medication definitions
        if let Some(meds_val) = attrs.get("medications") {
            if let Ok(remote_meds) = serde_json::from_value::<Vec<MedicationDef>>(meds_val.clone())
            {
                for rm in remote_meds {
                    if rm.name.trim().is_empty() {
                        continue;
                    }
                    let already_exists = self.medications.iter().any(|m| {
                        (!rm.id.is_empty() && m.id == rm.id)
                            || (m.name.eq_ignore_ascii_case(&rm.name)
                                && m.dosage.eq_ignore_ascii_case(&rm.dosage))
                    });
                    if !already_exists {
                        self.medications.push(rm);
                        changed = true;
                    }
                }
            }
        }

        // 2. Merge remote dose logs
        if let Some(doses_val) = attrs.get("recent_doses") {
            if let Ok(remote_doses) =
                serde_json::from_value::<Vec<MedicationDoseLog>>(doses_val.clone())
            {
                for mut rd in remote_doses {
                    if rd.medication_name.trim().is_empty() || rd.taken_at_epoch <= 0 {
                        continue;
                    }
                    if rd.taken_at_display.trim().is_empty() {
                        rd.taken_at_display = format_epoch_short(rd.taken_at_epoch);
                    }
                    let duplicate = self.dose_logs.iter().any(|d| {
                        (!rd.id.is_empty() && d.id == rd.id)
                            || (d.medication_name.eq_ignore_ascii_case(&rd.medication_name)
                                && (d.taken_at_epoch - rd.taken_at_epoch).abs() < 60
                                && d.skipped == rd.skipped)
                    });
                    if !duplicate {
                        self.dose_logs.push(rd);
                        changed = true;
                    }
                }
            }
        }

        // 3. Support simple Home Assistant automation/button triggers via attributes:
        //    `trigger_take_medication`: medication name or ID, `trigger_epoch`: unix epoch seconds
        if let Some(trig_med) = attrs
            .get("trigger_take_medication")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let trig_epoch = attrs
                .get("trigger_epoch")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if trig_epoch > 0 {
                let target_med = self
                    .medications
                    .iter()
                    .find(|m| m.id == trig_med || m.name.eq_ignore_ascii_case(trig_med))
                    .cloned();
                let med_name = target_med
                    .as_ref()
                    .map(|m| m.name.as_str())
                    .unwrap_or(trig_med);
                let already_logged = self.dose_logs.iter().any(|d| {
                    d.medication_name.eq_ignore_ascii_case(med_name)
                        && (d.taken_at_epoch - trig_epoch).abs() < 120
                });
                if !already_logged {
                    let id_or_name = target_med
                        .as_ref()
                        .map(|m| m.id.as_str())
                        .unwrap_or(trig_med);
                    if self.log_dose(id_or_name, None, trig_epoch, None, "Logged via Home Assistant").is_ok() {
                        changed = true;
                    }
                }
            }
        }

        if changed {
            self.dose_logs
                .sort_by_key(|d| std::cmp::Reverse(d.taken_at_epoch));
            if self.dose_logs.len() > 2_000 {
                self.dose_logs.truncate(2_000);
            }
        }
        changed
    }
}

/// Generates sample medications and a logged dose within the last 2 hours for `--demo` mode.
pub fn build_demo_medication_store(now_epoch: i64) -> MedicationStore {
    let mut store = MedicationStore::default();
    let id1 = store
        .upsert_medication(
            "demo-med-1",
            "Metformin ER",
            "500 mg",
            MedicationSchedule::TwiceDaily,
            "08:00,20:00",
            0,
            true,
            "Take with meals",
        )
        .unwrap_or_default();
    let _ = store.upsert_medication(
        "demo-med-2",
        "Basal Insulin",
        "16 units",
        MedicationSchedule::Daily,
        "21:00",
        0,
        true,
        "Evening long-acting dose",
    );
    let _ = store.upsert_medication(
        "demo-med-3",
        "Semaglutide",
        "1.0 mg",
        MedicationSchedule::Weekly,
        "08:00",
        4,
        true,
        "Weekly Friday morning dose",
    );
    let _ = store.log_dose(&id1, Some("500 mg"), now_epoch - 7 * 3600, Some(124.0), "Morning dose");
    let _ = store.log_dose(&id1, Some("500 mg"), now_epoch - 2 * 3600, Some(148.0), "Evening dose");
    store
}

pub fn load_medications_from_path(path: &Path) -> MedicationStore {
    match phi_secret().and_then(|secret| try_load_medications_from_path_with_secret(path, &secret))
    {
        Ok(Some(store)) => store,
        Ok(None) => MedicationStore::default(),
        Err(e) => {
            log_error("CRYPTO", format!("Could not open medication vault: {e}"));
            MedicationStore::default()
        }
    }
}

/// Loads a medication vault, returning an error string on decryption failure.
pub fn try_load_medications_from_path_with_secret(
    path: &Path,
    secret: &[u8],
) -> Result<Option<MedicationStore>, String> {
    use zeroize::Zeroize;

    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read medication vault: {e}"))?;
    let envelope: crate::crypto::EncryptedEnvelope = serde_json::from_str(&raw)
        .map_err(|e| format!("Corrupted medication envelope: {e}"))?;
    let mut plaintext = decrypt_bytes_with_secret(&envelope, secret, AAD_LABEL_MEDICATIONS)?;
    let parsed = serde_json::from_slice::<MedicationStore>(&plaintext)
        .map_err(|e| format!("Corrupted medication payload: {e}"));
    plaintext.zeroize();
    Ok(Some(parsed?))
}

pub fn save_medications_to_path(store: &MedicationStore, path: &Path) -> Result<(), String> {
    let secret = phi_secret()?;
    save_medications_to_path_with_secret(store, path, &secret)
}

pub fn save_medications_to_path_with_secret(
    store: &MedicationStore,
    path: &Path,
    secret: &[u8],
) -> Result<(), String> {
    use zeroize::Zeroize;

    let mut json = serde_json::to_vec(store)
        .map_err(|e| format!("Failed to serialize medications: {e}"))?;
    let res = encrypt_bytes_with_secret(&json, secret, AAD_LABEL_MEDICATIONS).and_then(|envelope| {
        let serialized = serde_json::to_string(&envelope)
            .map_err(|e| format!("Failed to serialize medication envelope: {e}"))?;
        write_file_atomic_0600(path, serialized.as_bytes())
            .map_err(|e| format!("Failed to write medication vault: {e}"))
    });
    json.zeroize();
    res
}

/// Loads the medication vault, transparently migrating a pre-0.5 plaintext
/// `medications.json` into the encrypted vault and deleting the plaintext copy.
pub fn load_default_medications() -> MedicationStore {
    let enc_path = medications_file_path();
    if enc_path.exists() {
        return load_medications_from_path(&enc_path);
    }

    let legacy_path = legacy_medications_file_path();
    if let Ok(raw) = fs::read_to_string(&legacy_path) {
        if let Ok(store) = serde_json::from_str::<MedicationStore>(&raw) {
            match save_medications_to_path(&store, &enc_path) {
                Ok(()) => {
                    let _ = fs::remove_file(&legacy_path);
                    log_info(
                        "CRYPTO",
                        format!(
                            "Migrated {} medications and {} dose logs from plaintext to the encrypted medication vault.",
                            store.medications.len(),
                            store.dose_logs.len()
                        ),
                    );
                }
                Err(e) => {
                    log_error("CRYPTO", format!("Could not migrate plaintext medications: {e}"));
                }
            }
            return store;
        }
    }
    MedicationStore::default()
}

pub fn save_default_medications(store: &MedicationStore) -> Result<(), String> {
    save_medications_to_path(store, &medications_file_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medication_crud_dose_logging_reminders_and_graph_window_work() {
        let now = Local
            .with_ymd_and_hms(2026, 10, 29, 10, 30, 0)
            .single()
            .unwrap()
            .timestamp();

        let mut store = MedicationStore::default();
        let med_id = store
            .upsert_medication(
                "",
                "Metformin",
                "500 mg",
                MedicationSchedule::Daily,
                "08:00",
                0,
                true,
                "With breakfast",
            )
            .unwrap();
        assert_eq!(store.medications.len(), 1);

        // At 10:30 AM, an 08:00 daily medication with no dose logged today is DueNow.
        let (status, _) = store.medication_status(&store.medications[0], now);
        assert_eq!(status, DoseStatusKind::DueNow);

        let mut notified = HashMap::new();
        let reminders = store.evaluate_due_reminders(now, &mut notified);
        assert_eq!(reminders.len(), 1);
        assert!(reminders[0].title.contains("Metformin"));

        // Second evaluation within 60 minutes does not re-fire.
        assert!(store.evaluate_due_reminders(now + 120, &mut notified).is_empty());

        // Logging the dose marks it Taken today and includes it in the 12h graph window.
        let dose = store
            .log_dose(&med_id, None, now - 1800, Some(132.0), "Taken")
            .unwrap();
        let (status_after, _) = store.medication_status(&store.medications[0], now);
        assert_eq!(status_after, DoseStatusKind::Taken);

        let in_win = store.doses_in_window(now - 12 * 3600, now);
        assert_eq!(in_win.len(), 1);
        assert_eq!(in_win[0].id, dose.id);

        // Ensure Debug redacts medication name/dosage
        let dbg = format!("{:?}", store.medications[0]);
        assert!(!dbg.contains("Metformin"));
        assert!(dbg.contains("[REDACTED]"));

        assert!(store.remove_dose_log(&dose.id));
        assert!(store.remove_medication(&med_id));
        assert!(store.medications.is_empty());

        // Weekly medication day-of-week & skip_dose test (2026-10-29 is Thursday -> weekday 3)
        let wk_id = store
            .upsert_medication(
                "",
                "Semaglutide",
                "1.0 mg",
                MedicationSchedule::Weekly,
                "09:00",
                3, // Thursday
                true,
                "Weekly injection",
            )
            .unwrap();
        let (wk_status, wk_label) = store.medication_status(&store.medications[0], now);
        assert_eq!(wk_status, DoseStatusKind::DueNow);
        assert!(wk_label.contains("Thu"));

        let skipped = store.skip_dose(&wk_id, now, "Doctor advised skip").unwrap();
        assert!(skipped.skipped);
        let (wk_after, _) = store.medication_status(&store.medications[0], now);
        assert_eq!(wk_after, DoseStatusKind::Skipped);

        // Two-way Home Assistant medication & dose sync round-trip
        let mut remote_store = MedicationStore::default();
        let rem_id = remote_store
            .upsert_medication(
                "ha-med-1",
                "Lantus SoloStar",
                "18 units",
                MedicationSchedule::Daily,
                "21:00",
                0,
                true,
                "Bedtime basal",
            )
            .unwrap();
        remote_store
            .log_dose(&rem_id, None, now - 600, Some(118.0), "Logged from HA")
            .unwrap();
        let ha_payload = remote_store.to_ha_sensor_payload(now);
        let ha_states = serde_json::json!([
            {
                "entity_id": "sensor.librelinkup_medications",
                "state": ha_payload["state"],
                "attributes": ha_payload["attributes"]
            }
        ]);
        let states_arr = ha_states.as_array().unwrap();
        assert!(store.merge_from_ha_states(states_arr));
        assert!(store.medications.iter().any(|m| m.name == "Lantus SoloStar"));
        assert!(store.dose_logs.iter().any(|d| d.medication_name == "Lantus SoloStar"));
        // Merging identical states a second time is idempotent (returns false)
        assert!(!store.merge_from_ha_states(states_arr));
    }
}
