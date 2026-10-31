//! `MedicationsController` implementation: medication schedule and dose log.
//!
//! All persistence goes through the encrypted medication vault (see `medications.rs` in the
//! core crate); nothing here writes plaintext health data to disk.

use std::pin::Pin;

use cxx_qt_lib::QString;

use crate::bridge::ops;
use crate::bridge::qobject_bridge::{qobject, MedicationsControllerRust};
use crate::bridge::state::{now_epoch, with_state};
use crate::event_log::{log_info, log_warn};
use crate::medications::{save_default_medications, MedicationSchedule};

macro_rules! sync_str {
    ($self:ident, $setter:ident, $getter:ident, $value:expr) => {{
        let new_value: String = $value;
        if $self.$getter().to_string() != new_value {
            $self.as_mut().$setter(QString::from(new_value.as_str()));
        }
    }};
}

macro_rules! sync_val {
    ($self:ident, $setter:ident, $getter:ident, $value:expr) => {{
        let new_value = $value;
        if *$self.$getter() != new_value {
            $self.as_mut().$setter(new_value);
        }
    }};
}

impl Default for MedicationsControllerRust {
    fn default() -> Self {
        // Valid empty values only; `initialize()` -> `sync()` fills them from shared state.
        Self {
            medications_json: QString::from("[]"),
            medication_doses_json: QString::from("[]"),
            graph_doses_json: QString::from("[]"),
            medications_summary: QString::from("No medications configured"),
            medications_count: 0,
            medication_doses_count: 0,
        }
    }
}

impl qobject::MedicationsController {
    pub fn initialize(mut self: Pin<&mut Self>) {
        with_state(ops::refresh_medication_views);
        self.as_mut().sync();
    }

    pub fn sync(mut self: Pin<&mut Self>) {
        let v = with_state(|s| {
            (
                s.medications_json.clone(),
                s.medication_doses_json.clone(),
                s.graph_doses_json.clone(),
                s.medications_summary.clone(),
                s.medications_count,
                s.medication_doses_count,
            )
        });
        let (meds, doses, markers, summary, med_count, dose_count) = v;
        sync_str!(self, set_medications_json, medications_json, meds);
        sync_str!(
            self,
            set_medication_doses_json,
            medication_doses_json,
            doses
        );
        sync_str!(self, set_graph_doses_json, graph_doses_json, markers);
        sync_str!(self, set_medications_summary, medications_summary, summary);
        sync_val!(self, set_medications_count, medications_count, med_count);
        sync_val!(
            self,
            set_medication_doses_count,
            medication_doses_count,
            dose_count
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_medication(
        mut self: Pin<&mut Self>,
        id: QString,
        name: QString,
        dosage: QString,
        schedule: QString,
        scheduled_time: QString,
        weekday: i32,
        reminder_enabled: bool,
        notes: QString,
    ) -> bool {
        let sched = MedicationSchedule::from_code(&schedule.to_string());
        let clean_name = name.to_string().trim().to_string();
        let outcome = with_state(|s| {
            let res = s.medication_store.upsert_medication(
                &id.to_string(),
                &name.to_string(),
                &dosage.to_string(),
                sched,
                &scheduled_time.to_string(),
                weekday.clamp(0, 6) as u8,
                reminder_enabled,
                &notes.to_string(),
            );
            if res.is_ok() {
                let _ = save_default_medications(&s.medication_store);
                ops::refresh_medication_views(s);
            }
            res
        });
        match outcome {
            Ok(_) => {
                log_info(
                    "MEDS",
                    format!("Saved medication '{clean_name}' ({})", sched.label()),
                );
                ops::spawn_ha_medications_push(None);
                self.as_mut().sync();
                true
            }
            Err(err) => {
                log_warn("MEDS", format!("Could not save medication: {err}"));
                false
            }
        }
    }

    pub fn remove_medication(mut self: Pin<&mut Self>, medication_id: QString) -> bool {
        let removed = with_state(|s| {
            let ok = s.medication_store.remove_medication(&medication_id.to_string());
            if ok {
                let _ = save_default_medications(&s.medication_store);
                ops::refresh_medication_views(s);
            }
            ok
        });
        if removed {
            log_info("MEDS", "Removed scheduled medication.");
            ops::spawn_ha_medications_push(None);
            self.as_mut().sync();
        }
        removed
    }

    pub fn log_medication_dose(
        mut self: Pin<&mut Self>,
        medication_id: QString,
        custom_dosage: QString,
        notes: QString,
    ) -> bool {
        let now = now_epoch();
        let dosage = custom_dosage.to_string();
        let cd = if dosage.trim().is_empty() {
            None
        } else {
            Some(dosage.as_str())
        };
        let entry = with_state(|s| {
            let glucose = (s.snapshot.value_mg_dl > 0.0).then_some(s.snapshot.value_mg_dl);
            let logged = s.medication_store.log_dose(
                &medication_id.to_string(),
                cd,
                now,
                glucose,
                &notes.to_string(),
            );
            if logged.is_ok() {
                let _ = save_default_medications(&s.medication_store);
                ops::refresh_medication_views(s);
            }
            logged
        });
        match entry {
            Ok(e) => {
                log_info(
                    "MEDS",
                    format!(
                        "Logged medication dose: {} ({}) at {}",
                        e.medication_name, e.dosage, e.taken_at_display
                    ),
                );
                ops::spawn_ha_medications_push(Some(e));
                self.as_mut().sync();
                true
            }
            Err(err) => {
                log_warn("MEDS", format!("Could not log medication dose: {err}"));
                false
            }
        }
    }

    pub fn skip_medication_dose(
        mut self: Pin<&mut Self>,
        medication_id: QString,
        notes: QString,
    ) -> bool {
        let now = now_epoch();
        let entry = with_state(|s| {
            let skipped =
                s.medication_store
                    .skip_dose(&medication_id.to_string(), now, &notes.to_string());
            if skipped.is_ok() {
                let _ = save_default_medications(&s.medication_store);
                ops::refresh_medication_views(s);
            }
            skipped
        });
        match entry {
            Ok(e) => {
                log_info(
                    "MEDS",
                    format!(
                        "Skipped medication dose: {} ({}) at {}",
                        e.medication_name, e.dosage, e.taken_at_display
                    ),
                );
                ops::spawn_ha_medications_push(Some(e));
                self.as_mut().sync();
                true
            }
            Err(err) => {
                log_warn("MEDS", format!("Could not skip medication dose: {err}"));
                false
            }
        }
    }

    pub fn remove_medication_dose(mut self: Pin<&mut Self>, dose_id: QString) -> bool {
        let removed = with_state(|s| {
            let ok = s.medication_store.remove_dose_log(&dose_id.to_string());
            if ok {
                let _ = save_default_medications(&s.medication_store);
                ops::refresh_medication_views(s);
            }
            ok
        });
        if removed {
            log_info("MEDS", "Removed logged medication dose entry.");
            ops::spawn_ha_medications_push(None);
            self.as_mut().sync();
        }
        removed
    }
}
