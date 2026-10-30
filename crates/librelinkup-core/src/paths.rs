#![cfg_attr(windows, allow(dead_code, unused_imports))]

use std::env;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;

use ring::rand::{SecureRandom, SystemRandom};

pub const APP_ID: &str = "io.github.russellmilliner.LibreLinkUpDesktop";
#[cfg(not(windows))]
pub const APP_DIR_NAME: &str = "librelinkup-kde";
/// Windows: %APPDATA%\LibreLinkUpDesktop (config) and %LOCALAPPDATA%\LibreLinkUpDesktop (data).
#[cfg(windows)]
pub const APP_DIR_NAME: &str = "LibreLinkUpDesktop";

/// Files migrated (copied, never moved) from the legacy `~/.config/librelinkup-kde`
/// directory into the XDG config directory on first run.
const LEGACY_CONFIG_FILES: &[&str] = &["settings.json", "credentials.enc.json", "master.key"];

fn home_dir() -> PathBuf {
    #[cfg(windows)]
    let var = "USERPROFILE";
    #[cfg(not(windows))]
    let var = "HOME";
    env::var(var)
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

/// Generates a random lowercase hex token of `byte_len * 2` characters for private tempfiles.
fn random_hex_suffix(byte_len: usize) -> String {
    let mut buf = vec![0u8; byte_len];
    let rng = SystemRandom::new();
    if rng.fill(&mut buf).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        return format!("{nanos:x}-{}", std::process::id());
    }
    let mut out = String::with_capacity(byte_len * 2);
    for b in buf {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

/// Opens a brand-new, private (`0600` on Unix) temporary file in `dir` using `create_new(true)`.
pub fn create_private_temp_file(
    dir: &Path,
    prefix: &str,
    suffix: &str,
) -> Result<(PathBuf, fs::File), String> {
    fs::create_dir_all(dir)
        .map_err(|e| format!("Failed creating directory {}: {e}", dir.display()))?;
    for _ in 0..8 {
        let rand_hex = random_hex_suffix(12);
        let candidate = dir.join(format!("{prefix}{rand_hex}{suffix}"));
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        match opts.open(&candidate) {
            Ok(file) => return Ok((candidate, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(format!(
                    "Failed creating private temp file in {}: {e}",
                    dir.display()
                ));
            }
        }
    }
    Err(format!(
        "Exhausted retries creating private temp file in {}",
        dir.display()
    ))
}

/// Atomically writes `contents` to `path` with mode `0600` (on Unix) from file creation time.
/// Writes to a sibling `create_new(true)` temporary file, syncs to disk, and renames into place.
pub fn write_file_atomic_0600(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("tmp");
    let prefix = format!(".{file_name}.tmp-");
    let (tmp_path, mut tmp_file) = create_private_temp_file(parent, &prefix, "")?;
    if let Err(e) = tmp_file
        .write_all(contents)
        .and_then(|()| tmp_file.sync_all())
    {
        drop(tmp_file);
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Failed writing {}: {e}", path.display()));
    }
    drop(tmp_file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600));
    }
    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "Failed atomically replacing {}: {e}",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Quotes and escapes a single command argument for an XDG `.desktop` `Exec=` line
/// per the Desktop Entry Specification (escapes `\`, `"`, `` ` ``, `$`, and doubles `%`).
pub fn quote_desktop_exec_arg(arg: &str) -> String {
    let mut out = String::with_capacity(arg.len() + 4);
    out.push('"');
    for ch in arg.chars() {
        match ch {
            '\\' | '"' | '`' | '$' => {
                out.push('\\');
                out.push(ch);
            }
            '%' => {
                out.push_str("%%");
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
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
        let Ok(bytes) = fs::read(&src) else {
            continue;
        };
        if write_file_atomic_0600(&dst, &bytes).is_ok() {
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
    #[cfg(windows)]
    {
        let dir = windows_known_dir("APPDATA", "AppData\\Roaming").join(APP_DIR_NAME);
        let _ = fs::create_dir_all(&dir);
        dir
    }
    #[cfg(not(windows))]
    {
    // XDG_CONFIG_HOME is ~/.config on the host and ~/.var/app/<APP_ID>/config inside
    // Flatpak, so the sandboxed build persists data without any host filesystem access.
    let base = resolve_xdg_base(
        env::var("XDG_CONFIG_HOME").ok().as_deref(),
        &home_dir(),
        ".config",
    );
    let dir = base.join(APP_DIR_NAME);
    let _ = fs::create_dir_all(&dir);
    restrict_dir_permissions(&dir);

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
}

/// Restricts a directory to the current user (`0700`) on Unix. Vault files are `0600`, but
/// the directory must not be listable/enterable by other local users either.
pub fn restrict_dir_permissions(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Windows known folder from its environment variable, falling back to
/// `%USERPROFILE%\<fallback_rel>` (or the temp dir) if unset.
#[cfg(windows)]
fn windows_known_dir(var: &str, fallback_rel: &str) -> PathBuf {
    env::var(var)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var("USERPROFILE")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .map(|p| PathBuf::from(p).join(fallback_rel))
        })
        .unwrap_or_else(env::temp_dir)
}

pub fn data_dir() -> PathBuf {
    if let Some(p) = custom_dir_override("LIBRELINKUP_DATA_DIR") {
        let _ = fs::create_dir_all(&p);
        return p;
    }
    #[cfg(windows)]
    let base = windows_known_dir("LOCALAPPDATA", "AppData\\Local");
    #[cfg(not(windows))]
    let base = resolve_xdg_base(
        env::var("XDG_DATA_HOME").ok().as_deref(),
        &home_dir(),
        ".local/share",
    );
    let dir = base.join(APP_DIR_NAME);
    let _ = fs::create_dir_all(&dir);
    restrict_dir_permissions(&dir);
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
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

/// Persistent 90-day rolling glucose sample history for eA1C / GMI / TIR analytics.
/// Encrypted at rest (see `analytics`): this is health data, not just a cache.
pub fn glucose_history_file_path() -> PathBuf {
    data_dir().join("glucose_history.enc.json")
}

/// Pre-0.5 plaintext glucose history, migrated into the encrypted vault on first run.
pub fn legacy_glucose_history_file_path() -> PathBuf {
    data_dir().join("glucose_history.json")
}

/// Persistent medication schedule definitions and dose history log (encrypted at rest).
pub fn medications_file_path() -> PathBuf {
    config_dir().join("medications.enc.json")
}

/// Pre-0.5 plaintext medication store, migrated into the encrypted vault on first run.
pub fn legacy_medications_file_path() -> PathBuf {
    config_dir().join("medications.json")
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
    #[cfg(unix)] // XDG resolution is Linux-only; "/home/u" is not absolute on Windows
    fn xdg_base_uses_absolute_env_value() {
        let home = Path::new("/home/u");
        assert_eq!(
            resolve_xdg_base(Some("/home/u/.var/app/x/config"), home, ".config"),
            PathBuf::from("/home/u/.var/app/x/config")
        );
    }

    #[test]
    #[cfg(unix)] // XDG resolution is Linux-only; "/home/u" is not absolute on Windows
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

    #[test]
    fn atomic_write_creates_0600_file_and_replaces_existing() {
        let dir = env::temp_dir().join(format!("llu-atomic-{}", std::process::id()));
        let target = dir.join("secret.json");
        write_file_atomic_0600(&target, b"first-payload").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "first-payload");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        write_file_atomic_0600(&target, b"second-payload").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "second-payload");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn quotes_desktop_exec_arguments_per_xdg_spec() {
        assert_eq!(
            quote_desktop_exec_arg("/usr/bin/librelinkup-kde"),
            "\"/usr/bin/librelinkup-kde\""
        );
        assert_eq!(
            quote_desktop_exec_arg("/opt/My Apps/libre \"link\" $HOME `x` %f\\bin"),
            "\"/opt/My Apps/libre \\\"link\\\" \\$HOME \\`x\\` %%f\\\\bin\""
        );
    }
}
