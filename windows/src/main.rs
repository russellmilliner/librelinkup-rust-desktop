#![cfg_attr(windows, windows_subsystem = "windows")]
//! LibreLinkUp Rust Desktop for Windows.

mod alerts;
mod app;
mod platform;
mod service;
mod tray;

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};

use eframe::egui;
use librelinkup_core::event_log::{log_error, log_info};

use crate::app::{build_tray_view, LibreLinkUpApp, UiHandle};
use crate::service::{Command, SharedState, UiNotifier};
use crate::tray::{TrayAction, TrayView};

const ICON_256: &[u8] = include_bytes!("../assets/icon-256.png");

struct Notifier {
    ui: Arc<UiHandle>,
    shared: OnceLock<SharedState>,
    tray: Mutex<Option<Sender<TrayView>>>,
}

impl UiNotifier for Notifier {
    fn state_changed(&self) {
        self.ui.repaint_all();
        if let (Some(shared), Ok(tray)) = (self.shared.get(), self.tray.lock()) {
            if let Some(tray) = tray.as_ref() {
                let view = build_tray_view(&shared.lock().unwrap_or_else(|p| p.into_inner()));
                let _ = tray.send(view);
            }
        }
    }

    fn attention(&self, flash: bool, bring_to_front: bool) {
        if bring_to_front {
            self.ui.show_main_window();
        }
        if flash {
            platform::flash_window(self.ui.hwnd.load(Ordering::SeqCst));
        }
    }
}

fn print_usage() {
    println!(
        "LibreLinkUp Rust Desktop for Windows v{}\n\n\
         Usage: LibreLinkUp.exe [OPTIONS]\n\n\
         Options:\n  \
           -m, --minimized     Start hidden in the system tray\n  \
           -u, --check-update  Check GitHub Releases for updates and exit\n  \
           -V, --version       Print version information\n  \
           -h, --help          Print this help message",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        platform::attach_parent_console();
        println!("LibreLinkUp Rust Desktop for Windows v{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h" || a == "/?") {
        platform::attach_parent_console();
        print_usage();
        return;
    }
    if args.iter().any(|a| a == "--check-update" || a == "-u") {
        platform::attach_parent_console();
        match librelinkup_core::updater::check_for_updates_blocking(env!("CARGO_PKG_VERSION")) {
            Ok(res) => {
                println!("{}", res.message);
                println!("Release page: {}", res.release_url);
                if res.update_available {
                    println!("Download URL: {}", res.download_url);
                }
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.iter().any(|a| a == "--self-test") {
        platform::attach_parent_console();
        std::process::exit(self_test());
    }
    let start_minimized = args
        .iter()
        .any(|a| matches!(a.as_str(), "--minimized" | "-m" | "--start-minimized" | "--autostart" | "--hidden"));

    install_panic_hook();
    let Some(_instance) = platform::acquire_single_instance() else {
        return; // existing instance was brought to the front
    };
    platform::register_notification_identity(ICON_256);

    // Prefer OpenGL (reliable transparent overlay); fall back to wgpu (DirectX 12 / WARP)
    // on machines without a usable OpenGL driver, e.g. some VMs.
    let mut last_error = None;
    for renderer in [eframe::Renderer::Glow, eframe::Renderer::Wgpu] {
        match run(renderer, start_minimized) {
            Ok(()) => {
                last_error = None;
                break;
            }
            Err(e) => {
                log_error("SYSTEM", format!("Renderer {renderer:?} failed: {e}"));
                last_error = Some(e.to_string());
            }
        }
    }
    if let Some(e) = last_error {
        show_fatal_error(&format!("LibreLinkUp could not start a graphics renderer:\n\n{e}"));
    }
    // Background threads (service, tray) end with the process.
    std::process::exit(0);
}

fn run(renderer: eframe::Renderer, start_minimized: bool) -> eframe::Result {
    let icon = eframe::icon_data::from_png_bytes(ICON_256).ok();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(platform::MAIN_WINDOW_TITLE)
        .with_inner_size([960.0, 720.0])
        .with_min_inner_size([640.0, 480.0])
        .with_visible(!start_minimized);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer,
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native(
        "LibreLinkUp Rust Desktop",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            let ui = Arc::new(UiHandle {
                ctx: cc.egui_ctx.clone(),
                hwnd: AtomicIsize::new(window_hwnd(cc)),
                window_visible: AtomicBool::new(!start_minimized),
                quitting: AtomicBool::new(false),
            });
            let notifier = Arc::new(Notifier {
                ui: ui.clone(),
                shared: OnceLock::new(),
                tray: Mutex::new(None),
            });
            let (shared, tx) = service::start(notifier.clone());
            let _ = notifier.shared.set(shared.clone());

            let tray_ui = ui.clone();
            let tray_tx = tx.clone();
            let tray_shared = shared.clone();
            let tray = tray::start(move |action| {
                handle_tray_action(action, &tray_ui, &tray_tx, &tray_shared)
            });
            if let Ok(mut slot) = notifier.tray.lock() {
                *slot = Some(tray);
            }
            notifier.state_changed();
            if std::env::args().any(|a| a == "--demo") {
                let _ = tx.send(Command::LoadDemo);
            }
            log_info("SYSTEM", format!("UI started with {renderer:?} renderer."));
            Ok(Box::new(LibreLinkUpApp::new(shared, tx, ui)))
        }),
    )
}

fn handle_tray_action(
    action: TrayAction,
    ui: &Arc<UiHandle>,
    tx: &tokio::sync::mpsc::UnboundedSender<Command>,
    shared: &SharedState,
) {
    let update_settings = |f: &dyn Fn(&mut librelinkup_core::models::AppSettings)| {
        let settings = {
            let mut s = shared.lock().unwrap_or_else(|p| p.into_inner());
            f(&mut s.settings);
            s.settings.clone()
        };
        let _ = tx.send(Command::SaveSettings(Box::new(settings)));
        ui.repaint_all();
    };
    match action {
        TrayAction::ToggleWindow => ui.toggle_main_window(),
        TrayAction::ShowWindow => ui.show_main_window(),
        TrayAction::Refresh => {
            let _ = tx.send(Command::RefreshNow);
        }
        TrayAction::Snooze(minutes) => {
            let _ = tx.send(Command::Snooze(minutes));
        }
        TrayAction::SetOverlay(visible) => update_settings(&|s| s.overlay_visible = visible),
        TrayAction::SetAutostart(enabled) => update_settings(&|s| s.autostart_enabled = enabled),
        TrayAction::CheckUpdates => {
            let _ = tx.send(Command::CheckUpdates { manual: true });
        }
        TrayAction::Quit => {
            ui.quit();
            // Safety net if the window cannot process the close request.
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(3));
                std::process::exit(0);
            });
        }
    }
}

