use std::fmt;
use std::fs;
use std::num::NonZeroU32;
use std::path::Path;

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::paths::{
    credentials_file_path, legacy_flatpak_leveldb_dir, master_key_file_path, write_file_atomic_0600,
};

const ENVELOPE_VERSION: u32 = 1;
const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const AAD_CONTEXT: &[u8] = b"librelinkup-kde-credentials-v1";

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredCredentials {
    pub username: String,
    pub password: String,
    pub token: String,
    pub account_id: String,
    pub country: String,
    pub language: String,
    #[serde(default)]
    pub patient_id: String,
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub last_name: String,
}

impl fmt::Debug for StoredCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredCredentials")
            .field("username", &"[REDACTED]")
            .field("password", &"[REDACTED]")
            .field("token", &"[REDACTED]")
            .field("account_id", &"[REDACTED]")
            .field("country", &self.country)
            .field("language", &self.language)
            .field("patient_id", &"[REDACTED]")
            .field("first_name", &"[REDACTED]")
            .field("last_name", &"[REDACTED]")
            .finish()
    }
}

impl Drop for StoredCredentials {
    fn drop(&mut self) {
        self.username.zeroize();
        self.password.zeroize();
        self.token.zeroize();
        self.account_id.zeroize();
        self.patient_id.zeroize();
    }
}

impl StoredCredentials {
    pub fn has_valid_session(&self) -> bool {
        !self.token.trim().is_empty() && !self.account_id.trim().is_empty()
    }

    /// Returns a copy prepared for vault storage, stripping `password` unless `remember_password` is `true`.
    pub fn sanitized_for_storage(&self, remember_password: bool) -> Self {
        let mut out = self.clone();
        if !remember_password {
            out.password.zeroize();
            out.password.clear();
        }
        out
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    pub version: u32,
    pub algorithm: String,
    pub kdf: String,
    pub iterations: u32,
    pub salt_hex: String,
    pub nonce_hex: String,
    pub ciphertext_hex: String,
}

impl fmt::Debug for EncryptedEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncryptedEnvelope")
            .field("version", &self.version)
            .field("algorithm", &self.algorithm)
            .field("kdf", &self.kdf)
            .field("iterations", &self.iterations)
            .field("salt_hex", &"[REDACTED]")
            .field("nonce_hex", &"[REDACTED]")
            .field("ciphertext_hex", &"[REDACTED]")
            .finish()
    }
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    let clean = hex.trim();
    if clean.len() % 2 != 0 {
        return Err("Invalid hex length (must be even)".to_string());
    }
    let mut out = Vec::with_capacity(clean.len() / 2);
    let bytes = clean.as_bytes();
    for chunk in bytes.chunks_exact(2) {
        let s = std::str::from_utf8(chunk).map_err(|e| e.to_string())?;
        let val = u8::from_str_radix(s, 16).map_err(|e| format!("Invalid hex byte '{s}': {e}"))?;
        out.push(val);
    }
    Ok(out)
}

/// Returns a new random 256-bit key from the OS CSPRNG.
pub fn generate_random_key() -> Result<[u8; KEY_LEN], String> {
    let mut key = [0u8; KEY_LEN];
    SystemRandom::new()
        .fill(&mut key)
        .map_err(|_| "CSPRNG failure generating key".to_string())?;
    Ok(key)
}

fn derive_aes_key(master_secret: &[u8], salt: &[u8]) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    let iterations = NonZeroU32::new(PBKDF2_ITERATIONS).expect("non-zero iterations");
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        iterations,
        salt,
        master_secret,
        &mut key,
    );
    key
}

