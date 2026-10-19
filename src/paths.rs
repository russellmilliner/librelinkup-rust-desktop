use std::env;
use std::fs;
use std::path::PathBuf;

pub const APP_ID: &str = "io.github.russellmilliner.LibreLinkUpDesktop";
pub const APP_DIR_NAME: &str = "librelinkup-kde";

fn home_dir() -> PathBuf {
    env::var("HOME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn config_dir() -> PathBuf {
    if let Ok(custom) = env::var("LIBRELINKUP_CONFIG_DIR") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            let _ = fs::create_dir_all(&p);
            return p;
        }
    }
    // Always use ~/.config/librelinkup-kde so host and Flatpak (--filesystem=xdg-config/librelinkup-kde:create)
    // share the same encrypted credential vault and settings.
    let dir = home_dir().join(".config").join(APP_DIR_NAME);
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn data_dir() -> PathBuf {
    if let Ok(custom) = env::var("LIBRELINKUP_DATA_DIR") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            let _ = fs::create_dir_all(&p);
            return p;
        }
    }
    let dir = home_dir().join(".local/share").join(APP_DIR_NAME);
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn runtime_dir() -> PathBuf {
    let base = env::var("XDG_RUNTIME_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    let dir = base.join(APP_DIR_NAME);
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn settings_file_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn credentials_file_path() -> PathBuf {
    config_dir().join("credentials.enc.json")
}

pub fn master_key_file_path() -> PathBuf {
    config_dir().join("master.key")
}

pub fn default_log_file_path() -> PathBuf {
    data_dir().join("librelinkup-kde.log")
}

pub fn autostart_desktop_path() -> PathBuf {
    let autostart_dir = home_dir().join(".config/autostart");
    let _ = fs::create_dir_all(&autostart_dir);
    autostart_dir.join(format!("{APP_ID}.desktop"))
}

pub fn legacy_flatpak_leveldb_dir() -> PathBuf {
    home_dir()
        .join(".var/app/rocks.poopjournal.librelinkupdesktop/config/librelinkupdesktop/Local Storage/leveldb")
}
