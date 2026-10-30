use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new()
        .qt_module("Network")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qt_module("Widgets")
        .qt_module("Svg")
        .qt_module("DBus")
        .qobject_header("src/cpp/app_helper.h")
        .cc_builder(|cc| {
            cc.include("src/cpp");
            cc.include("/usr/include/KF6/KColorScheme");
            cc.include("/usr/include/KF6/KConfig");
            cc.include("/usr/include/KF6/KConfigCore");
            cc.include("/usr/include/KF6/KIconThemes");
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
                "src/qml/LogViewerDialog.qml",
                "src/qml/OverlayWidget.qml",
                "src/qml/AboutDialog.qml",
            ],
            qrc_files: &[
                "src/qml/icons/tray-icon-dark.svg",
                "src/qml/icons/tray-icon-light.svg",
            ],
        })
        .build();

    println!("cargo:rerun-if-changed=src/cpp/app_helper.h");
    println!("cargo:rerun-if-changed=src/cpp/icon_renderer.h");
    println!("cargo:rerun-if-changed=src/cpp/window_manager.h");
    println!("cargo:rerun-if-changed=src/cpp/tray_manager.h");
    println!("cargo:rerun-if-changed=src/cpp/audio_portal.h");
    println!("cargo:rerun-if-changed=src/cpp/ui_tests.h");
    println!("cargo:rerun-if-changed=src/bridge/qobject_bridge.rs");
    println!("cargo:rerun-if-changed=src/bridge/autostart.rs");
    println!("cargo:rerun-if-changed=src/bridge/state.rs");
    println!("cargo:rerun-if-changed=src/bridge/ops.rs");
    println!("cargo:rerun-if-changed=src/bridge/telemetry.rs");
    println!("cargo:rerun-if-changed=src/bridge/settings.rs");
    println!("cargo:rerun-if-changed=src/bridge/medications.rs");
    println!("cargo:rerun-if-changed=src/qml/Main.qml");
    println!("cargo:rerun-if-changed=src/qml/LogViewerDialog.qml");
    println!("cargo:rerun-if-changed=src/qml/OverlayWidget.qml");
    println!("cargo:rerun-if-changed=src/qml/AboutDialog.qml");
    println!("cargo:rerun-if-changed=src/qml/icons/tray-icon-dark.svg");
    println!("cargo:rerun-if-changed=src/qml/icons/tray-icon-light.svg");
    println!("cargo:rustc-link-lib=KF6ColorScheme");
    println!("cargo:rustc-link-lib=KF6ConfigCore");
    println!("cargo:rustc-link-lib=KF6IconThemes");
}