fn window_hwnd(cc: &eframe::CreationContext<'_>) -> isize {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
        _ => 0,
    }
}

/// Uses Segoe UI (the Windows UI font) with Segoe UI Symbol for arrows, keeping egui's
/// bundled fonts as fallbacks.
fn install_fonts(ctx: &egui::Context) {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let fonts_dir = std::path::Path::new(&windir).join("Fonts");
    let mut fonts = egui::FontDefinitions::default();
    let mut inserted = Vec::new();
    for (name, file) in [("segoe-ui", "segoeui.ttf"), ("segoe-ui-symbol", "seguisym.ttf")] {
        if let Ok(bytes) = std::fs::read(fonts_dir.join(file)) {
            fonts
                .font_data
                .insert(name.to_string(), Arc::new(egui::FontData::from_owned(bytes)));
            inserted.push(name.to_string());
        }
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        for (i, name) in inserted.iter().enumerate() {
            family.insert(i, name.clone());
        }
    }
    ctx.set_fonts(fonts);
}

#[cfg(windows)]
fn show_fatal_error(message: &str) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let wide = |s: &str| -> Vec<u16> { std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect() };
    let text = wide(message);
    let caption = wide(platform::APP_DISPLAY_NAME);
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_OK | MB_ICONERROR) };
}

#[cfg(not(windows))]
fn show_fatal_error(message: &str) {
    eprintln!("{message}");
}