pub fn encrypt_credentials_with_secret(
    creds: &StoredCredentials,
    master_secret: &[u8],
) -> Result<EncryptedEnvelope, String> {
    if master_secret.is_empty() {
        return Err("Master secret must not be empty".to_string());
    }
    let rng = SystemRandom::new();
    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rng.fill(&mut salt)
        .map_err(|_| "CSPRNG failure generating salt".to_string())?;
    rng.fill(&mut nonce_bytes)
        .map_err(|_| "CSPRNG failure generating nonce".to_string())?;

    let mut derived_key = derive_aes_key(master_secret, &salt);
    let unbound_key = UnboundKey::new(&AES_256_GCM, &derived_key)
        .map_err(|_| "Failed to initialize AES-256-GCM key".to_string())?;
    derived_key.zeroize();

    let key = LessSafeKey::new(unbound_key);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let mut plaintext = serde_json::to_vec(creds)
        .map_err(|e| format!("Failed to serialize credentials: {e}"))?;

    let mut in_out = plaintext.clone();
    plaintext.zeroize();

    key.seal_in_place_append_tag(nonce, Aad::from(AAD_CONTEXT), &mut in_out)
        .map_err(|_| "AES-256-GCM encryption failed".to_string())?;

    Ok(EncryptedEnvelope {
        version: ENVELOPE_VERSION,
        algorithm: "AES-256-GCM".to_string(),
        kdf: "PBKDF2-HMAC-SHA256".to_string(),
        iterations: PBKDF2_ITERATIONS,
        salt_hex: bytes_to_hex(&salt),
        nonce_hex: bytes_to_hex(&nonce_bytes),
        ciphertext_hex: bytes_to_hex(&in_out),
    })
}

pub fn decrypt_credentials_with_secret(
    envelope: &EncryptedEnvelope,
    master_secret: &[u8],
) -> Result<StoredCredentials, String> {
    if envelope.version != ENVELOPE_VERSION {
        return Err(format!("Unsupported envelope version: {}", envelope.version));
    }
    if envelope.algorithm != "AES-256-GCM" {
        return Err(format!("Unsupported cipher algorithm: {}", envelope.algorithm));
    }
    if master_secret.is_empty() {
        return Err("Master secret must not be empty".to_string());
    }

    let salt = hex_to_bytes(&envelope.salt_hex)?;
    if salt.len() != SALT_LEN {
        return Err("Invalid salt length in encrypted envelope".to_string());
    }
    let nonce_vec = hex_to_bytes(&envelope.nonce_hex)?;
    let nonce_bytes: [u8; NONCE_LEN] = nonce_vec
        .try_into()
        .map_err(|_| "Invalid nonce length in encrypted envelope".to_string())?;
    let mut ciphertext = hex_to_bytes(&envelope.ciphertext_hex)?;
    if ciphertext.len() < AES_256_GCM.tag_len() {
        return Err("Ciphertext is shorter than authentication tag".to_string());
    }

    let mut derived_key = derive_aes_key(master_secret, &salt);
    let unbound_key = UnboundKey::new(&AES_256_GCM, &derived_key)
        .map_err(|_| "Failed to initialize AES-256-GCM key".to_string())?;
    derived_key.zeroize();

    let key = LessSafeKey::new(unbound_key);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let decrypted_slice = key
        .open_in_place(nonce, Aad::from(AAD_CONTEXT), &mut ciphertext)
        .map_err(|_| "Credential authentication/decryption failed (tampered or wrong key)".to_string())?;

    let creds: StoredCredentials = serde_json::from_slice(decrypted_slice)
        .map_err(|e| format!("Failed to parse decrypted credentials JSON: {e}"))?;
    ciphertext.zeroize();

    Ok(creds)
}

#[cfg(unix)]
fn should_use_desktop_keyring(key_path: &Path) -> bool {
    if key_path != master_key_file_path() {
        return false;
    }
    if std::env::var("LIBRELINKUP_CONFIG_DIR").is_ok() {
        return false;
    }
    if std::env::var("LIBRELINKUP_DISABLE_KEYRING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return false;
    }
    if std::env::var("QT_QPA_PLATFORM")
        .map(|v| v.eq_ignore_ascii_case("offscreen"))
        .unwrap_or(false)
    {
        return false;
    }
    std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

/// Attempts to retrieve the 256-bit vault master secret hex string from the desktop
/// keyring on Linux (freedesktop Secret Service via `secret-tool` or KDE KWallet via `kwallet-query`).
#[cfg(unix)]
fn try_keyring_lookup() -> Option<String> {
    use std::process::Command;

    // 1. Freedesktop Secret Service (KWallet6/GNOME Keyring/KeePassXC Secret Service)
    if let Ok(out) = Command::new("secret-tool")
        .args([
            "lookup",
            "application",
            crate::paths::APP_ID,
            "key",
            "vault-master-secret",
        ])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.len() == KEY_LEN * 2 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Some(s);
            }
        }
    }

    // 2. KDE KWallet CLI (`kwallet-query`)
    if let Ok(out) = Command::new("kwallet-query")
        .args(["-r", "vault-master-secret", "-f", "LibreLinkUp", "kdewallet"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.len() == KEY_LEN * 2 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Some(s);
            }
        }
    }

    None
}

