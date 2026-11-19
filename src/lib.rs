//! KDE Plasma 6 front-end. Platform-independent logic lives in `librelinkup-core`
//! (re-exported here so `crate::api`, `crate::models`, … keep working).

pub use librelinkup_core::{
    alert_sound, analytics, api, crypto, event_log, medications, models, paths, session, updater,
    zeroize,
};

pub mod bridge {
    #[macro_use]
    pub mod macros;
    pub mod autostart;
    pub mod medications;
    pub mod ops;
    pub mod qobject_bridge;
    pub mod settings;
    pub mod state;
    pub mod telemetry;
}
