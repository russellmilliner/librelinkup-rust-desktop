//! Glycemic analytics: Estimated A1C (`eA1C` via ADAG formula), Glucose Management Indicator
//! (`GMI` via Bergenstal 2018 CGM formula), Time-in-Range (`TIR %`), Coefficient of Variation
//! (`CV %`), and a persistent 90-day rolling local glucose sample store.

use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::crypto::{
    decrypt_bytes_with_secret, encrypt_bytes_with_secret, phi_secret, AAD_LABEL_GLUCOSE_HISTORY,
};
use crate::event_log::{log_error, log_info};
use crate::models::{format_glucose_value, parse_libreview_timestamp, GlucoseSnapshot};
use crate::paths::{
    glucose_history_file_path, legacy_glucose_history_file_path, write_file_atomic_0600,
};

/// Maximum number of days retained in the rolling local glucose history file.
pub const MAX_HISTORY_DAYS: i64 = 90;
/// Maximum number of deduplicated minute-bucketed samples retained (~90 days at 1-min resolution).
pub const MAX_HISTORY_SAMPLES: usize = 135_000;
/// Minimum seconds between encrypted-history vault rewrites.
///
/// Ingestion happens on every poll, but at full history the vault is several megabytes, so
/// persist at most this often (front-ends force a write on sign-out/quit).
pub const HISTORY_PERSIST_MIN_INTERVAL_SECS: i64 = 300;

/// Supported time windows for computing eA1C, GMI, and Time-in-Range statistics.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatsWindow {
    #[default]
    Hours12,
    Days7,
    Days14,
    Days30,
    Days90,
}

impl StatsWindow {
    pub const ALL: [Self; 5] = [
        Self::Hours12,
        Self::Days7,
        Self::Days14,
        Self::Days30,
        Self::Days90,
    ];

    pub fn from_code(code: &str) -> Self {
        match code.trim().to_ascii_lowercase().as_str() {
            "12h" | "hours12" => Self::Hours12,
            "7d" | "days7" => Self::Days7,
            "14d" | "days14" => Self::Days14,
            "30d" | "days30" => Self::Days30,
            "90d" | "days90" => Self::Days90,
            _ => Self::Days14,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Hours12 => "12h",
            Self::Days7 => "7d",
            Self::Days14 => "14d",
            Self::Days30 => "30d",
            Self::Days90 => "90d",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Hours12 => "12 Hours",
            Self::Days7 => "7 Days",
            Self::Days14 => "14 Days",
            Self::Days30 => "30 Days",
            Self::Days90 => "90 Days",
        }
    }

    pub fn duration_seconds(&self) -> i64 {
        match self {
            Self::Hours12 => 12 * 3600,
            Self::Days7 => 7 * 86_400,
            Self::Days14 => 14 * 86_400,
            Self::Days30 => 30 * 86_400,
            Self::Days90 => 90 * 86_400,
        }
    }
}

/// Calculates Estimated A1C (`eA1C %`, NGSP) from mean blood glucose (`mg/dL`)
/// using the Nathan et al. (2008) ADAG linear regression:
/// `eA1C (%) = (mean_mg_dl + 46.7) / 28.7`.
pub fn calculate_ea1c_adag(mean_mg_dl: f64) -> f64 {
    if mean_mg_dl <= 0.0 || mean_mg_dl.is_nan() {
        return 0.0;
    }
    (mean_mg_dl + 46.7) / 28.7
}

/// Calculates the Glucose Management Indicator (`GMI %`, NGSP) from mean CGM glucose (`mg/dL`)
/// using the Bergenstal et al. (2018) CGM standard formula:
/// `GMI (%) = 3.31 + 0.02392 * mean_mg_dl`.
pub fn calculate_gmi(mean_mg_dl: f64) -> f64 {
    if mean_mg_dl <= 0.0 || mean_mg_dl.is_nan() {
        return 0.0;
    }
    3.31 + 0.02392 * mean_mg_dl
}

