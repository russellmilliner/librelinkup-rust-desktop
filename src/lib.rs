//! KDE Plasma 6 front-end. Platform-independent logic lives in `librelinkup-core`
//! (re-exported here so `crate::api`, `crate::models`, … keep working).

pub use librelinkup_core::{
    alert_sound, api, crypto, event_log, models, paths, session, updater,
};

pub mod bridge {
    pub mod app_controller;
    pub mod autostart;
    pub mod settings_handlers;
}
