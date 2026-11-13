use cxx_qt_lib::QString;
use librelinkup_kde::bridge::qobject_bridge::qobject::{
    librelinkup_run_app, librelinkup_run_ui_overflow_tests, librelinkup_try_dispatch_cli_action,
};

fn resolve_cli_action(args: &[String]) -> Option<String> {
    let mut i = 1usize;
    while i < args.len() {
        let a = args[i].as_str();
        if let Some(val) = a.strip_prefix("--action=") {
            if !val.trim().is_empty() {
                return Some(val.trim().to_string());
            }
        } else if a == "--action" && i + 1 < args.len() {
            let val = args[i + 1].trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
        let mapped = match a {
            "--toggle-window" => Some("ToggleMainWindow"),
            "--show-window" => Some("ShowMainWindow"),
            "--hide-window" => Some("MinimizeToTray"),
            "--toggle-overlay" => Some("ToggleOverlay"),
            "--toggle-overlay-all-desktops" => Some("ToggleOverlayAllDesktops"),
            "--refresh" => Some("RefreshGlucoseNow"),
            "--notify-reading" => Some("ShowPopupSummary"),
            "--medications" => Some("OpenMedicationsTab"),
            "--mark-due-meds-taken" => Some("MarkDueMedicationsTaken"),
            "--snooze-30" => Some("SnoozeAlerts30m"),
            "--snooze-60" => Some("SnoozeAlerts60m"),
            "--snooze-120" => Some("SnoozeAlerts120m"),
            "--cancel-snooze" => Some("CancelAlertSnooze"),
            "--toggle-high-contrast" => Some("ToggleHighContrast"),
            "--logs" => Some("OpenLogViewer"),
            _ => None,
        };
        if let Some(action_id) = mapped {
            return Some(action_id.to_string());
        }
        i += 1;
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!(
            "LibreLinkUp Desktop (KDE Plasma 6) v{}",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "LibreLinkUp Desktop — KDE Plasma 6 Continuous Glucose Monitor\n\n\
             Usage: librelinkup-kde [OPTIONS]\n\n\
             Options:\n  \
               -m, --minimized, --start-minimized, --autostart   Start hidden in the system tray\n  \
               -u, --check-update                                Check GitHub Releases for updates and exit\n  \
               --ui-test                                         Run headless QtQuick UI/a11y/overflow test suite and exit\n  \
               -V, --version                                     Print version information\n  \
               -h, --help                                        Print this help message\n\n\
             Global Shortcut / Desktop Actions (forward to running instance via D-Bus or run at startup):\n  \
               --toggle-window                                   Toggle main window visibility\n  \
               --show-window                                     Show main window\n  \
               --hide-window                                     Minimize main window to system tray\n  \
               --toggle-overlay                                  Toggle floating desktop overlay widget\n  \
               --toggle-overlay-all-desktops                     Toggle overlay visibility across all virtual desktops\n  \
               --refresh                                         Refresh blood glucose reading now\n  \
               --notify-reading                                  Show desktop notification with current glucose reading\n  \
               --medications                                     Open Medications tab\n  \
               --mark-due-meds-taken                             Mark due scheduled medications as taken\n  \
               --snooze-30 | --snooze-60 | --snooze-120          Snooze non-urgent alerts for 30 / 60 / 120 minutes\n  \
               --cancel-snooze                                   Cancel active alert snooze\n  \
               --toggle-high-contrast                            Toggle WCAG AAA high contrast mode\n  \
               --logs                                            Open diagnostic log viewer\n  \
               --action <ACTION_ID>                              Invoke a desktop/global shortcut action by ID"
        );
        return;
    }

    if args.iter().any(|a| a == "--check-update" || a == "-u") {
        match librelinkup_kde::updater::check_for_updates_blocking(env!("CARGO_PKG_VERSION")) {
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

    if args.iter().any(|a| a == "--ui-test") {
        let dir = std::env::temp_dir().join(format!("llu_cli_uitest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("LIBRELINKUP_CONFIG_DIR", &dir);
        std::env::set_var("LIBRELINKUP_DATA_DIR", &dir);
        let report = librelinkup_run_ui_overflow_tests().to_string();
        let _ = std::fs::remove_dir_all(&dir);
        if report == "OK" {
            println!("UI_TEST_OK");
            return;
        }
        eprintln!("{report}");
        std::process::exit(1);
    }

    if let Some(action_id) = resolve_cli_action(&args) {
        if librelinkup_try_dispatch_cli_action(&QString::from(action_id.as_str())) {
            return;
        }
        std::env::set_var("LIBRELINKUP_INITIAL_ACTION", &action_id);
    }

    let start_minimized = args.iter().any(|a| {
        a == "--minimized"
            || a == "-m"
            || a == "--start-minimized"
            || a == "--autostart"
            || a == "--hidden"
    }) || std::env::var("START_MINIMIZED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let exit_code = librelinkup_run_app(start_minimized);
    std::process::exit(exit_code);
}