/// Converts an NGSP A1C percentage (e.g. `5.7%`) to IFCC `mmol/mol` (`(pct - 2.15) * 10.929`).
pub fn a1c_percent_to_mmol_mol(a1c_percent: f64) -> f64 {
    if a1c_percent <= 2.15 || a1c_percent.is_nan() {
        return 0.0;
    }
    (a1c_percent - 2.15) * 10.929
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct HistorySample {
    /// Unix epoch timestamp in seconds (binned to 60s resolution on ingestion).
    pub epoch: i64,
    /// Blood glucose reading in `mg/dL`.
    pub value_mg_dl: f64,
}

impl fmt::Debug for HistorySample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HistorySample")
            .field("epoch", &self.epoch)
            .field("value_mg_dl", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GlucoseHistoryStore {
    #[serde(default)]
    pub samples: Vec<HistorySample>,
}

impl fmt::Debug for GlucoseHistoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GlucoseHistoryStore")
            .field("samples_len", &self.samples.len())
            .finish()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GlycemicStats {
    pub window: Option<StatsWindow>,
    pub sample_count: usize,
    pub span_hours: f64,
    pub mean_mg_dl: f64,
    pub std_dev_mg_dl: f64,
    pub cv_percent: f64,
    pub ea1c_percent: f64,
    pub ea1c_mmol_mol: f64,
    pub gmi_percent: f64,
    pub gmi_mmol_mol: f64,
    pub tir_in_range_percent: f64,
    pub tir_below_percent: f64,
    pub tir_above_percent: f64,
}

impl GlycemicStats {
    pub fn has_data(&self) -> bool {
        self.sample_count > 0 && self.mean_mg_dl > 0.0
    }

    /// Confidence descriptor based on the actual time span covered by collected samples.
    pub fn confidence_label(&self) -> &'static str {
        if !self.has_data() {
            "Waiting for readings"
        } else if self.span_hours >= 14.0 * 24.0 {
            "High confidence (14+ days)"
        } else if self.span_hours >= 7.0 * 24.0 {
            "Moderate confidence (7–13 days)"
        } else if self.span_hours >= 24.0 {
            "Building history (1–6 days)"
        } else {
            "Live 12h estimate"
        }
    }

    /// Formats a concise headline badge for the Dashboard card, e.g.:
    /// `"eA1C 5.6% (38 mmol/mol) • GMI 6.0% • TIR 92%"`
    pub fn format_headline(&self) -> String {
        if !self.has_data() {
            return "eA1C --% • GMI --% • TIR --%".to_string();
        }
        format!(
            "eA1C {:.1}% ({:.0} mmol/mol) • GMI {:.1}% • TIR {:.0}%",
            self.ea1c_percent, self.ea1c_mmol_mol, self.gmi_percent, self.tir_in_range_percent
        )
    }

    /// Formats a secondary metrics line for the Dashboard card, e.g.:
    /// `"Avg 115 mg/dL • CV 14.2% • Low 0% / High 8% • 24 readings (12h)"`
    pub fn format_subline(&self, unit: &str) -> String {
        if !self.has_data() {
            return "Accumulating CGM readings for A1C / GMI prediction...".to_string();
        }
        let avg_str = format_glucose_value(self.mean_mg_dl, unit);
        let span_str = if self.span_hours >= 48.0 {
            format!("{:.1}d span", self.span_hours / 24.0)
        } else if self.span_hours >= 1.0 {
            format!("{:.1}h span", self.span_hours)
        } else {
            "latest".to_string()
        };
        format!(
            "Avg {} {} • CV {:.1}% • Low {:.0}% / High {:.0}% • {} pts ({}, {})",
            avg_str,
            unit,
            self.cv_percent,
            self.tir_below_percent,
            self.tir_above_percent,
            self.sample_count,
            span_str,
            self.confidence_label()
        )
    }
}

impl GlucoseHistoryStore {
    /// Ingests all graph points and the current reading from `snapshot` into the rolling store.
    /// Deduplicates by 60-second minute buckets and prunes samples older than 90 days.
    ///
    /// Returns `true` **only when the stored data actually changed**, so callers can avoid
    /// rewriting a multi-megabyte vault on every poll (see the persistence throttle in the
    /// front-ends).
    pub fn ingest_snapshot(&mut self, snapshot: &GlucoseSnapshot, now_epoch: i64) -> bool {
        let cutoff = now_epoch - MAX_HISTORY_DAYS * 86_400;
        let mut changed = false;

        let mut incoming: Vec<HistorySample> = Vec::with_capacity(snapshot.graph_points.len() + 1);
        for (idx, pt) in snapshot.graph_points.iter().enumerate() {
            if pt.value_mg_dl.is_nan() || pt.value_mg_dl <= 0.0 {
                continue;
            }
            let epoch = parse_libreview_timestamp(&pt.timestamp, false).unwrap_or_else(|| {
                let remaining = snapshot.graph_points.len().saturating_sub(1 + idx) as i64;
                now_epoch - remaining * 15 * 60
            });
            if epoch >= cutoff {
                incoming.push(HistorySample {
                    epoch: (epoch / 60) * 60,
                    value_mg_dl: pt.value_mg_dl,
                });
            }
        }

        if snapshot.value_mg_dl > 0.0 && !snapshot.value_mg_dl.is_nan() {
            let cur_epoch = snapshot
                .measured_at_epoch
                .or_else(|| parse_libreview_timestamp(&snapshot.timestamp, false))
                .unwrap_or(now_epoch);
            if cur_epoch >= cutoff {
                incoming.push(HistorySample {
                    epoch: (cur_epoch / 60) * 60,
                    value_mg_dl: snapshot.value_mg_dl,
                });
            }
        }

        // Merge in place: `self.samples` stays sorted by epoch, so each incoming sample is
        // either a new bucket (insert) or an update to an existing bucket (replace). This is
        // O(n log n) per inserted sample instead of re-sorting the whole vector every poll.
        incoming.sort_by_key(|s| s.epoch);
        for sample in incoming {
            match self.samples.binary_search_by_key(&sample.epoch, |s| s.epoch) {
                Ok(idx) => {
                    if self.samples[idx].value_mg_dl != sample.value_mg_dl {
                        self.samples[idx].value_mg_dl = sample.value_mg_dl;
                        changed = true;
                    }
                }
                Err(idx) => {
                    self.samples.insert(idx, sample);
                    changed = true;
                }
            }
        }

        let before_prune = self.samples.len();
        self.samples
            .retain(|s| s.epoch >= cutoff && s.value_mg_dl > 0.0);
        if self.samples.len() != before_prune {
            changed = true;
        }

        if self.samples.len() > MAX_HISTORY_SAMPLES {
            let excess = self.samples.len() - MAX_HISTORY_SAMPLES;
            self.samples.drain(0..excess);
            changed = true;
        }

        changed
    }

    /// Computes [`GlycemicStats`] over `window` ending at `now_epoch`.
    /// If the persistent store has no samples in `window` yet, falls back to `snapshot`.
    pub fn compute_stats(
        &self,
        window: StatsWindow,
        now_epoch: i64,
        target_low_mg_dl: f64,
        target_high_mg_dl: f64,
        fallback_snapshot: Option<&GlucoseSnapshot>,
    ) -> GlycemicStats {
        let cutoff = now_epoch - window.duration_seconds();
        let mut values: Vec<(i64, f64)> = self
            .samples
            .iter()
            .filter(|s| s.epoch >= cutoff && s.epoch <= now_epoch + 3600 && s.value_mg_dl > 0.0)
            .map(|s| (s.epoch, s.value_mg_dl))
            .collect();

        if values.is_empty() {
            if let Some(snap) = fallback_snapshot {
                for (idx, pt) in snap.graph_points.iter().enumerate() {
                    if pt.value_mg_dl > 0.0 && !pt.value_mg_dl.is_nan() {
                        let ep = parse_libreview_timestamp(&pt.timestamp, false).unwrap_or_else(
                            || {
                                let rem = snap.graph_points.len().saturating_sub(1 + idx) as i64;
                                now_epoch - rem * 15 * 60
                            },
                        );
                        values.push((ep, pt.value_mg_dl));
                    }
                }
                if values.is_empty() && snap.value_mg_dl > 0.0 {
                    values.push((
                        snap.measured_at_epoch.unwrap_or(now_epoch),
                        snap.value_mg_dl,
                    ));
                }
            }
        }

        if values.is_empty() {
            return GlycemicStats {
                window: Some(window),
                ..Default::default()
            };
        }

        let n = values.len() as f64;
        let min_epoch = values.iter().map(|(e, _)| *e).min().unwrap_or(now_epoch);
        let max_epoch = values.iter().map(|(e, _)| *e).max().unwrap_or(now_epoch);
        let span_hours = ((max_epoch - min_epoch).max(0) as f64) / 3600.0;

        let sum: f64 = values.iter().map(|(_, v)| *v).sum();
        let mean_mg_dl = sum / n;

        let variance: f64 = values
            .iter()
            .map(|(_, v)| {
                let d = *v - mean_mg_dl;
                d * d
            })
            .sum::<f64>()
            / n;
        let std_dev_mg_dl = variance.sqrt();
        let cv_percent = if mean_mg_dl > 0.0 {
            (std_dev_mg_dl / mean_mg_dl) * 100.0
        } else {
            0.0
        };

        let low = if target_low_mg_dl > 0.0 {
            target_low_mg_dl
        } else {
            70.0
        };
        let high = if target_high_mg_dl > low {
            target_high_mg_dl
        } else {
            180.0
        };

        let mut below_count = 0usize;
        let mut above_count = 0usize;
        let mut in_range_count = 0usize;
        for (_, v) in &values {
            if *v < low {
                below_count += 1;
            } else if *v > high {
                above_count += 1;
            } else {
                in_range_count += 1;
            }
        }

        let ea1c_percent = calculate_ea1c_adag(mean_mg_dl);
        let gmi_percent = calculate_gmi(mean_mg_dl);

        GlycemicStats {
            window: Some(window),
            sample_count: values.len(),
            span_hours,
            mean_mg_dl,
            std_dev_mg_dl,
            cv_percent,
            ea1c_percent,
            ea1c_mmol_mol: a1c_percent_to_mmol_mol(ea1c_percent),
            gmi_percent,
            gmi_mmol_mol: a1c_percent_to_mmol_mol(gmi_percent),
            tir_in_range_percent: (in_range_count as f64 / n) * 100.0,
            tir_below_percent: (below_count as f64 / n) * 100.0,
            tir_above_percent: (above_count as f64 / n) * 100.0,
        }
    }
}

/// Loads a glucose history vault, returning an error string on decryption failure.
pub fn try_load_glucose_history_from_path_with_secret(
    path: &Path,
    secret: &[u8],
) -> Result<Option<GlucoseHistoryStore>, String> {
    use zeroize::Zeroize;

    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read glucose history vault: {e}"))?;
    let envelope: crate::crypto::EncryptedEnvelope = serde_json::from_str(&raw)
        .map_err(|e| format!("Corrupted glucose history envelope: {e}"))?;
    let mut plaintext = decrypt_bytes_with_secret(&envelope, secret, AAD_LABEL_GLUCOSE_HISTORY)?;
    let parsed = serde_json::from_slice::<GlucoseHistoryStore>(&plaintext)
        .map_err(|e| format!("Corrupted glucose history payload: {e}"));
    plaintext.zeroize();
    let mut store = parsed?;
    // Guarantee the sorted-by-epoch invariant used by `ingest_snapshot`.
    store.samples.retain(|s| s.value_mg_dl > 0.0);
    store.samples.sort_by_key(|s| s.epoch);
    Ok(Some(store))
}

pub fn load_glucose_history_from_path(path: &Path) -> GlucoseHistoryStore {
    match phi_secret()
        .and_then(|secret| try_load_glucose_history_from_path_with_secret(path, &secret))
    {
        Ok(Some(store)) => store,
        Ok(None) => GlucoseHistoryStore::default(),
        Err(e) => {
            log_error("CRYPTO", format!("Could not open glucose history vault: {e}"));
            GlucoseHistoryStore::default()
        }
    }
}

pub fn save_glucose_history_to_path(
    store: &GlucoseHistoryStore,
    path: &Path,
) -> Result<(), String> {
    let secret = phi_secret()?;
    save_glucose_history_to_path_with_secret(store, path, &secret)
}

pub fn save_glucose_history_to_path_with_secret(
    store: &GlucoseHistoryStore,
    path: &Path,
    secret: &[u8],
) -> Result<(), String> {
    use zeroize::Zeroize;

    let mut json = serde_json::to_vec(store)
        .map_err(|e| format!("Failed to serialize glucose history: {e}"))?;
    let res = encrypt_bytes_with_secret(&json, secret, AAD_LABEL_GLUCOSE_HISTORY).and_then(
        |envelope| {
            let serialized = serde_json::to_string(&envelope)
                .map_err(|e| format!("Failed to serialize glucose history envelope: {e}"))?;
            write_file_atomic_0600(path, serialized.as_bytes())
                .map_err(|e| format!("Failed to write glucose history vault: {e}"))
        },
    );
    json.zeroize();
    res
}

/// Loads the 90-day history vault, transparently migrating a pre-0.5 plaintext
/// `glucose_history.json` into the encrypted vault and deleting the plaintext copy.
pub fn load_default_glucose_history() -> GlucoseHistoryStore {
    let enc_path = glucose_history_file_path();
    if enc_path.exists() {
        return load_glucose_history_from_path(&enc_path);
    }

    let legacy_path = legacy_glucose_history_file_path();
    if let Ok(raw) = fs::read_to_string(&legacy_path) {
        if let Ok(mut store) = serde_json::from_str::<GlucoseHistoryStore>(&raw) {
            store.samples.retain(|s| s.value_mg_dl > 0.0);
            store.samples.sort_by_key(|s| s.epoch);
            match save_glucose_history_to_path(&store, &enc_path) {
                Ok(()) => {
                    let _ = fs::remove_file(&legacy_path);
                    log_info(
                        "CRYPTO",
                        format!(
                            "Migrated {} glucose samples from plaintext to the encrypted history vault.",
                            store.samples.len()
                        ),
                    );
                }
                Err(e) => {
                    log_error("CRYPTO", format!("Could not migrate plaintext glucose history: {e}"));
                }
            }
            return store;
        }
    }
    GlucoseHistoryStore::default()
}

pub fn save_default_glucose_history(store: &GlucoseHistoryStore) -> Result<(), String> {
    save_glucose_history_to_path(store, &glucose_history_file_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::build_demo_snapshot;

    #[test]
    fn ea1c_and_gmi_formulas_match_published_clinical_tables() {
        // Mean 126 mg/dL -> ADAG eA1C = (126 + 46.7) / 28.7 = 6.017% (~6.0%)
        let ea1c_126 = calculate_ea1c_adag(126.0);
        assert!((ea1c_126 - 6.0174).abs() < 0.01);

        // Mean 154 mg/dL -> ADAG eA1C = (154 + 46.7) / 28.7 = 6.993% (~7.0%)
        let ea1c_154 = calculate_ea1c_adag(154.0);
        assert!((ea1c_154 - 6.993).abs() < 0.01);

        // Mean 150 mg/dL -> Bergenstal GMI = 3.31 + 0.02392 * 150 = 6.898% (~6.9%)
        let gmi_150 = calculate_gmi(150.0);
        assert!((gmi_150 - 6.898).abs() < 0.01);

        // 7.0% NGSP -> ~53 mmol/mol IFCC
        let ifcc_7 = a1c_percent_to_mmol_mol(7.0);
        assert!((ifcc_7 - 53.0).abs() < 0.5);
    }

    #[test]
    fn history_ingests_deduplicates_prunes_and_computes_window_stats() {
        let now = 1_750_000_000;
        let snap = build_demo_snapshot(now);
        let mut store = GlucoseHistoryStore::default();

        // Add an expired sample older than 90 days that should be pruned.
        store.samples.push(HistorySample {
            epoch: now - 95 * 86_400,
            value_mg_dl: 250.0,
        });

        assert!(store.ingest_snapshot(&snap, now), "first ingest changes data");
        let count_after_first = store.samples.len();
        assert!(count_after_first >= 20);
        assert!(store.samples.iter().all(|s| s.epoch >= now - 90 * 86_400));

        // Re-ingesting identical timestamps/values is a no-op: this is what prevents the
        // multi-megabyte vault rewrite on every 60 s poll.
        assert!(
            !store.ingest_snapshot(&snap, now),
            "identical re-ingest must report no change"
        );
        assert_eq!(store.samples.len(), count_after_first);

        // A genuinely new reading reports a change and is inserted in epoch order.
        let mut newer = snap.clone();
        newer.value_mg_dl = 121.0;
        newer.measured_at_epoch = Some(now + 60);
        assert!(store.ingest_snapshot(&newer, now + 60));
        assert_eq!(store.samples.len(), count_after_first + 1);
        assert!(store.samples.windows(2).all(|w| w[0].epoch <= w[1].epoch));

        let stats = store.compute_stats(StatsWindow::Days14, now, 70.0, 180.0, Some(&snap));
        assert!(stats.has_data());
        assert!(stats.ea1c_percent > 5.0 && stats.ea1c_percent < 7.0);
        assert!(stats.gmi_percent > 5.5 && stats.gmi_percent < 7.0);
        assert!(
            stats.tir_in_range_percent > 95.0,
            "TIR should be ~100% for the demo series, got {}",
            stats.tir_in_range_percent
        );
        assert!(stats.format_headline().contains("eA1C"));
        assert!(stats.format_headline().contains("GMI"));
        assert!(stats.format_subline("mg/dL").contains("Avg"));

        // Ensure Debug redacts values
        let dbg = format!("{:?}", store.samples[0]);
        assert!(dbg.contains("[REDACTED]"));
    }

    #[test]
    fn history_vault_is_encrypted_at_rest_and_round_trips() {
        let dir = std::env::temp_dir().join(format!("llu-histvault-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("glucose_history.enc.json");
        let secret = b"history-test-secret";

        let now = 1_750_000_000;
        let mut store = GlucoseHistoryStore::default();
        store.ingest_snapshot(&build_demo_snapshot(now), now);

        save_glucose_history_to_path_with_secret(&store, &path, secret).unwrap();

        // No plaintext reading values or timestamps may appear in the file.
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"ciphertext_hex\""), "expected an encrypted envelope");
        assert!(!raw.contains("\"value_mg_dl\""), "plaintext samples leaked to disk");
        assert!(!raw.contains("112.0"));

        let loaded = try_load_glucose_history_from_path_with_secret(&path, secret)
            .unwrap()
            .expect("vault present");
        assert_eq!(loaded.samples.len(), store.samples.len());
        assert_eq!(loaded.samples, store.samples);

        // Wrong key fails closed rather than silently returning an empty history.
        assert!(try_load_glucose_history_from_path_with_secret(&path, b"wrong").is_err());

        // A credentials vault must not decrypt as a history vault (AAD label binding).
        let creds_env = crate::crypto::encrypt_bytes_with_secret(
            b"{}",
            secret,
            crate::crypto::AAD_LABEL_CREDENTIALS,
        )
        .unwrap();
        fs::write(&path, serde_json::to_string(&creds_env).unwrap()).unwrap();
        assert!(try_load_glucose_history_from_path_with_secret(&path, secret).is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}
