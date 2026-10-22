//! Background service: owns all LibreView network I/O, polling, token re-authentication,
//! credential persistence, staleness checks and alert decisions.
//!
//! It runs on its own thread with a Tokio runtime and never depends on the UI being painted,
//! so alerts keep firing while the main window is hidden in the tray.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use librelinkup_core::api::{
    authenticate_async, fetch_cgm_snapshot_async, normalize_country_code, ApiError,
};
use librelinkup_core::crypto::{
    clear_credentials_at_path, load_credentials_with_secret, save_credentials_with_secret,
    StoredCredentials,
};
use librelinkup_core::event_log::{log_error, log_info, log_warn};
use librelinkup_core::models::{
    format_glucose_value, format_reading_age, load_default_settings, reading_age_minutes,
    save_default_settings, trend_arrow_symbol, AlertKind, AppSettings, GlucoseSnapshot, GraphPoint,
};
use librelinkup_core::paths::credentials_file_path;
use librelinkup_core::updater::{check_for_updates_async, UpdateCheckResult};
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
    pub last_alert: Option<(AlertKind, i64)>,
    pub vault_ok: bool,
    pub checking_updates: bool,
    pub update_info: Option<UpdateCheckResult>,
}

pub type SharedState = Arc<Mutex<Shared>>;

#[derive(Debug)]
pub enum Command {
    Login { username: String, password: String, country: String },
    Logout,
    RefreshNow,
    /// Persist new settings (and re-evaluate alerts / tray).
    SaveSettings(Box<AppSettings>),
    /// Snooze non-urgent alerts for N minutes (<= 0 cancels).
    Snooze(i32),
    TestAlert,
    LoadDemo,
    CheckUpdates { manual: bool },
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
        status: "Ready".into(),
        ..Default::default()
    }));
    let (tx, rx) = unbounded_channel();
    let thread_shared = shared.clone();
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
            runtime.block_on(Service { shared: thread_shared, notifier, creds: None }.run(rx));
        })
        .expect("spawn service thread");
    (shared, tx)
}

