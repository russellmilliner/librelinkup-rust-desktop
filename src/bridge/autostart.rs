//! Background Tokio runtime, XDG autostart `.desktop` entry management, and desktop alert firing.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use cxx_qt_lib::QString;

use crate::alert_sound::default_alert_sound;
use crate::bridge::qobject_bridge::qobject;
use crate::event_log::{log_error, log_info, log_warn};
use crate::models::AppSettings;
use crate::paths::{
    autostart_desktop_path, quote_desktop_exec_arg, write_file_atomic_0600, APP_ID,
};

/// Single long-lived multi-thread runtime for all LibreView network I/O, so nothing
/// network-related ever blocks the Qt GUI thread (and connections/TLS can be reused).
pub(crate) fn background_runtime() -> Option<&'static tokio::runtime::Runtime> {
    static RUNTIME: OnceLock<Option<tokio::runtime::Runtime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("librelinkup-net")
                .enable_all()
                .build()
                .map_err(|e| log_error("SYSTEM", format!("Failed to start async runtime: {e}")))
                .ok()
        })
        .as_ref()
}

pub(crate) fn now_epoch() -> i64 {
    chrono::Utc::now().timestamp()
}

pub(crate) fn is_flatpak() -> bool {
    Path::new("/.flatpak-info").exists()
}

/// Formats an XDG Desktop Entry `Exec=` line with proper quoting of the executable path.
pub fn format_autostart_desktop_entry(exe_path: &str, start_minimized: bool) -> String {
    let quoted_exe = quote_desktop_exec_arg(exe_path);
    let exec_cmd = if start_minimized {
        format!("{quoted_exe} --minimized")
    } else {
        quoted_exe
    };
    format!(
        "[Desktop Entry]\nType=Application\nName=LibreLinkUp Rust Desktop\nComment=KDE Plasma 6 Blood Glucose Monitor for LibreLinkUp\nExec={exec_cmd}\nIcon={APP_ID}\nTerminal=false\nStartupNotify=false\nX-KDE-autostart-after=panel\n"
    )
}

pub(crate) fn sync_autostart_desktop_file(enabled: bool, start_minimized: bool) {
    if is_flatpak() {
        // Sandboxed: ask the XDG Background portal to manage the host autostart entry.
        if qobject::librelinkup_request_background_autostart(enabled, start_minimized) {
            log_info(
                "SYSTEM",
                format!(
                    "Requested login autostart={enabled} (minimized={start_minimized}) via the Background portal."
                ),
            );
        } else {
            log_warn(
                "SYSTEM",
                "Could not reach the Background portal to update autostart.",
            );
        }
        return;
    }
    let desktop_path = autostart_desktop_path();
    if !enabled {
        if desktop_path.exists() {
            let _ = fs::remove_file(&desktop_path);
            log_info("SYSTEM", "Removed KDE autostart desktop entry.");
        }
        return;
    }

    // Host (non-Flatpak) build: point the autostart entry at the current executable.
    let exe_str = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "librelinkup-kde".to_string());

    let content = format_autostart_desktop_entry(&exe_str, start_minimized);
    if write_file_atomic_0600(&desktop_path, content.as_bytes()).is_ok() {
        log_info(
            "SYSTEM",
            format!("Updated autostart entry at {}", desktop_path.display()),
        );
    }
}

/// Fires all enabled alert channels: window raise/flash, audio, and a desktop notification.
pub(crate) fn fire_alert(settings: &AppSettings, title: &str, body: &str, critical: bool) {
    if settings.bring_to_front_enabled || settings.flash_window_enabled {
        qobject::librelinkup_trigger_window_alert(
            settings.bring_to_front_enabled,
            settings.flash_window_enabled,
        );
    }
    if settings.audio_alert_enabled {
        let custom = settings.custom_sound_path.trim();
        let sound_path =
            if settings.use_custom_sound && !custom.is_empty() && Path::new(custom).is_file() {
                custom.to_string()
            } else {
                if settings.use_custom_sound && !custom.is_empty() {
                    log_warn(
                        "ALERT",
                        format!("Custom alert sound not found ({custom}); using built-in alarm."),
                    );
                }
                default_alert_sound()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
            };
        qobject::librelinkup_play_alert_sound(&QString::from(sound_path.as_str()));
    }
    qobject::librelinkup_show_notification(
        &QString::from(title),
        &QString::from(body),
        critical,
        true,
    );
}
