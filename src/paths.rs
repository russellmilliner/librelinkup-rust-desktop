use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

pub const APP_ID: &str = "io.github.russellmilliner.LibreLinkUpDesktop";
pub const APP_DIR_NAME: &str = "librelinkup-kde";

/// Files migrated (copied, never moved) from the legacy `~/.config/librelinkup-kde`
/// directory into the XDG config directory on first run.
const LEGACY_CONFIG_FILES: &[&str] = &["settings.json", "credentials.enc.json", "master.key"];

fn home_dir() -> PathBuf {
    env::var("HOME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Resolves an XDG base directory per the XDG Base Directory spec: the env value is
/// used only when it is a non-empty absolute path, otherwise `$HOME/<fallback_rel>`.
pub fn resolve_xdg_base(xdg_value: Option<&str>, home: &Path, fallback_rel: &str) -> PathBuf {
    xdg_value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(fallback_rel))
}

fn custom_dir_override(var: &str) -> Option<PathBuf> {
    let custom = env::var(var).ok()?;
    let trimmed = custom.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// Legacy location used by v0.1.x/v0.2.0, shared between host and Flatpak builds.
pub fn legacy_config_dir() -> PathBuf {
    home_dir().join(".config").join(APP_DIR_NAME)
}

/// Copies legacy files into `dest_dir` when the destination file does not exist yet.
/// Returns the list of migrated file names. Never overwrites and never deletes the source.
pub fn migrate_legacy_files(legacy_dir: &Path, dest_dir: &Path, files: &[&str]) -> Vec<String> {
    let mut migrated = Vec::new();
    if legacy_dir == dest_dir || !legacy_dir.is_dir() {
        return migrated;
    }
    for name in files {
        let src = legacy_dir.join(name);
        let dst = dest_dir.join(name);
        if !src.is_file() || dst.exists() {
            continue;
        }
        if fs::copy(&src, &dst).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&dst, fs::Permissions::from_mode(0o600));
            }
            migrated.push((*name).to_string());
        }
    }
    migrated
}

pub fn config_dir() -> PathBuf {
    if let Some(p) = custom_dir_override("LIBRELINKUP_CONFIG_DIR") {
        let _ = fs::create_dir_all(&p);
        return p;
    }
    // XDG_CONFIG_HOME is ~/.config on the host and ~/.var/app/<APP_ID>/config inside
    // Flatpak, so the sandboxed build persists data without any host filesystem access.
    let base = resolve_xdg_base(
        env::var("XDG_CONFIG_HOME").ok().as_deref(),
        &home_dir(),
        ".config",
    );
    let dir = base.join(APP_DIR_NAME);
    let _ = fs::create_dir_all(&dir);

    static MIGRATE_ONCE: Once = Once::new();
    MIGRATE_ONCE.call_once(|| {
        let migrated = migrate_legacy_files(&legacy_config_dir(), &dir, LEGACY_CONFIG_FILES);
        if !migrated.is_empty() {
            eprintln!(
                "librelinkup-kde: migrated {} from {} to {}",
                migrated.join(", "),
                legacy_config_dir().display(),
                dir.display()
            );
        }
    });
    dir
}

pub fn data_dir() -> PathBuf {
    if let Some(p) = custom_dir_override("LIBRELINKUP_DATA_DIR") {
        let _ = fs::create_dir_all(&p);
        return p;
    }
    let base = resolve_xdg_base(
        env::var("XDG_DATA_HOME").ok().as_deref(),
        &home_dir(),
        ".local/share",
    );
    let dir = base.join(APP_DIR_NAME);
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

/// Generated default alarm sound (see `alert_sound`).
pub fn default_alert_sound_path() -> PathBuf {
    data_dir().join("default-alert.wav")
}

/// Host autostart directory. Intentionally NOT XDG_CONFIG_HOME: inside Flatpak that points
/// at the sandbox, but autostart entries must live in the host's ~/.config/autostart.
pub fn autostart_desktop_path() -> PathBuf {
    let autostart_dir = home_dir().join(".config/autostart");
    let _ = fs::create_dir_all(&autostart_dir);
    autostart_dir.join(format!("{APP_ID}.desktop"))
}

pub fn legacy_flatpak_leveldb_dir() -> PathBuf {
    home_dir()
        .join(".var/app/rocks.poopjournal.librelinkupdesktop/config/librelinkupdesktop/Local Storage/leveldb")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_base_uses_absolute_env_value() {
        let home = Path::new("/home/u");
        assert_eq!(
            resolve_xdg_base(Some("/home/u/.var/app/x/config"), home, ".config"),
            PathBuf::from("/home/u/.var/app/x/config")
        );
    }

    #[test]
    fn xdg_base_falls_back_for_unset_empty_or_relative() {
        let home = Path::new("/home/u");
        let expected = PathBuf::from("/home/u/.config");
        assert_eq!(resolve_xdg_base(None, home, ".config"), expected);
        assert_eq!(resolve_xdg_base(Some("  "), home, ".config"), expected);
        assert_eq!(resolve_xdg_base(Some("relative/dir"), home, ".config"), expected);
    }

    #[test]
    fn legacy_migration_copies_without_overwriting() {
        let root = env::temp_dir().join(format!("llu-migrate-{}", std::process::id()));
        let legacy = root.join("legacy");
        let dest = root.join("dest");
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&dest).unwrap();
        fs::write(legacy.join("master.key"), "legacy-key").unwrap();
        fs::write(legacy.join("settings.json"), "legacy-settings").unwrap();
        fs::write(dest.join("settings.json"), "new-settings").unwrap();

        let migrated =
            migrate_legacy_files(&legacy, &dest, &["settings.json", "master.key", "missing"]);
        assert_eq!(migrated, vec!["master.key".to_string()]);
        assert_eq!(fs::read_to_string(dest.join("master.key")).unwrap(), "legacy-key");
        assert_eq!(fs::read_to_string(dest.join("settings.json")).unwrap(), "new-settings");
        assert!(legacy.join("master.key").exists(), "source must never be removed");

        // Same directory is a no-op.
        assert!(migrate_legacy_files(&dest, &dest, &["master.key"]).is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}