/// Headless checks of the Windows integration (used by CI and Wine smoke tests).
/// Prints one line per check and returns the process exit code (0 = all passed).
fn self_test() -> i32 {
    use librelinkup_core::crypto::{load_credentials_with_secret, save_credentials_with_secret, StoredCredentials};
    let mut failures = 0;
    let mut check = |name: &str, result: Result<String, String>| match result {
        Ok(detail) => println!("PASS {name}: {detail}"),
        Err(e) => {
            failures += 1;
            println!("FAIL {name}: {e}");
        }
    };
    println!("LibreLinkUp Rust Desktop for Windows v{} self-test", env!("CARGO_PKG_VERSION"));

    check("paths", Ok(format!(
        "config={} data={}",
        librelinkup_core::paths::config_dir().display(),
        librelinkup_core::paths::data_dir().display()
    )));

    check("dpapi", (|| {
        let secret = b"llu-self-test-secret";
        let blob = platform::dpapi_protect(secret)?;
        let back = platform::dpapi_unprotect(&blob)?;
        if back == secret && blob != secret { Ok(format!("{} byte blob round-trip", blob.len())) } else { Err("round-trip mismatch".into()) }
    })());

    check("vault", (|| {
        let key = librelinkup_core::crypto::generate_random_key()?;
        let secret = platform::dpapi_unprotect(&platform::dpapi_protect(&key)?)?;
        let path = std::env::temp_dir().join(format!("llu-selftest-{}.json", std::process::id()));
        let creds = StoredCredentials {
            username: "self-test@example.com".into(),
            password: "p\"w\\d".into(),
            token: "tok".into(),
            account_id: "acc".into(),
            country: "us".into(),
            language: "en".into(),
            patient_id: String::new(),
            first_name: String::new(),
            last_name: String::new(),
        };
        save_credentials_with_secret(&creds, &path, &secret)?;
        let loaded = load_credentials_with_secret(&path, &secret)?.ok_or("vault missing")?;
        let _ = std::fs::remove_file(&path);
        if loaded == creds { Ok("AES-256-GCM envelope with DPAPI-protected key".into()) } else { Err("decrypted credentials differ".into()) }
    })());

    check("registry", Ok(format!("autostart registered = {}", platform::is_autostart_registered())));

    check("tray-badge", {
        let font = ab_glyph::FontArc::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).map_err(|e| e.to_string());
        font.and_then(|f| tray::render_badge(&f, "148", [34, 197, 94]).map(|_| "32x32 icon rendered".to_string()).ok_or_else(|| "render failed".to_string()))
    });

    check("alert-sound", {
        let samples = librelinkup_core::alert_sound::generate_alert_samples();
        if samples.len() > 44_100 { Ok(format!("{} samples", samples.len())) } else { Err("too short".into()) }
    });

    check("updater", {
        let sample = r#"{"tag_name":"v9.9.9","html_url":"https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v9.9.9"}"#;
        librelinkup_core::updater::parse_github_release_json(sample, env!("CARGO_PKG_VERSION"))
            .and_then(|res| if res.update_available { Ok(res.message) } else { Err("expected update_available=true".into()) })
    });

    check("region-normalization", (|| {
        use librelinkup_core::api::{normalize_country_code, resolve_base_url};
        let fi = normalize_country_code("FI");
        let gb = normalize_country_code("GB");
        let eu_url = resolve_base_url("FI");
        if fi == "eu" && gb == "eu2" && eu_url == "https://api-eu.libreview.io/llu" {
            Ok(format!("FI -> {fi} ({eu_url}), GB -> {gb}"))
        } else {
            Err(format!("unexpected mapping: FI->{fi} ({eu_url}), GB->{gb}"))
        }
    })());

    check("network", (|| {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
        // Invalid credentials must yield LibreView's "invalid credentials" (proves DNS, TLS and
        // the HTTP client/curl fallback work), never a transport error. Also test "FI" -> api-eu.
        match rt.block_on(librelinkup_core::api::authenticate_async("FI", "llu-self-test@example.invalid", "not-a-real-password")) {
            Err(librelinkup_core::api::ApiError::InvalidCredentials) => Ok("LibreView EU reachable via FI->eu normalization (invalid test credentials rejected as expected)".into()),
            Err(librelinkup_core::api::ApiError::NetworkError(e)) => Err(format!("network error: {e}")),
            other => Ok(format!("LibreView responded: {other:?}")),
        }
    })());

    println!("{}", if failures == 0 { "ALL CHECKS PASSED".to_string() } else { format!("{failures} CHECK(S) FAILED") });
    if failures == 0 { 0 } else { 1 }
}

/// Writes panics to %LOCALAPPDATA%\LibreLinkUpDesktop\crash.log (a GUI app has no console).
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".into());
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let line = format!(
            "{} panic in thread '{thread}' at {location}: {payload}\n{}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            std::backtrace::Backtrace::force_capture()
        );
        let path = librelinkup_core::paths::data_dir().join("crash.log");
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            use std::io::Write;
            let _ = f.write_all(line.as_bytes());
        }
        log_error("SYSTEM", format!("Panic at {location}: {payload}"));
        default_hook(info);
    }));
}