/// Attempts to store the 256-bit vault master secret hex string in the desktop keyring
/// on Linux (Secret Service `secret-tool` or KDE KWallet `kwallet-query`), piping the
/// secret over stdin so it never appears in process command-line arguments.
#[cfg(unix)]
fn try_keyring_store(hex_secret: &str) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let try_pipe = |mut cmd: Command| -> bool {
        let Ok(mut child) = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return false;
        };
        if let Some(mut stdin) = child.stdin.take() {
            if stdin.write_all(hex_secret.as_bytes()).is_err() {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
        child.wait().map(|s| s.success()).unwrap_or(false)
    };

    let mut secret_tool = Command::new("secret-tool");
    secret_tool.args([
        "store",
        "--label=LibreLinkUp Rust Desktop Vault Key",
        "application",
        crate::paths::APP_ID,
        "key",
        "vault-master-secret",
    ]);
    if try_pipe(secret_tool) {
        return true;
    }

    let mut kwallet = Command::new("kwallet-query");
    kwallet.args(["-w", "vault-master-secret", "-f", "LibreLinkUp", "kdewallet"]);
    try_pipe(kwallet)
}

pub fn load_or_create_master_secret(key_path: &Path) -> Result<Vec<u8>, String> {
    let machine_id = fs::read_to_string("/etc/machine-id")
        .unwrap_or_else(|_| "librelinkup-kde-fallback-machine-id".to_string());

    #[cfg(unix)]
    let use_keyring = should_use_desktop_keyring(key_path);

    let file_secret = if key_path.exists() {
        let mut raw_hex = fs::read_to_string(key_path)
            .map_err(|e| format!("Failed to read master key file: {e}"))?;
        let trimmed = raw_hex.trim().to_string();
        let bytes = hex_to_bytes(&trimmed)?;
        #[cfg(unix)]
        if use_keyring && try_keyring_store(&trimmed) {
            // Migrated on-disk master.key into Secret Service / KWallet; remove plaintext file.
            let _ = write_file_atomic_0600(key_path, &[0u8; KEY_LEN * 2]);
            let _ = fs::remove_file(key_path);
        }
        raw_hex.zeroize();
        bytes
    } else {
        #[cfg(unix)]
        let keyring_secret = if use_keyring {
            try_keyring_lookup().and_then(|hex| hex_to_bytes(&hex).ok())
        } else {
            None
        };
        #[cfg(not(unix))]
        let keyring_secret: Option<Vec<u8>> = None;

        if let Some(secret) = keyring_secret {
            secret
        } else {
            let mut random_key = generate_random_key()?;
            let mut hex_str = bytes_to_hex(&random_key);
            random_key.zeroize();

            #[cfg(unix)]
            let stored_in_keyring = use_keyring && try_keyring_store(&hex_str);
            #[cfg(not(unix))]
            let stored_in_keyring = false;

            if !stored_in_keyring {
                write_file_atomic_0600(key_path, hex_str.as_bytes())
                    .map_err(|e| format!("Failed to write master key file: {e}"))?;
            }
            let res = hex_to_bytes(&hex_str);
            hex_str.zeroize();
            res?
        }
    };

    let mut combined = Vec::with_capacity(file_secret.len() + machine_id.len() + 32);
    combined.extend_from_slice(&file_secret);
    combined.extend_from_slice(b":librelinkup-kde:");
    combined.extend_from_slice(machine_id.trim().as_bytes());
    Ok(combined)
}

