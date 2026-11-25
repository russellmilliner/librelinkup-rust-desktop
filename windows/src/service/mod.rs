//! Background service: owns all LibreView network I/O, polling, token re-authentication,
//! credential persistence, staleness checks and alert decisions.
//!
//! It runs on its own thread with a Tokio runtime and never depends on the UI being painted,
//! so alerts keep firing while the main window is hidden in the tray.

mod alerts;
mod commands;
mod sync;

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use librelinkup_core::analytics::{
    GlucoseHistoryStore, StatsWindow, save_default_glucose_history,
    try_load_default_glucose_history,
};
use librelinkup_core::api::{HaPatientOption, normalize_country_code};
use librelinkup_core::crypto::{
    StoredCredentials, load_credentials_with_secret, save_credentials_with_secret,
};
use librelinkup_core::event_log::{log_error, log_info, log_warn};
use librelinkup_core::medications::{
    MedicationSchedule, MedicationStore, save_default_medications, try_load_default_medications,
};
use librelinkup_core::models::{
    AlertKind, AppSettings, GlucoseSnapshot, jittered_poll_interval, load_default_settings,
    save_default_settings,
};
use librelinkup_core::paths::credentials_file_path;
use librelinkup_core::updater::UpdateCheckResult;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::time::{Instant, sleep_until};
use zeroize::Zeroizing;

use crate::platform;

/// State shared between the service thread, the UI and the tray.
#[derive(Debug, Clone, Default)]
pub struct Shared {
    pub data_rev: u64,
    pub settings: AppSettings,
    pub logged_in: bool,
    pub demo: bool,
    pub busy: bool,
    pub username: String,
    pub status: String,
    pub error: String,
    pub snapshot: Arc<GlucoseSnapshot>,
    pub stats_window: StatsWindow,
    pub glucose_history: Arc<GlucoseHistoryStore>,
    pub medication_store: Arc<MedicationStore>,
    pub history_vault_load_failed: bool,
    pub meds_vault_load_failed: bool,
    pub last_alert: Option<(AlertKind, i64)>,
    /// Epoch of the last successful CGM fetch (fail-closed staleness clock).
    pub last_successful_fetch_epoch: Option<i64>,
    /// Epoch of the last encrypted-history write (throttles multi-MB vault rewrites).
    pub last_history_persist_epoch: i64,
    pub vault_ok: bool,
    pub ha_token_configured: bool,
    pub discovered_ha_patients: Vec<HaPatientOption>,
    pub checking_updates: bool,
    pub installing_update: bool,
    pub last_notified_update_version: String,
    pub update_info: Option<UpdateCheckResult>,
}

pub type SharedState = Arc<Mutex<Shared>>;

pub enum Command {
    Login {
        username: String,
        password: Zeroizing<String>,
        country: String,
    },
    SaveHomeAssistantConfig {
        data_source: String,
        ha_url: String,
        ha_token: Zeroizing<String>,
        ha_sensor_entity_id: String,
    },
    AutoFillHomeAssistantFromEnv,
    SelectHaPatient(String),
    Logout,
    RefreshNow,
    /// Persist new settings (and re-evaluate alerts / tray).
    SaveSettings(Box<AppSettings>),
    /// Snooze non-urgent alerts for N minutes (<= 0 cancels).
    Snooze(i32),
    TestAlert,
    LoadDemo,
    CheckUpdates {
        manual: bool,
    },
    InstallUpdate,
    SetStatsWindow(StatsWindow),
    UpsertMedication {
        id: String,
        name: String,
        dosage: String,
        schedule: MedicationSchedule,
        scheduled_time: String,
        weekday: u8,
        reminder_enabled: bool,
        notes: String,
    },
    RemoveMedication(String),
    LogMedicationDose {
        medication_id: String,
        custom_dosage: String,
        notes: String,
    },
    SkipMedicationDose {
        medication_id: String,
        notes: String,
    },
    RemoveMedicationDose(String),
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login { country, .. } => f
                .debug_struct("Login")
                .field("username", &"[REDACTED]")
                .field("password", &"[REDACTED]")
                .field("country", country)
                .finish(),
            Self::SaveHomeAssistantConfig {
                data_source,
                ha_sensor_entity_id,
                ..
            } => f
                .debug_struct("SaveHomeAssistantConfig")
                .field("data_source", data_source)
                .field("ha_url", &"[REDACTED]")
                .field("ha_token", &"[REDACTED]")
                .field("ha_sensor_entity_id", ha_sensor_entity_id)
                .finish(),
            Self::AutoFillHomeAssistantFromEnv => write!(f, "AutoFillHomeAssistantFromEnv"),
            Self::SelectHaPatient(id) => f.debug_tuple("SelectHaPatient").field(id).finish(),
            Self::Logout => write!(f, "Logout"),
            Self::RefreshNow => write!(f, "RefreshNow"),
            Self::SaveSettings(_) => write!(f, "SaveSettings(..)"),
            Self::Snooze(m) => f.debug_tuple("Snooze").field(m).finish(),
            Self::TestAlert => write!(f, "TestAlert"),
            Self::LoadDemo => write!(f, "LoadDemo"),
            Self::CheckUpdates { manual } => f
                .debug_struct("CheckUpdates")
                .field("manual", manual)
                .finish(),
            Self::InstallUpdate => write!(f, "InstallUpdate"),
            Self::SetStatsWindow(w) => f.debug_tuple("SetStatsWindow").field(w).finish(),
            Self::UpsertMedication { schedule, .. } => f
                .debug_struct("UpsertMedication")
                .field("schedule", schedule)
                .field("phi", &"[REDACTED]")
                .finish(),
            Self::RemoveMedication(_) => write!(f, "RemoveMedication([REDACTED])"),
            Self::LogMedicationDose { .. } => write!(f, "LogMedicationDose([REDACTED])"),
            Self::SkipMedicationDose { .. } => write!(f, "SkipMedicationDose([REDACTED])"),
            Self::RemoveMedicationDose(_) => write!(f, "RemoveMedicationDose([REDACTED])"),
        }
    }
}