struct Service {
    shared: SharedState,
    notifier: Arc<dyn UiNotifier>,
    creds: Option<StoredCredentials>,
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
        loop {
            let polling = self.creds.as_ref().is_some_and(|c| c.has_valid_session());
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
            }
        }
    }

    fn poll_interval(&self) -> Duration {
        let secs = lock(&self.shared).settings.poll_interval_seconds.clamp(15, 3600);
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
        match load_credentials_with_secret(&credentials_file_path(), &secret) {
            Ok(Some(mut creds)) if creds.has_valid_session() => {
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
                    format!("Loaded DPAPI-protected credentials (region '{}').", creds.country),
                );
                let username = creds.username.clone();
                let active_country = creds.country.clone();
                self.creds = Some(creds);
                self.update(|s| {
                    s.logged_in = true;
                    s.vault_ok = true;
                    s.username = username;
                    s.settings.country = active_country;
                    s.status = "Signed in — fetching glucose data...".into();
                });
                let settings = lock(&self.shared).settings.clone();
                let _ = save_default_settings(&settings);
            }
            Ok(_) => {
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
        let result = platform::load_or_create_master_secret()
            .and_then(|secret| save_credentials_with_secret(creds, &credentials_file_path(), &secret));
        match result {
            Ok(()) => log_info("CRYPTO", "Saved credentials (AES-256-GCM, key protected by Windows DPAPI)."),
            Err(e) => log_error("CRYPTO", format!("Failed to save credentials: {e}")),
        }
    }

    /// Returns true if an immediate refresh should follow.
    async fn handle(&mut self, cmd: Command) -> bool {
        match cmd {
            Command::Login { username, password, country } => {
                self.login(username, password, country).await
            }
            Command::Logout => {
                let _ = clear_credentials_at_path(&credentials_file_path());
                self.creds = None;
                log_info("AUTH", "Signed out; encrypted vault deleted.");
                self.update(|s| {
                    s.logged_in = false;
                    s.demo = false;
                    s.vault_ok = false;
                    s.username.clear();
                    s.snapshot = GlucoseSnapshot::default();
                    s.last_alert = None;
                    s.error.clear();
                    s.status = "Signed out — encrypted vault cleared.".into();
                });
                false
            }
            Command::RefreshNow => true,
            Command::SaveSettings(settings) => {
                let mut settings = *settings;
                settings.country = normalize_country_code(&settings.country);
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
                }
                if country_changed {
                    if let Some(ref creds) = self.creds {
                        self.save_vault(creds);
                    }
                }
                if let Err(e) = save_default_settings(&settings) {
                    log_error("SYSTEM", format!("Failed to save settings: {e}"));
                }
                self.update(|s| s.settings = settings);
                self.evaluate_alerts();
                country_changed
            }
            Command::Snooze(minutes) => {
                let until = if minutes > 0 { now_epoch() + i64::from(minutes.min(1440)) * 60 } else { 0 };
                let settings = {
                    let mut s = lock(&self.shared);
                    s.settings.snooze_until_epoch = until;
                    if until == 0 {
                        s.last_alert = None; // re-arm
                    }
                    s.settings.clone()
                };
                let _ = save_default_settings(&settings);
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
                );
                false
            }
            Command::LoadDemo => {
                let now = chrono::Local::now();
                let points = (0..24)
                    .map(|i| GraphPoint {
                        timestamp: (now - chrono::Duration::minutes(15 * (24 - i)))
                            .format("%m/%d/%Y %-I:%M:%S %p")
                            .to_string(),
                        value_mg_dl: 110.0 + 40.0 * ((i as f64) / 3.0).sin(),
                    })
                    .collect();
                self.update(|s| {
                    s.demo = true;
                    s.logged_in = true;
                    s.status = "Sample CGM telemetry loaded".into();
                    s.snapshot = GlucoseSnapshot {
                        value_mg_dl: 112.0,
                        trend_arrow: 3,
                        timestamp: now.format("%m/%d/%Y %-I:%M:%S %p").to_string(),
                        target_low_mg_dl: 70.0,
                        target_high_mg_dl: 180.0,
                        patient_id: "demo-patient-001".into(),
                        first_name: "Sample".into(),
                        last_name: "Sensor".into(),
                        graph_points: points,
                        measured_at_epoch: Some(now_epoch()),
                    };
                });
                false
            }
            Command::CheckUpdates { manual } => {
                self.spawn_update_check(manual);
                false
            }
        }
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
                    {
                        let mut s = lock(&shared);
                        s.checking_updates = false;
                        s.update_info = Some(res);
                    }
                    notifier.state_changed();
                    if update_available {
                        log_info(
                            "SYSTEM",
                            format!("Update available: {latest} (current {current}). Release: {url}"),
                        );
                        alerts::show_notification(
                            "LibreLinkUp Update Available",
                            &format!("Version {latest} is available on GitHub Releases (you have {current})."),
                        );
                    } else if manual {
                        log_info("SYSTEM", format!("Update check complete: {msg}"));
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
                };
                self.save_vault(&creds);
                self.creds = Some(creds);
                log_info("AUTH", format!("Login succeeded (region '{}').", auth.country));
                let country_for_settings = auth.country.clone();
                self.update(|s| {
                    s.busy = false;
                    s.logged_in = true;
                    s.demo = false;
                    s.vault_ok = true;
                    s.username = username;
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

    async fn refresh(&mut self) {
        let Some(mut creds) = self.creds.clone() else { return };
        if !creds.has_valid_session() {
            return;
        }
        self.update(|s| s.busy = true);

        let mut result = fetch_cgm_snapshot_async(&creds.country, &creds.token, &creds.account_id).await;
        if matches!(result, Err(ApiError::Unauthorized))
            && !creds.username.is_empty()
            && !creds.password.is_empty()
        {
            log_info("AUTH", "Token expired; re-authenticating with stored credentials...");
            if let Ok(auth) = authenticate_async(&creds.country, &creds.username, &creds.password).await {
                creds.token = auth.token;
                creds.account_id = auth.account_id;
                creds.country = auth.country;
                self.save_vault(&creds);
                self.creds = Some(creds.clone());
                result = fetch_cgm_snapshot_async(&creds.country, &creds.token, &creds.account_id).await;
            }
        }

        match result {
            Ok(snap) => {
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
                self.update(|s| {
                    s.busy = false;
                    s.error.clear();
                    s.demo = false;
                    s.snapshot = snap;
                    s.status = "Live CGM telemetry active".into();
                });
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

    /// Same decision logic as the KDE build (see `AppSettings::should_fire_alert`).
    fn evaluate_alerts(&self) {
        let (settings, snap, last, logged_in) = {
            let s = lock(&self.shared);
            (s.settings.clone(), s.snapshot.clone(), s.last_alert, s.logged_in)
        };
        if !logged_in {
            return;
        }
        let now = now_epoch();
        let age = reading_age_minutes(snap.measured_at_epoch, now);
        let stale = snap.value_mg_dl > 0.0 && settings.is_reading_stale(age);
        let Some(kind) =
            settings.classify_alert(snap.value_mg_dl, snap.target_low_mg_dl, snap.target_high_mg_dl, stale)
        else {
            if last.is_some() {
                log_info("ALERT", "Glucose back within alert thresholds.");
                lock(&self.shared).last_alert = None;
            }
            return;
        };
        if !settings.should_fire_alert(kind, last, now) {
            return;
        }
        let unit = settings.result_unit.clone();
        let value = format_glucose_value(snap.value_mg_dl, &unit);
        let arrow = trend_arrow_symbol(snap.trend_arrow);
        let body = match kind {
            AlertKind::Stale => format!(
                "Last reading was {} ({value} {unit}). Check the sensor and phone connection.",
                format_reading_age(age)
            ),
            AlertKind::UrgentLow => format!(
                "{value} {unit} {arrow} — below urgent low ({}).",
                format_glucose_value(settings.effective_urgent_low(), &unit)
            ),
            AlertKind::Low | AlertKind::High => {
                format!("{value} {unit} {arrow} — outside your target range.")
            }
        };
        log_warn("ALERT", format!("{}: {body}", kind.title()));
        self.fire(&settings, kind.title(), &body);
        lock(&self.shared).last_alert = Some((kind, now));
        self.notifier.state_changed();
    }

    fn fire(&self, settings: &AppSettings, title: &str, body: &str) {
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
        alerts::show_notification(title, body);
    }
}
