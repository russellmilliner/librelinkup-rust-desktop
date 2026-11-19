use cxx_qt_build::{CxxQtBuilder, QmlModule};
use std::path::PathBuf;

fn collect_kf6_includes() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let pkgs = [
        "KF6ColorScheme",
        "KF6ConfigCore",
        "KF6IconThemes",
        "KF6StatusNotifierItem",
    ];
    for pkg in pkgs {
        if let Ok(out) = std::process::Command::new("pkg-config")
            .args(["--cflags-only-I", pkg])
            .output()
            && out.status.success()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            for token in text.split_whitespace() {
                if let Some(path) = token.strip_prefix("-I") {
                    let pb = PathBuf::from(path);
                    if !dirs.contains(&pb) {
                        dirs.push(pb);
                    }
                }
            }
        }
    }
    let fallbacks = [
        "/usr/include/KF6/KColorScheme",
        "/usr/include/KF6/KConfig",
        "/usr/include/KF6/KConfigCore",
        "/usr/include/KF6/KIconThemes",
        "/usr/include/KF6/KStatusNotifierItem",
    ];
    for fb in fallbacks {
        let pb = PathBuf::from(fb);
        if pb.exists() && !dirs.contains(&pb) {
            dirs.push(pb);
        }
    }
    dirs
}

fn main() {
    let ui_tests_enabled = std::env::var_os("CARGO_FEATURE_UI_TESTS").is_some();

    CxxQtBuilder::new()
        .qt_module("Network")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qt_module("Widgets")
        .qt_module("Svg")
        .qt_module("DBus")
        .qobject_header("src/cpp/app_helper.h")
        .cc_builder(move |cc| {
            cc.include("src/cpp");
            for dir in collect_kf6_includes() {
                cc.include(dir);
            }
            cc.file("src/cpp/icon_renderer.cpp");
            cc.file("src/cpp/window_manager.cpp");
            cc.file("src/cpp/tray_manager.cpp");
            cc.file("src/cpp/audio_portal.cpp");
            if ui_tests_enabled {
                cc.file("src/cpp/ui_tests.cpp");
            } else {
                cc.file("src/cpp/ui_tests_stub.cpp");
            }
        })
        .qml_module(QmlModule {
            uri: "io.github.librelinkup",
            version_major: 1,
            version_minor: 0,
            // Only files that contain a `#[cxx_qt::bridge]` module. The controller
            // implementations live in telemetry.rs / settings.rs / medications.rs and are
            // ordinary Rust `impl` blocks.
            rust_files: &["src/bridge/qobject_bridge.rs"],
            qml_files: &[
                "src/qml/Main.qml",
                "src/qml/HighContrastScrollBar.qml",
                "src/qml/GlucoseChart.qml",
                "src/qml/LoginView.qml",
                "src/qml/DashboardView.qml",
                "src/qml/MedicationsView.qml",
                "src/qml/SettingsView.qml",
                "src/qml/AlertsView.qml",
                "src/qml/AccountView.qml",
                "src/qml/LogViewerDialog.qml",
                "src/qml/OverlayWidget.qml",
                "src/qml/AboutDialog.qml",
            ],
            qrc_files: &[
                "src/qml/icons/tray-icon-dark.svg",
                "src/qml/icons/tray-icon-light.svg",
                "src/cpp/resources/kwin-resident.js",
                "data/io.github.russellmilliner.LibreLinkUpDesktop.notifyrc",
            ],
        })
        .build();

    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_UI_TESTS");
    for path in [
        "src/cpp/app_helper.h",
        "src/cpp/icon_renderer.h",
        "src/cpp/icon_renderer.cpp",
        "src/cpp/window_manager.h",
        "src/cpp/window_manager.cpp",
        "src/cpp/tray_manager.h",
        "src/cpp/tray_manager.cpp",
        "src/cpp/audio_portal.h",
        "src/cpp/audio_portal.cpp",
        "src/cpp/ui_tests.h",
        "src/cpp/ui_tests.cpp",
        "src/cpp/ui_tests_stub.cpp",
        "src/cpp/resources/kwin-resident.js",
        "data/io.github.russellmilliner.LibreLinkUpDesktop.notifyrc",
        "src/bridge/macros.rs",
        "src/bridge/qobject_bridge.rs",
        "src/bridge/autostart.rs",
        "src/bridge/state.rs",
        "src/bridge/ops.rs",
        "src/bridge/telemetry.rs",
        "src/bridge/settings.rs",
        "src/bridge/medications.rs",
        "src/qml/Main.qml",
        "src/qml/HighContrastScrollBar.qml",
        "src/qml/GlucoseChart.qml",
        "src/qml/LoginView.qml",
        "src/qml/DashboardView.qml",
        "src/qml/MedicationsView.qml",
        "src/qml/SettingsView.qml",
        "src/qml/AlertsView.qml",
        "src/qml/AccountView.qml",
        "src/qml/LogViewerDialog.qml",
        "src/qml/OverlayWidget.qml",
        "src/qml/AboutDialog.qml",
        "src/qml/icons/tray-icon-dark.svg",
        "src/qml/icons/tray-icon-light.svg",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rustc-link-lib=KF6ColorScheme");
    println!("cargo:rustc-link-lib=KF6ConfigCore");
    println!("cargo:rustc-link-lib=KF6IconThemes");
    println!("cargo:rustc-link-lib=KF6StatusNotifierItem");
}
