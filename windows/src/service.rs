//! Background service: owns all LibreView network I/O, polling, token re-authentication,
//! credential persistence, staleness checks and alert decisions.
//!
//! It runs on its own thread with a Tokio runtime and never depends on the UI being painted,
//! so alerts keep firing while the main window is hidden in the tray.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use librelinkup_core::api::{authenticate_async, normalize_country_code};
use librelinkup_core::crypto::{
    clear_credentials_at_path, load_credentials_with_secret, save_credentials_with_secret,
    StoredCredentials,
};
use librelinkup_core::event_log::{log_error, log_info, log_warn};
use librelinkup_core::models::{
    clamp_poll_interval, format_glucose_value, load_default_settings, save_default_settings,
    trend_arrow_symbol, AlertKind, AppSettings, GlucoseSnapshot,
};
use librelinkup_core::paths::credentials_file_path;
use librelinkup_core::session::{
    build_demo_snapshot, evaluate_alert_decision, fetch_snapshot_with_reauth_async,
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
    pub last_alert: Option<(AlertKind, i64)>,
    pub vault_ok: bool,
    pub checking_updates: bool,
    pub installing_update: bool,
    pub last_notified_update_version: String,
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
    InstallUpdate,
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
                _ = update_check_timer.tick() => {
                    if lock(&self.shared).settings.check_updates_on_startup {
                        self.spawn_update_check(false);
                    }
                }
            }
        }
    }

    fn poll_interval(&self) -> Duration {
        let secs = clamp_poll_interval(lock(&self.shared).settings.poll_interval_seconds);
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
                    true,
                );
                false
            }
            Command::LoadDemo => {
                let snap = build_demo_snapshot(now_epoch());
                self.update(|s| {
                    s.demo = true;
                    s.logged_in = true;
                    s.status = "Sample CGM telemetry loaded".into();
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
                }
                .sanitized_for_storage(remember_pw);
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
        let Some(creds) = self.creds.clone() else { return };
        if !creds.has_valid_session() {
            return;
        }
        self.update(|s| s.busy = true);

        match fetch_snapshot_with_reauth_async(&creds).await {
            Ok(outcome) => {
                if let Some(updated_creds) = outcome.updated_credentials {
                    self.save_vault(&updated_creds);
                    self.creds = Some(updated_creds);
                }
                let snap = outcome.snapshot;
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

    /// Evaluates alert thresholds using `librelinkup_core::session::evaluate_alert_decision`.
    fn evaluate_alerts(&self) {
        let (settings, snap, last, logged_in) = {
            let s = lock(&self.shared);
            (s.settings.clone(), s.snapshot.clone(), s.last_alert, s.logged_in)
        };
        if !logged_in {
            return;
        }
        let now = now_epoch();
        let (decision, clear_last) = evaluate_alert_decision(&settings, &snap, last, now);
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