/// Callbacks into the UI layer (implemented in `main.rs`).
pub trait UiNotifier: Send + Sync {
    /// Shared state changed: repaint UI, overlay and tray.
    fn state_changed(&self);
    /// Out-of-range alert: flash and/or raise the main window.
    fn attention(&self, flash: bool, bring_to_front: bool);
}

pub fn now_epoch() -> i64 {
    chrono::Utc::now().timestamp()
}

fn lock(shared: &SharedState) -> std::sync::MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|p| p.into_inner())
}

/// Loads settings and starts the service thread. Returns the shared state and command sender.
/// Health-data vaults load asynchronously at startup on the service thread so the window opens cleanly.
pub fn start(notifier: Arc<dyn UiNotifier>) -> (SharedState, UnboundedSender<Command>) {
    let mut settings = load_default_settings();
    settings.country = normalize_country_code(&settings.country);
    // The registry Run key is the source of truth on Windows (the installer may set it).
    settings.autostart_enabled = platform::is_autostart_registered();
    let stats_window = StatsWindow::from_code(&settings.stats_window);
    let shared = Arc::new(Mutex::new(Shared {
        settings,
        stats_window,
        status: "Ready".into(),
        ..Default::default()
    }));
    let (tx, rx) = unbounded_channel();
    let thread_shared = shared.clone();
    let thread_tx = tx.clone();
    std::thread::Builder::new()
        .name("llu-service".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    log_error("SYSTEM", format!("Failed to start async runtime: {e}"));
                    return;
                }
            };
            runtime.block_on(
                Service {
                    shared: thread_shared,
                    notifier,
                    tx: thread_tx,
                    creds: None,
                    medication_last_notified: HashMap::new(),
                    sensor_expiry_last_notified_epoch: 0,
                    demo_backup: None,
                    pending_settings_save: None,
                }
                .run(rx),
            );
        })
        .expect("spawn service thread");
    (shared, tx)
}

fn vault_save_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn history_save_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn meds_save_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct Service {
    shared: SharedState,
    notifier: Arc<dyn UiNotifier>,
    tx: UnboundedSender<Command>,
    creds: Option<StoredCredentials>,
    medication_last_notified: HashMap<String, i64>,
    sensor_expiry_last_notified_epoch: i64,
    demo_backup: Option<(Arc<GlucoseHistoryStore>, Arc<MedicationStore>)>,
    pending_settings_save: Option<(Instant, AppSettings)>,
}

impl Service {
    pub(super) fn save_history_if_safe(s: &mut Shared, now: i64, blocking: bool) {
        if s.demo || s.history_vault_load_failed {
            return;
        }
        if !blocking {
            let hist = Arc::clone(&s.glucose_history);
            s.last_history_persist_epoch = now;
            tokio::task::spawn_blocking(move || {
                let _guard = history_save_lock()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if let Err(e) = save_default_glucose_history(&hist) {
                    log_error(
                        "CRYPTO",
                        format!("Failed to persist glucose history vault: {e}"),
                    );
                }
            });
            return;
        }
        let _guard = history_save_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match save_default_glucose_history(&s.glucose_history) {
            Ok(()) => s.last_history_persist_epoch = now,
            Err(e) => log_error(
                "CRYPTO",
                format!("Failed to persist glucose history vault: {e}"),
            ),
        }
    }

