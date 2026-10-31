//! Background service: owns all LibreView network I/O, polling, token re-authentication,
//! credential persistence, staleness checks and alert decisions.
//!
//! It runs on its own thread with a Tokio runtime and never depends on the UI being painted,
//! so alerts keep firing while the main window is hidden in the tray.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use librelinkup_core::analytics::{
    load_default_glucose_history, save_default_glucose_history, GlucoseHistoryStore, StatsWindow,
};
use librelinkup_core::api::{
    authenticate_async, discover_ha_patients, fetch_ha_states_array_async, normalize_country_code,
    parse_ha_remote_snooze, publish_ha_medications_state_async, publish_ha_snooze_state_async,
    validate_ha_base_url, HaPatientOption,
};
use librelinkup_core::crypto::{
    clear_credentials_at_path, load_credentials_with_secret, save_credentials_with_secret,
    try_load_host_ha_env, StoredCredentials,
};
use librelinkup_core::event_log::{log_error, log_info, log_warn};
use librelinkup_core::medications::{
    build_demo_medication_store, load_default_medications, save_default_medications,
    MedicationDoseLog, MedicationSchedule, MedicationStore,
};
use librelinkup_core::models::{
    clamp_poll_interval, format_glucose_value, jittered_poll_interval, load_default_settings,
    save_default_settings, trend_arrow_symbol, AlertKind, AppSettings, GlucoseSnapshot,
};
use librelinkup_core::paths::credentials_file_path;
use librelinkup_core::session::{
    build_demo_snapshot, evaluate_alert_decision, fetch_snapshot_for_source_async,
};
use librelinkup_core::updater::{
    check_for_updates_async, download_and_install_update_async, UpdateCheckResult,
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::{sleep_until, Instant};

use crate::{alerts, platform};

/// State shared between the service thread, the UI and the tray.
#[derive(Debug, Clone, Default)]
pub struct Shared {
    pub settings: AppSettings,
    pub logged_in: bool,
    pub demo: bool,
    pub busy: bool,
    pub username: String,
    pub status: String,
    pub error: String,
    pub snapshot: GlucoseSnapshot,
    pub stats_window: StatsWindow,
    pub glucose_history: GlucoseHistoryStore,
    pub medication_store: MedicationStore,
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

#[derive(Debug)]
pub enum Command {
    Login { username: String, password: String, country: String },
    SaveHomeAssistantConfig {
        data_source: String,
        ha_url: String,
        ha_token: String,
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
    CheckUpdates { manual: bool },
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
pub fn start(notifier: Arc<dyn UiNotifier>) -> (SharedState, UnboundedSender<Command>) {
    let mut settings = load_default_settings();
    settings.country = normalize_country_code(&settings.country);
    // The registry Run key is the source of truth on Windows (the installer may set it).
    settings.autostart_enabled = platform::is_autostart_registered();
    let shared = Arc::new(Mutex::new(Shared {
        settings,
        stats_window: StatsWindow::Hours12,
        glucose_history: load_default_glucose_history(),
        medication_store: load_default_medications(),
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
                }
                .run(rx),
            );
        })
        .expect("spawn service thread");
    (shared, tx)
}

struct Service {
    shared: SharedState,
    notifier: Arc<dyn UiNotifier>,
    tx: UnboundedSender<Command>,
    creds: Option<StoredCredentials>,
    medication_last_notified: HashMap<String, i64>,
    sensor_expiry_last_notified_epoch: i64,
}

impl Service {
    async fn run(mut self, mut rx: UnboundedReceiver<Command>) {
        log_info(
            "SYSTEM",
            format!("Starting LibreLinkUp Rust Desktop v{} (Windows)", env!("CARGO_PKG_VERSION")),
        );
        self.load_vault();
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
    }

    fn poll_interval(&self) -> Duration {
        // Jitter (+/-10%) so restarts and multiple instances never poll in lock-step.
        let secs = jittered_poll_interval(lock(&self.shared).settings.poll_interval_seconds);
        Duration::from_secs(secs as u64)
    }

    fn update(&self, f: impl FnOnce(&mut Shared)) {
        f(&mut lock(&self.shared));
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
                log_info("AUTH", "No active session for current source — awaiting configuration.");
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
                log_warn("CRYPTO", format!("Could not decrypt stored credentials: {e}"));
                self.update(|s| s.error = e);
            }
        }
    }

    fn save_vault(&self, creds: &StoredCredentials) {
        let remember_pw = lock(&self.shared).settings.remember_password;
        let sanitized = creds.sanitized_for_storage(remember_pw);
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
    }

    /// Returns true if an immediate refresh should follow.
    async fn handle(&mut self, cmd: Command) -> bool {
        match cmd {
            Command::Login { username, password, country } => {
                self.login(username, password, country).await
            }
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
                        if clean.is_empty() { "auto-discover" } else { &clean }
                    ),
                );
                self.notifier.state_changed();
                should_refresh
            }
            Command::Logout => {
                let _ = clear_credentials_at_path(&credentials_file_path());
                self.creds = None;
                log_info("AUTH", "Signed out; encrypted vault deleted.");
                self.update(|s| {
                    s.logged_in = false;
                    s.demo = false;
                    s.vault_ok = false;
                    s.ha_token_configured = false;
                    s.username.clear();
                    s.snapshot = GlucoseSnapshot::default();
                    s.last_alert = None;
                    s.last_successful_fetch_epoch = None;
                    s.error.clear();
                    s.status = "Signed out — encrypted vault cleared.".into();
                });
                false
            }
            Command::RefreshNow => true,
            Command::SaveSettings(settings) => {
                let mut settings = *settings;
                settings.country = normalize_country_code(&settings.country);
                settings.poll_interval_seconds = clamp_poll_interval(settings.poll_interval_seconds);
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
                if let Err(e) = save_default_settings(&settings) {
                    log_error("SYSTEM", format!("Failed to save settings: {e}"));
                }
                self.update(|s| s.settings = settings);
                if country_changed || remember_changed {
                    if let Some(ref creds) = self.creds {
                        self.save_vault(creds);
                    }
                }
                self.evaluate_alerts();
                country_changed
            }
            Command::Snooze(minutes) => {
                let now = now_epoch();
                let until = if minutes > 0 { now + i64::from(minutes.min(1440)) * 60 } else { 0 };
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
                    log_info("ALERT", format!("Alerts snoozed for {minutes} min (urgent low still alerts)."));
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
                self.update(|s| {
                    s.demo = true;
                    s.logged_in = true;
                    s.status = "Sample CGM telemetry loaded".into();
                    s.glucose_history.ingest_snapshot(&snap, now);
                    if s.medication_store.medications.is_empty() {
                        s.medication_store = build_demo_medication_store(now);
                    }
                    s.snapshot = snap;
                });
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
                log_info("GLUCOSE", format!("Selected glycemic stats window: {}", win.label()));
                self.update(|s| s.stats_window = win);
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
                self.update(|s| {
                    if s.medication_store
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
                        let _ = save_default_medications(&s.medication_store);
                        log_info("MEDS", format!("Saved medication '{}' ({})", name.trim(), schedule.label()));
                        changed = true;
                    }
                });
                if changed {
                    self.spawn_ha_medications_push(None);
                }
                false
            }
            Command::RemoveMedication(id) => {
                let mut changed = false;
                self.update(|s| {
                    if s.medication_store.remove_medication(&id) {
                        let _ = save_default_medications(&s.medication_store);
                        log_info("MEDS", "Removed scheduled medication.");
                        changed = true;
                    }
                });
                if changed {
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
                self.update(|s| {
                    let g = (s.snapshot.value_mg_dl > 0.0).then_some(s.snapshot.value_mg_dl);
                    let cd = (!custom_dosage.trim().is_empty()).then_some(custom_dosage.as_str());
                    if let Ok(entry) = s.medication_store.log_dose(&medication_id, cd, now, g, &notes) {
                        let _ = save_default_medications(&s.medication_store);
                        log_info(
                            "MEDS",
                            format!(
                                "Logged medication dose: {} ({}) at {}",
                                entry.medication_name, entry.dosage, entry.taken_at_display
                            ),
                        );
                        logged_entry = Some(entry);
                    }
                });
                if logged_entry.is_some() {
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
                self.update(|s| {
                    if let Ok(entry) = s.medication_store.skip_dose(&medication_id, now, &notes) {
                        let _ = save_default_medications(&s.medication_store);
                        log_info(
                            "MEDS",
                            format!(
                                "Skipped medication dose: {} ({}) at {}",
                                entry.medication_name, entry.dosage, entry.taken_at_display
                            ),
                        );
                        skipped_entry = Some(entry);
                    }
                });
                if skipped_entry.is_some() {
                    self.spawn_ha_medications_push(skipped_entry);
                }
                false
            }
            Command::RemoveMedicationDose(dose_id) => {
                let mut changed = false;
                self.update(|s| {
                    if s.medication_store.remove_dose_log(&dose_id) {
                        let _ = save_default_medications(&s.medication_store);
                        log_info("MEDS", "Removed logged medication dose entry.");
                        changed = true;
                    }
                });
                if changed {
                    self.spawn_ha_medications_push(None);
                }
                false
            }
        }
    }

    fn spawn_install_update(&self) {
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
                            info.message = format!("Installer launched ({latest_version}) — upgrading...");
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

    fn spawn_update_check(&self, manual: bool) {
        {
            let mut s = lock(&self.shared);
            if s.checking_updates {
                return;
            }
            s.checking_updates = true;
        }
        self.notifier.state_changed();
        if manual {
            log_info("SYSTEM", "Checking GitHub Releases for application updates...");
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
                            format!("Update available: {latest} (current {current}). Release: {url}"),
                        );
                        if should_notify {
                            alerts::show_notification(
                                "LibreLinkUp Update Available",
                                &format!("Version {latest} is available on GitHub Releases (you have {current})."),
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
                            release_url: librelinkup_core::updater::GITHUB_RELEASES_PAGE_URL.to_string(),
                            download_url: librelinkup_core::updater::GITHUB_RELEASES_PAGE_URL.to_string(),
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

    async fn login(&mut self, username: String, password: String, country: String) -> bool {
        let username = username.trim().to_string();
        if username.len() < 2 || password.len() < 2 {
            self.update(|s| s.error = "Please enter both your LibreLinkUp email and password.".into());
            return false;
        }
        let country = if country.trim().is_empty() {
            "us".to_string()
        } else {
            normalize_country_code(&country)
        };
        log_info("AUTH", format!("Attempting LibreLinkUp login (region: {country})"));
        self.update(|s| {
            s.busy = true;
            s.error.clear();
            s.status = "Signing in to LibreLinkUp...".into();
        });
        let result = authenticate_async(&country, &username, &password).await;
        match result {
            Ok(auth) => {
                let remember_pw = lock(&self.shared).settings.remember_password;
                let existing_ha_token = self
                    .creds
                    .as_ref()
                    .map(|c| c.ha_token.clone())
                    .unwrap_or_default();
                let creds = StoredCredentials {
                    username: username.clone(),
                    password,
                    token: auth.token,
                    account_id: auth.account_id,
                    country: auth.country.clone(),
                    language: "en".into(),
                    patient_id: String::new(),
                    first_name: auth.first_name,
                    last_name: auth.last_name,
                    ha_token: existing_ha_token,
                }
                .sanitized_for_storage(remember_pw);
                let has_ha_tok = creds.has_valid_ha_session();
                self.save_vault(&creds);
                self.creds = Some(creds);
                log_info("AUTH", format!("Login succeeded (region '{}').", auth.country));
                let country_for_settings = auth.country.clone();
                self.update(|s| {
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

    fn save_ha_config(
        &mut self,
        data_source: String,
        ha_url: String,
        ha_token: String,
        ha_sensor_entity_id: String,
    ) -> bool {
        let clean_source = if data_source.trim().eq_ignore_ascii_case("home_assistant") {
            "home_assistant".to_string()
        } else {
            "librelinkup".to_string()
        };
        let clean_url = ha_url.trim().trim_end_matches('/').to_string();
        let clean_entity = ha_sensor_entity_id.trim().to_string();
        let clean_token = ha_token.trim().to_string();

        if clean_source == "home_assistant" && !clean_url.is_empty() {
            if let Err(e) = validate_ha_base_url(&clean_url) {
                self.update(|s| s.error = e.to_string());
                return false;
            }
        }

        let mut creds = self.creds.clone().unwrap_or_default();
        if !clean_token.is_empty() {
            creds.ha_token = clean_token;
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

        let settings = {
            let mut s = lock(&self.shared);
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
            s.settings.clone()
        };
        let _ = save_default_settings(&settings);
        self.notifier.state_changed();
        valid_for_source
    }

    fn auto_fill_ha_env(&mut self) -> bool {
        let Some((env_url, env_token)) = try_load_host_ha_env() else {
            self.update(|s| {
                s.error =
                    "Could not find HA_URL and HA_TOKEN in ~/.config/home-assistant.env".into();
            });
            return false;
        };
        let entity = lock(&self.shared).settings.ha_sensor_entity_id.clone();
        log_info(
            "SYSTEM",
            format!("Imported Home Assistant URL ({env_url}) and token from ~/.config/home-assistant.env"),
        );
        self.save_ha_config("home_assistant".into(), env_url, env_token, entity)
    }

    fn spawn_ha_medications_push(&self, last_logged_dose: Option<MedicationDoseLog>) {
        let Some(creds) = self.creds.clone() else { return };
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
                log_info("MEDS", "Synced medication schedule & dose log to Home Assistant");
            }
        });
    }

    fn spawn_ha_snooze_push(&self, snooze_until_epoch: i64, updated_at_epoch: i64) {
        let Some(creds) = self.creds.clone() else { return };
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
                    format!("Synced alert snooze state to Home Assistant (until={snooze_until_epoch})"),
                );
            }
        });
    }

    async fn refresh(&mut self) {
        let Some(creds) = self.creds.clone() else { return };
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

        match fetch_snapshot_for_source_async(&data_source, &ha_url, &ha_entity_id, &creds).await {
            Ok(mut outcome) => {
                if let Some(updated_creds) = outcome.updated_credentials.take() {
                    self.save_vault(&updated_creds);
                    self.creds = Some(updated_creds);
                }
                if outcome.ha_states.is_none()
                    && (sync_meds || sync_snooze)
                    && !ha_url.trim().is_empty()
                    && creds.has_valid_ha_session()
                {
                    outcome.ha_states =
                        fetch_ha_states_array_async(&ha_url, &creds.ha_token).await.ok();
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
                self.update(|s| {
                    s.busy = false;
                    s.error.clear();
                    s.demo = false;
                    s.last_successful_fetch_epoch = Some(now);
                    if s.glucose_history.ingest_snapshot(&snap, now)
                        && now - s.last_history_persist_epoch
                            >= librelinkup_core::analytics::HISTORY_PERSIST_MIN_INTERVAL_SECS
                    {
                        match save_default_glucose_history(&s.glucose_history) {
                            Ok(()) => s.last_history_persist_epoch = now,
                            Err(e) => log_error(
                                "CRYPTO",
                                format!("Failed to persist glucose history vault: {e}"),
                            ),
                        }
                    }
                    if let Some(ref states) = ha_states {
                        s.discovered_ha_patients = discover_ha_patients(states);
                        if s.settings.ha_sync_medications {
                            if s.medication_store.merge_from_ha_states(states) {
                                let _ = save_default_medications(&s.medication_store);
                                log_info(
                                    "MEDS",
                                    "Merged remote medication schedule/dose updates from Home Assistant",
                                );
                            }
                            push_meds = true;
                        }
                        if s.settings.ha_sync_snooze {
                            if let Some((remote_until, remote_updated_at)) =
                                parse_ha_remote_snooze(states, now)
                            {
                                if remote_updated_at > s.settings.snooze_updated_at_epoch
                                    && remote_until != s.settings.snooze_until_epoch
                                {
                                    s.settings.snooze_until_epoch =
                                        if remote_until > now { remote_until } else { 0 };
                                    s.settings.snooze_updated_at_epoch = remote_updated_at;
                                    if s.settings.snooze_until_epoch == 0 {
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
                        }
                    }
                    s.snapshot = snap;
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

    /// Evaluates alert thresholds using `librelinkup_core::session::evaluate_alert_decision`.
    fn evaluate_alerts(&mut self) {
        let now = now_epoch();
        let reminders = {
            let s = lock(&self.shared);
            s.medication_store
                .evaluate_due_reminders(now, &mut self.medication_last_notified)
        };
        for r in reminders {
            log_info("MEDS", format!("{}: {}", r.title, r.body));
            alerts::show_notification(&r.title, &r.body);
        }

        let (settings, snap, last, logged_in, last_fetch) = {
            let s = lock(&self.shared);
            (
                s.settings.clone(),
                s.snapshot.clone(),
                s.last_alert,
                s.logged_in,
                s.last_successful_fetch_epoch,
            )
        };
        if !logged_in {
            return;
        }
        if snap.is_sensor_expiring_soon(now, settings.sensor_expiry_alert_hours)
            && now - self.sensor_expiry_last_notified_epoch >= 6 * 3600
        {
            self.sensor_expiry_last_notified_epoch = now;
            let body = format!(
                "CGM Sensor {} — prepare a replacement sensor.",
                snap.format_sensor_status(now)
            );
            log_warn("ALERT", format!("CGM Sensor Expiring Soon: {body}"));
            alerts::show_notification("CGM Sensor Expiring Soon", &body);
        }
        let (decision, clear_last) =
            evaluate_alert_decision(&settings, &snap, last, now, last_fetch);
        if clear_last {
            log_info("ALERT", "Glucose back within alert thresholds.");
            lock(&self.shared).last_alert = None;
            return;
        }
        let Some(d) = decision else {
            return;
        };
        log_warn("ALERT", format!("{}: {}", d.title, d.body));
        self.fire(&settings, d.title, &d.body, d.critical);
        lock(&self.shared).last_alert = Some((d.kind, now));
        self.notifier.state_changed();
    }

    fn fire(&self, settings: &AppSettings, title: &str, body: &str, critical: bool) {
        if settings.flash_window_enabled || settings.bring_to_front_enabled {
            self.notifier.attention(settings.flash_window_enabled, settings.bring_to_front_enabled);
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
            let _ = tx.send(Command::Snooze(minutes));
        });
    }
}
