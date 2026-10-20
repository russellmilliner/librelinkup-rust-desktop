use std::fs;
use std::num::NonZeroU32;
use std::path::Path;

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::paths::{credentials_file_path, legacy_flatpak_leveldb_dir, master_key_file_path};

const ENVELOPE_VERSION: u32 = 1;
const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const AAD_CONTEXT: &[u8] = b"librelinkup-kde-credentials-v1";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    pub version: u32,
    pub algorithm: String,
    pub kdf: String,
    pub iterations: u32,
    pub salt_hex: String,
    pub nonce_hex: String,
    pub ciphertext_hex: String,
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

fn set_owner_only_permissions(path: &Path) {
    // Windows: %APPDATA% is already restricted to the user by its ACL.
    #[cfg(not(unix))]
    let _ = path;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = fs::set_permissions(path, perms);
        }
    }
}

pub fn load_or_create_master_secret(key_path: &Path) -> Result<Vec<u8>, String> {
    let machine_id = fs::read_to_string("/etc/machine-id")
        .unwrap_or_else(|_| "librelinkup-kde-fallback-machine-id".to_string());

    let file_secret = if key_path.exists() {
        let raw_hex = fs::read_to_string(key_path)
            .map_err(|e| format!("Failed to read master key file: {e}"))?;
        hex_to_bytes(raw_hex.trim())?
    } else {
        if let Some(parent) = key_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let rng = SystemRandom::new();
        let mut random_key = [0u8; KEY_LEN];
        rng.fill(&mut random_key)
            .map_err(|_| "CSPRNG failure generating master key".to_string())?;
        let hex_str = bytes_to_hex(&random_key);
        random_key.zeroize();
        fs::write(key_path, &hex_str)
            .map_err(|e| format!("Failed to write master key file: {e}"))?;
        set_owner_only_permissions(key_path);
        hex_to_bytes(&hex_str)?
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

    if let Some(parent) = creds_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let serialized = serde_json::to_string_pretty(&envelope)
        .map_err(|e| format!("Failed to serialize encrypted envelope: {e}"))?;
    fs::write(creds_path, serialized)
        .map_err(|e| format!("Failed to write encrypted credentials file: {e}"))?;
    set_owner_only_permissions(creds_path);
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
    if let Some(parent) = creds_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let serialized = serde_json::to_string_pretty(&envelope)
        .map_err(|e| format!("Failed to serialize encrypted envelope: {e}"))?;
    fs::write(creds_path, serialized)
        .map_err(|e| format!("Failed to write encrypted credentials file: {e}"))?;
    set_owner_only_permissions(creds_path);
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
    let mut paths: Vec< std::path::PathBuf > = entries
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
        let bytes = fs::read(&path).ok()?;
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
                    let country = val
                        .get("country")
                        .and_then(|v| v.as_str())
                        .unwrap_or("us")
                        .trim();
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
                            country: if country.is_empty() {
                                "us".to_string()
                            } else {
                                country.to_string()
                            },
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