pub fn save_credentials_to_path(
    creds: &StoredCredentials,
    creds_path: &Path,
    key_path: &Path,
) -> Result<(), String> {
    let mut secret = load_or_create_master_secret(key_path)?;
    let envelope = encrypt_credentials_with_secret(creds, &secret)?;
    secret.zeroize();

    let serialized = serde_json::to_string_pretty(&envelope)
        .map_err(|e| format!("Failed to serialize encrypted envelope: {e}"))?;
    write_file_atomic_0600(creds_path, serialized.as_bytes())
        .map_err(|e| format!("Failed to write encrypted credentials file: {e}"))?;
    Ok(())
}

/// Encrypts and saves credentials using a caller-supplied master secret (e.g. a key
/// protected by Windows DPAPI) instead of the file + machine-id derived secret.
pub fn save_credentials_with_secret(
    creds: &StoredCredentials,
    creds_path: &Path,
    master_secret: &[u8],
) -> Result<(), String> {
    let envelope = encrypt_credentials_with_secret(creds, master_secret)?;
    let serialized = serde_json::to_string_pretty(&envelope)
        .map_err(|e| format!("Failed to serialize encrypted envelope: {e}"))?;
    write_file_atomic_0600(creds_path, serialized.as_bytes())
        .map_err(|e| format!("Failed to write encrypted credentials file: {e}"))?;
    Ok(())
}

/// Loads and decrypts credentials using a caller-supplied master secret.
pub fn load_credentials_with_secret(
    creds_path: &Path,
    master_secret: &[u8],
) -> Result<Option<StoredCredentials>, String> {
    if !creds_path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(creds_path)
        .map_err(|e| format!("Failed to read encrypted credentials file: {e}"))?;
    let envelope: EncryptedEnvelope = serde_json::from_str(&raw)
        .map_err(|e| format!("Corrupted encrypted credentials envelope: {e}"))?;
    decrypt_credentials_with_secret(&envelope, master_secret).map(Some)
}

pub fn load_credentials_from_path(
    creds_path: &Path,
    key_path: &Path,
) -> Result<Option<StoredCredentials>, String> {
    if !creds_path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(creds_path)
        .map_err(|e| format!("Failed to read encrypted credentials file: {e}"))?;
    let envelope: EncryptedEnvelope = serde_json::from_str(&raw)
        .map_err(|e| format!("Corrupted encrypted credentials envelope: {e}"))?;
    let mut secret = load_or_create_master_secret(key_path)?;
    let res = decrypt_credentials_with_secret(&envelope, &secret);
    secret.zeroize();
    res.map(Some)
}

pub fn clear_credentials_at_path(creds_path: &Path) -> Result<(), String> {
    if creds_path.exists() {
        fs::remove_file(creds_path)
            .map_err(|e| format!("Failed to remove credentials file: {e}"))?;
    }
    Ok(())
}

pub fn save_default_credentials(creds: &StoredCredentials) -> Result<(), String> {
    save_credentials_to_path(creds, &credentials_file_path(), &master_key_file_path())
}

pub fn load_default_credentials() -> Result<Option<StoredCredentials>, String> {
    let creds_path = credentials_file_path();
    let key_path = master_key_file_path();
    if let Some(c) = load_credentials_from_path(&creds_path, &key_path)? {
        return Ok(Some(c));
    }
    // Check if we can migrate a valid token from the legacy Electron Flatpak LevelDB
    if let Some(migrated) = try_migrate_legacy_leveldb_credentials(&legacy_flatpak_leveldb_dir()) {
        let _ = save_credentials_to_path(&migrated, &creds_path, &key_path);
        return Ok(Some(migrated));
    }
    Ok(None)
}

pub fn clear_default_credentials() -> Result<(), String> {
    clear_credentials_at_path(&credentials_file_path())
}

