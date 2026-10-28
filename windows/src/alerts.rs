//! Alert outputs: the built-in alarm (or a custom sound file) played through the default
//! audio device, and Windows toast notifications.

use std::fs::File;
use std::num::NonZero;
use std::path::PathBuf;

use librelinkup_core::alert_sound::{generate_alert_samples, SAMPLE_RATE};
use librelinkup_core::event_log::{log_info, log_warn};
use librelinkup_core::models::{
    parse_notification_snooze_action, ALERT_NOTIFICATION_SNOOZE_ACTIONS,
};

use crate::platform::APP_USER_MODEL_ID;

/// Plays the alert on a background thread so callers never block.
pub fn play_alert_sound(custom_sound: Option<PathBuf>) {
    std::thread::Builder::new()
        .name("llu-alert-audio".into())
        .spawn(move || {
            if let Err(e) = play_blocking(custom_sound.clone()) {
                log_warn("ALERT", format!("Alert sound failed: {e}"));
                // A broken custom file must never silence the alarm: retry with the built-in one.
                if custom_sound.is_some() {
                    if let Err(e) = play_blocking(None) {
                        log_warn("ALERT", format!("Built-in alarm failed: {e}"));
                    }
                }
            }
        })
        .ok();
}

fn play_blocking(custom_sound: Option<PathBuf>) -> Result<(), String> {
    let sink = rodio::DeviceSinkBuilder::open_default_sink()
        .map_err(|e| format!("no audio output device: {e}"))?;
    let player = rodio::Player::connect_new(sink.mixer());
    match custom_sound {
        Some(path) => {
            let file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let source = rodio::Decoder::try_from(file)
                .map_err(|e| format!("cannot decode {}: {e}", path.display()))?;
            player.append(source);
        }
        None => {
            let samples: Vec<f32> = generate_alert_samples()
                .iter()
                .map(|s| *s as f32 / 32768.0)
                .collect();
            let channels = NonZero::new(1u16).expect("non-zero");
            let rate = NonZero::new(SAMPLE_RATE).expect("non-zero");
            player.append(rodio::buffer::SamplesBuffer::new(channels, rate, samples));
        }
    }
    player.sleep_until_end();
    Ok(())
}

/// Shows a toast notification (attributed to our AppUserModelID).
pub fn show_notification(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    // WinRT toast APIs can block briefly; keep them off the UI/worker threads.
    std::thread::Builder::new()
        .name("llu-notify".into())
        .spawn(move || {
            let mut notif = notify_rust::Notification::new();
            notif.summary(&title).body(&body);
            #[cfg(target_os = "windows")]
            notif.app_id(APP_USER_MODEL_ID);
            if let Err(e) = notif.show() {
                log_warn("ALERT", format!("Desktop notification failed: {e}"));
            }
        })
        .ok();
}

/// Shows an alert toast notification with interactive snooze buttons
/// ("Snooze 30 min", "Snooze 1 hr", "Snooze 2 hr") and invokes `on_snooze(minutes)`
/// if the user clicks one of the snooze buttons.
pub fn show_alert_notification(
    title: &str,
    body: &str,
    critical: bool,
    on_snooze: impl FnOnce(i32) + Send + 'static,
) {
    let title = title.to_string();
    let body = body.to_string();
    // WinRT toast APIs and action waits block on the notification lifecycle; run on a
    // dedicated background thread so the service and UI threads never block.
    std::thread::Builder::new()
        .name("llu-notify".into())
        .spawn(move || {
            let mut notif = notify_rust::Notification::new();
            notif
                .summary(&title)
                .body(&body)
                .urgency(if critical {
                    notify_rust::Urgency::Critical
                } else {
                    notify_rust::Urgency::Normal
                });
            #[cfg(target_os = "windows")]
            notif.app_id(APP_USER_MODEL_ID);
            for &(action_id, label, _) in ALERT_NOTIFICATION_SNOOZE_ACTIONS {
                notif.action(action_id, label);
            }
            match notif.show() {
                Ok(handle) => {
                    handle.wait_for_action(|action| {
                        if let Some(minutes) = parse_notification_snooze_action(action) {
                            log_info(
                                "ALERT",
                                format!(
                                    "Desktop notification action '{action}' clicked -> snoozing alerts for {minutes} min."
                                ),
                            );
                            on_snooze(minutes);
                        }
                    });
                }
                Err(e) => {
                    log_warn("ALERT", format!("Desktop notification failed: {e}"));
                }
            }
        })
        .ok();
}