    pub(super) fn save_meds_if_safe(s: &Shared, blocking: bool) {
        if s.demo || s.meds_vault_load_failed {
            return;
        }
        if !blocking {
            let meds = Arc::clone(&s.medication_store);
            tokio::task::spawn_blocking(move || {
                let _guard = meds_save_lock().lock().unwrap_or_else(|e| e.into_inner());
                if let Err(e) = save_default_medications(&meds) {
                    log_error("CRYPTO", format!("Failed to persist medication vault: {e}"));
                }
            });
            return;
        }
        let _guard = meds_save_lock().lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = save_default_medications(&s.medication_store) {
            log_error("CRYPTO", format!("Failed to persist medication vault: {e}"));
        }
    }

    pub(super) fn restore_demo_backup_from(
        backup: &mut Option<(Arc<GlucoseHistoryStore>, Arc<MedicationStore>)>,
        s: &mut Shared,
    ) {
        if let Some((orig_hist, orig_meds)) = backup.take() {
            s.glucose_history = orig_hist;
            s.medication_store = orig_meds;
        }
    }

    pub(super) fn update_restore_demo(&mut self, f: impl FnOnce(&mut Shared)) {
        {
            let mut s = lock(&self.shared);
            Self::restore_demo_backup_from(&mut self.demo_backup, &mut s);
            f(&mut s);
            s.data_rev = s.data_rev.wrapping_add(1);
        }
        self.notifier.state_changed();
    }
    async fn run(mut self, mut rx: UnboundedReceiver<Command>) {
        log_info(
            "SYSTEM",
            format!(
                "Starting LibreLinkUp Rust Desktop v{} (Windows)",
                env!("CARGO_PKG_VERSION")
            ),
        );
        self.load_startup_vaults().await;
        if lock(&self.shared).settings.check_updates_on_startup {
            self.spawn_update_check(false);
        }

        let mut next_poll = Instant::now();
        let mut staleness = tokio::time::interval(Duration::from_secs(30));
        let mut update_check_timer = tokio::time::interval(Duration::from_secs(6 * 3600));
        update_check_timer.tick().await; // consume initial immediate tick
        loop {
            let (data_source, ha_url) = {
                let s = lock(&self.shared);
                (s.settings.data_source.clone(), s.settings.ha_url.clone())
            };
            let polling = self
                .creds
                .as_ref()
                .is_some_and(|c| c.has_valid_session_for(&data_source, &ha_url));
            tokio::select! {
                cmd = rx.recv() => {
                    let Some(cmd) = cmd else { break };
                    let refresh_after = self.handle(cmd).await;
                    if refresh_after {
                        next_poll = Instant::now();
                    }
                }
                _ = sleep_until(next_poll), if polling => {
                    self.refresh().await;
                    next_poll = Instant::now() + self.poll_interval();
                }
                _ = async {
                    if let Some((deadline, _)) = self.pending_settings_save {
                        sleep_until(deadline).await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    if let Some((_, to_save)) = self.pending_settings_save.take() {
                        tokio::task::spawn_blocking(move || {
                            if let Err(e) = save_default_settings(&to_save) {
                                log_error("SYSTEM", format!("Failed to save settings: {e}"));
                            }
                        });
                    }
                }
                _ = staleness.tick() => {
                    self.evaluate_alerts();
                    self.notifier.state_changed();
                }
                _ = update_check_timer.tick() => {
                    if lock(&self.shared).settings.check_updates_on_startup {
                        self.spawn_update_check(false);
                    }
                }
            }
        }
        if let Some((_, to_save)) = self.pending_settings_save.take() {
            let _ = save_default_settings(&to_save);
        }
        let now = now_epoch();
        let mut s = lock(&self.shared);
        Self::restore_demo_backup_from(&mut self.demo_backup, &mut s);
        if !s.glucose_history.samples.is_empty() {
            Self::save_history_if_safe(&mut s, now, true);
        }
    }

    async fn load_startup_vaults(&mut self) {
        let (hist_res, meds_res) = tokio::task::spawn_blocking(|| {
            (
                try_load_default_glucose_history(),
                try_load_default_medications(),
            )
        })
        .await
        .unwrap_or_else(|e| {
            (
                Err(format!("vault task panic: {e}")),
                Err(format!("vault task panic: {e}")),
            )
        });

        let (glucose_history, history_vault_load_failed) = match hist_res {
            Ok(store) => (store, false),
            Err(e) => {
                log_error(
                    "CRYPTO",
                    format!(
                        "Could not open glucose history vault (refusing to overwrite vault on disk): {e}"
                    ),
                );
                (GlucoseHistoryStore::default(), true)
            }
        };
        let (medication_store, meds_vault_load_failed) = match meds_res {
            Ok(store) => (store, false),
            Err(e) => {
                log_error(
                    "CRYPTO",
                    format!(
                        "Could not open medication vault (refusing to overwrite vault on disk): {e}"
                    ),
                );
                (MedicationStore::default(), true)
            }
        };
        {
            let mut s = lock(&self.shared);
            if !s.demo {
                s.glucose_history = Arc::new(glucose_history);
                if s.medication_store.medications.is_empty()
                    && s.medication_store.dose_logs.is_empty()
                {
                    s.medication_store = Arc::new(medication_store);
                }
            }
            s.history_vault_load_failed = history_vault_load_failed;
            s.meds_vault_load_failed = meds_vault_load_failed;
            s.data_rev = s.data_rev.wrapping_add(1);
        }
        self.notifier.state_changed();
        self.load_vault();
    }

    fn poll_interval(&self) -> Duration {
        // Jitter (+/-10%) so restarts and multiple instances never poll in lock-step.
        let secs = jittered_poll_interval(lock(&self.shared).settings.poll_interval_seconds);
        Duration::from_secs(secs as u64)
    }

    pub(super) fn update(&self, f: impl FnOnce(&mut Shared)) {
        {
            let mut s = lock(&self.shared);
            f(&mut s);
            s.data_rev = s.data_rev.wrapping_add(1);
        }
        self.notifier.state_changed();
    }

    fn load_vault(&mut self) {
        let secret = match platform::load_or_create_master_secret() {
            Ok(s) => s,
            Err(e) => {
                log_error("CRYPTO", format!("Cannot access DPAPI master key: {e}"));
                self.update(|s| s.error = e);
                return;
            }
        };
        let (data_source, ha_url) = {
            let s = lock(&self.shared);
            (s.settings.data_source.clone(), s.settings.ha_url.clone())
        };
        match load_credentials_with_secret(&credentials_file_path(), &secret) {
            Ok(Some(mut creds)) if creds.has_valid_session_for(&data_source, &ha_url) => {
                let normalized_country = normalize_country_code(&creds.country);
                if normalized_country != creds.country {
                    log_info(
                        "CRYPTO",
                        format!(
                            "Normalized stored credential region '{}' -> '{}'",
                            creds.country, normalized_country
                        ),
                    );
                    creds.country = normalized_country;
                    self.save_vault(&creds);
                }
                log_info(
                    "CRYPTO",
                    format!(
                        "Loaded DPAPI-protected credentials (source '{}', region '{}').",
                        data_source, creds.country
                    ),
                );
                let username = creds.username.clone();
                let active_country = creds.country.clone();
                let has_ha_tok = creds.has_valid_ha_session();
                self.creds = Some(creds);
                self.update(|s| {
                    s.logged_in = true;
                    s.vault_ok = true;
                    s.ha_token_configured = has_ha_tok;
                    s.username = username;
                    s.settings.country = active_country;
                    s.status = if s.settings.data_source == "home_assistant" {
                        "Connected to Home Assistant — fetching glucose data...".into()
                    } else {
                        "Signed in — fetching glucose data...".into()
                    };
                });
                let settings = lock(&self.shared).settings.clone();
                let _ = save_default_settings(&settings);
            }
            Ok(Some(creds)) => {
                let has_ha_tok = creds.has_valid_ha_session();
                let has_any = creds.has_valid_llu_session() || has_ha_tok;
                self.creds = Some(creds);
                log_info(
                    "AUTH",
                    "No active session for current source — awaiting configuration.",
                );
                self.update(|s| {
                    s.vault_ok = has_any;
                    s.ha_token_configured = has_ha_tok;
                    s.status = if s.settings.data_source == "home_assistant" {
                        "Configure Home Assistant URL and Long-Lived Access Token".into()
                    } else {
                        "Please sign in to LibreLinkUp".into()
                    };
                });
            }
            Ok(None) => {
                log_info("AUTH", "No stored session — awaiting sign-in.");
                self.update(|s| s.status = "Please sign in to LibreLinkUp".into());
            }
            Err(e) => {
                log_warn(
                    "CRYPTO",
                    format!("Could not decrypt stored credentials: {e}"),
                );
                self.update(|s| s.error = e);
            }
        }
    }

    pub(super) fn save_vault(&self, creds: &StoredCredentials) {
        let remember_pw = lock(&self.shared).settings.remember_password;
        let sanitized = creds.sanitized_for_storage(remember_pw);
        tokio::task::spawn_blocking(move || {
            let _guard = vault_save_lock().lock().unwrap_or_else(|e| e.into_inner());
            let result = platform::load_or_create_master_secret().and_then(|secret| {
                save_credentials_with_secret(&sanitized, &credentials_file_path(), &secret)
            });
            match result {
                Ok(()) => log_info(
                    "CRYPTO",
                    "Saved credentials (AES-256-GCM, key protected by Windows DPAPI).",
                ),
                Err(e) => log_error("CRYPTO", format!("Failed to save credentials: {e}")),
            }
        });
    }
}