/// Scans legacy Chromium LevelDB files from `rocks.poopjournal.librelinkupdesktop`
/// to migrate an active token/accountId/country into encrypted storage on first run.
pub fn try_migrate_legacy_leveldb_credentials(leveldb_dir: &Path) -> Option<StoredCredentials> {
    if !leveldb_dir.exists() {
        return None;
    }
    let entries = fs::read_dir(leveldb_dir).ok()?;
    let mut paths: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".ldb") || n.ends_with(".log"))
                .unwrap_or(false)
        })
        .collect();
    paths.sort();

    let mut best: Option<StoredCredentials> = None;

    for path in paths {
        // Skip unreadable files instead of aborting the entire LevelDB scan early.
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let lossy = String::from_utf8_lossy(&bytes);
        for chunk in lossy.split("{\"state\":{") {
            if let Some(end_idx) = chunk.find("},\"version\"") {
                let json_str = format!("{{{}}}", &chunk[..end_idx]);
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let token = val
                        .get("token")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    let account_id = val
                        .get("accountId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    let raw_country = val
                        .get("country")
                        .and_then(|v| v.as_str())
                        .unwrap_or("us")
                        .trim();
                    let country = if raw_country.is_empty() {
                        "us".to_string()
                    } else {
                        crate::api::normalize_country_code(raw_country)
                    };
                    let language = val
                        .get("language")
                        .and_then(|v| v.as_str())
                        .unwrap_or("en")
                        .trim();
                    if !token.is_empty() && !account_id.is_empty() {
                        best = Some(StoredCredentials {
                            username: String::new(),
                            password: String::new(),
                            token: token.to_string(),
                            account_id: account_id.to_string(),
                            country,
                            language: if language.is_empty() {
                                "en".to_string()
                            } else {
                                language.to_string()
                            },
                            patient_id: String::new(),
                            first_name: String::new(),
                            last_name: String::new(),
                        });
                    }
                }
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_credentials_and_envelope_debug_redact_secrets() {
        let creds = StoredCredentials {
            username: "secret.user@example.com".into(),
            password: "MySuperSecretPassword!".into(),
            token: "eyJhbGciOiJIUzI1NiJ9.secret".into(),
            account_id: "acc-uuid-9999".into(),
            country: "us".into(),
            language: "en".into(),
            patient_id: "pat-uuid-8888".into(),
            first_name: "SecretFirst".into(),
            last_name: "SecretLast".into(),
        };
        let dbg = format!("{creds:?}");
        assert!(!dbg.contains("secret.user@example.com"));
        assert!(!dbg.contains("MySuperSecretPassword!"));
        assert!(!dbg.contains("eyJhbGciOiJIUzI1NiJ9.secret"));
        assert!(!dbg.contains("acc-uuid-9999"));
        assert!(!dbg.contains("pat-uuid-8888"));
        assert!(!dbg.contains("SecretFirst"));
        assert!(!dbg.contains("SecretLast"));
        assert!(dbg.contains("[REDACTED]"));

        // Opt-in password storage: sanitized_for_storage(false) strips password
        let stripped = creds.sanitized_for_storage(false);
        assert!(stripped.password.is_empty());
        assert_eq!(stripped.token, creds.token);
        let kept = creds.sanitized_for_storage(true);
        assert_eq!(kept.password, "MySuperSecretPassword!");

        let env = encrypt_credentials_with_secret(&creds, b"unit-test-secret").unwrap();
        let env_dbg = format!("{env:?}");
        assert!(!env_dbg.contains(&env.ciphertext_hex));
        assert!(!env_dbg.contains(&env.salt_hex));
        assert!(!env_dbg.contains(&env.nonce_hex));
        assert!(env_dbg.contains("[REDACTED]"));
    }

    #[test]
    #[cfg(unix)]
    fn legacy_leveldb_migration_continues_past_unreadable_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("llu-leveldb-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // 000001.ldb is a directory (or unreadable), so fs::read(&path) fails on it!
        // Previously `fs::read(&path).ok()?` aborted the entire function before reading 000002.log.
        let bad_entry = dir.join("000001.ldb");
        fs::create_dir_all(&bad_entry).unwrap();

        let good_entry = dir.join("000002.log");
        let payload = r#"junk{"state":{"token":"legacy-tok-42","accountId":"legacy-acc-42","country":"de","language":"de"},"version":0}tail"#;
        fs::write(&good_entry, payload).unwrap();
        let _ = fs::set_permissions(&good_entry, fs::Permissions::from_mode(0o600));

        let migrated = try_migrate_legacy_leveldb_credentials(&dir).expect("should migrate from 000002.log");
        assert_eq!(migrated.token, "legacy-tok-42");
        assert_eq!(migrated.account_id, "legacy-acc-42");
        assert_eq!(migrated.country, "de");

        let _ = fs::remove_dir_all(&dir);
    }
}
