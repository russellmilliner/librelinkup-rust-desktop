//! Credential (and health-data) vault: AES-256-GCM envelopes with a PBKDF2-derived key,
//! an injectable master-secret provider (file+keyring on Linux, DPAPI on Windows), and
//! forward-compatible envelope versioning.
//!
//! ## Security model
//!
//! The KDF input is a **256-bit CSPRNG key**, never a user-chosen password. The PBKDF2
//! step therefore is not the security boundary (there is no low-entropy secret to stretch);
//! it only domain-separates the stored key from the AES key. The actual boundary is the
//! key's storage: the OS keyring (Secret Service / KWallet) or a `0600` file owned by the
//! user, DPAPI-protected on Windows.
//!
//! `/etc/machine-id` is mixed into the *legacy* file+key path as a speed bump against
//! copying `master.key` alone to another machine. It is world-readable by design, so it
//! provides no secrecy against a local attacker - it is not relied on for that.

use std::fmt;
use std::fs;
use std::num::NonZeroU32;
use std::path::Path;
#[cfg(unix)]
use std::sync::Mutex;
use std::sync::OnceLock;

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::hkdf;
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::paths::{
    credentials_file_path, legacy_flatpak_leveldb_dir, master_key_file_path, write_file_atomic_0600,
};

/// Current envelope format: AES-256-GCM with HKDF-SHA256 key derivation and bound metadata.
const ENVELOPE_VERSION: u32 = 3;
/// Legacy v2 envelope format (PBKDF2-HMAC-SHA256 + bound metadata). Accepted for reads only.
const ENVELOPE_VERSION_V2: u32 = 2;
/// Legacy v1 envelope format (PBKDF2-HMAC-SHA256 + static AAD). Accepted for credential reads only.
const ENVELOPE_VERSION_V1: u32 = 1;
const KDF_HKDF_SHA256: &str = "HKDF-SHA256";
const KDF_PBKDF2_SHA256: &str = "PBKDF2-HMAC-SHA256";
/// Envelopes claiming implausible PBKDF2 iteration counts are rejected (DoS / downgrade guard).
const PBKDF2_MIN_ACCEPTED_ITERATIONS: u32 = 1_000;
const PBKDF2_MAX_ACCEPTED_ITERATIONS: u32 = 10_000_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
/// Associated-data context used by format v1 (fixed string, no metadata binding).
const AAD_CONTEXT_V1: &[u8] = b"librelinkup-kde-credentials-v1";
/// Separates credential envelopes from health-data envelopes so ciphertexts are not
/// interchangeable between vaults.
pub const AAD_LABEL_CREDENTIALS: &str = "credentials";
pub const AAD_LABEL_GLUCOSE_HISTORY: &str = "glucose-history";
pub const AAD_LABEL_MEDICATIONS: &str = "medications";

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
    /// Optional Home Assistant Long-Lived Access Token (encrypted inside the same AES-256-GCM vault).
    #[serde(default)]
    pub ha_token: String,
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
            .field("ha_token", &"[REDACTED]")
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
        self.first_name.zeroize();
        self.last_name.zeroize();
        self.ha_token.zeroize();
    }
}

impl StoredCredentials {
    pub fn has_valid_llu_session(&self) -> bool {
        !self.token.trim().is_empty() && !self.account_id.trim().is_empty()
    }

    pub fn has_valid_ha_session(&self) -> bool {
        !self.ha_token.trim().is_empty()
    }

    pub fn has_valid_session(&self) -> bool {
        self.has_valid_llu_session() || self.has_valid_ha_session()
    }

    pub fn has_valid_session_for(&self, data_source: &str, ha_url: &str) -> bool {
        if data_source.trim().eq_ignore_ascii_case("home_assistant") {
            self.has_valid_ha_session() && !ha_url.trim().is_empty()
        } else {
            self.has_valid_llu_session()
        }
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

/// Parses `HA_URL` and `HA_TOKEN` from a dotenv-style configuration string.
pub fn parse_ha_env_contents(raw: &str) -> Option<(String, String)> {
    let mut url = String::new();
    let mut token = String::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let cleaned = trimmed.strip_prefix("export ").unwrap_or(trimmed).trim();
        if let Some((k, v)) = cleaned.split_once('=') {
            let key = k.trim();
            let val = v.trim().trim_matches('"').trim_matches('\'').trim();
            if key == "HA_URL" && !val.is_empty() {
                url = val.to_string();
            } else if key == "HA_TOKEN" && !val.is_empty() {
                token.zeroize();
                token = val.to_string();
            }
        }
    }
    if !url.is_empty() && !token.is_empty() {
        Some((url, token))
    } else {
        url.zeroize();
        token.zeroize();
        None
    }
}

/// Attempts to read `HA_URL` and `HA_TOKEN` from `~/.config/home-assistant.env` on the local host.
pub fn try_load_host_ha_env() -> Option<(String, String)> {
    let mut candidates = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(std::path::PathBuf::from(home).join(".config/home-assistant.env"));
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        candidates.push(std::path::PathBuf::from(userprofile).join(".config/home-assistant.env"));
    }
    for path in candidates {
        if let Ok(mut raw) = fs::read_to_string(&path) {
            let parsed = parse_ha_env_contents(&raw);
            raw.zeroize();
            if parsed.is_some() {
                return parsed;
            }
        }
    }
    None
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

const HEX_LOWER: &[u8; 16] = b"0123456789abcdef";

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX_LOWER[(b >> 4) as usize] as char);
        out.push(HEX_LOWER[(b & 0x0f) as usize] as char);
    }
    out
}

pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    let clean = hex.trim();
    if !clean.len().is_multiple_of(2) {
        return Err("Invalid hex length (must be even)".to_string());
    }
    let mut out = Vec::with_capacity(clean.len() / 2);
    let bytes = clean.as_bytes();
    for pair in bytes.as_chunks::<2>().0 {
        let s = std::str::from_utf8(pair).map_err(|e| e.to_string())?;
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

struct HkdfAes256KeyType;

impl hkdf::KeyType for HkdfAes256KeyType {
    fn len(&self) -> usize {
        KEY_LEN
    }
}

fn derive_hkdf_aes_key(
    master_secret: &[u8],
    salt: &[u8],
    label: &str,
) -> Result<[u8; KEY_LEN], String> {
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, salt).extract(master_secret);
    let info = [label.as_bytes(), b":v3:aes256gcm"];
    let okm = prk
        .expand(&info, HkdfAes256KeyType)
        .map_err(|_| "HKDF key expansion failed".to_string())?;
    let mut key = [0u8; KEY_LEN];
    okm.fill(&mut key)
        .map_err(|_| "HKDF key material extraction failed".to_string())?;
    Ok(key)
}

fn derive_aes_key(
    master_secret: &[u8],
    salt: &[u8],
    iterations: u32,
) -> Result<[u8; KEY_LEN], String> {
    let iterations = NonZeroU32::new(iterations)
        .filter(|n| {
            let n = n.get();
            (PBKDF2_MIN_ACCEPTED_ITERATIONS..=PBKDF2_MAX_ACCEPTED_ITERATIONS).contains(&n)
        })
        .ok_or_else(|| format!("Refusing implausible PBKDF2 iteration count: {iterations}"))?;
    let mut key = [0u8; KEY_LEN];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        iterations,
        salt,
        master_secret,
        &mut key,
    );
    Ok(key)
}

/// Builds the AEAD associated data for an envelope. Format v1 used a fixed context;
/// formats v2 and v3 bind the label, KDF parameters, and salt so they cannot be tampered
/// with (or swapped between vaults) without failing the authentication tag.
fn associated_data(version: u32, label: &str, iterations: u32, salt: &[u8]) -> Vec<u8> {
    if version == ENVELOPE_VERSION_V1 {
        return AAD_CONTEXT_V1.to_vec();
    }
    let mut aad = Vec::with_capacity(48 + label.len() + salt.len());
    aad.extend_from_slice(b"librelinkup-kde-");
    aad.extend_from_slice(label.as_bytes());
    if version == ENVELOPE_VERSION {
        aad.extend_from_slice(b"-v3\0");
        aad.extend_from_slice(KDF_HKDF_SHA256.as_bytes());
        aad.push(0);
        aad.extend_from_slice(&iterations.to_le_bytes());
        aad.extend_from_slice(salt);
    } else {
        aad.extend_from_slice(b"-v2\0");
        aad.extend_from_slice(&iterations.to_le_bytes());
        aad.extend_from_slice(salt);
    }
    aad
}

/// Encrypts arbitrary plaintext bytes into an [`EncryptedEnvelope`] (format v3, HKDF-SHA256).
pub fn encrypt_bytes_with_secret(
    plaintext: &[u8],
    master_secret: &[u8],
    label: &str,
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

    let mut derived_key = derive_hkdf_aes_key(master_secret, &salt, label)?;
    let unbound_key = UnboundKey::new(&AES_256_GCM, &derived_key)
        .map_err(|_| "Failed to initialize AES-256-GCM key".to_string())?;
    derived_key.zeroize();

    let key = LessSafeKey::new(unbound_key);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let aad = associated_data(ENVELOPE_VERSION, label, 0, &salt);

    let mut in_out = plaintext.to_vec();
    if key
        .seal_in_place_append_tag(nonce, Aad::from(aad.as_slice()), &mut in_out)
        .is_err()
    {
        in_out.zeroize();
        return Err("AES-256-GCM encryption failed".to_string());
    }

    let ciphertext_hex = bytes_to_hex(&in_out);
    in_out.zeroize();

    Ok(EncryptedEnvelope {
        version: ENVELOPE_VERSION,
        algorithm: "AES-256-GCM".to_string(),
        kdf: KDF_HKDF_SHA256.to_string(),
        iterations: 0,
        salt_hex: bytes_to_hex(&salt),
        nonce_hex: bytes_to_hex(&nonce_bytes),
        ciphertext_hex,
    })
}

/// Decrypts an [`EncryptedEnvelope`] (v1, v2 PBKDF2, or v3 HKDF) back into plaintext bytes.
pub fn decrypt_bytes_with_secret(
    envelope: &EncryptedEnvelope,
    master_secret: &[u8],
    label: &str,
) -> Result<Vec<u8>, String> {
    if envelope.version != ENVELOPE_VERSION
        && envelope.version != ENVELOPE_VERSION_V2
        && envelope.version != ENVELOPE_VERSION_V1
    {
        return Err(format!("Unsupported envelope version: {}", envelope.version));
    }
    if envelope.version == ENVELOPE_VERSION_V1 && label != AAD_LABEL_CREDENTIALS {
        return Err("Legacy v1 envelopes are only accepted for credentials".to_string());
    }
    if envelope.algorithm != "AES-256-GCM" {
        return Err(format!(
            "Unsupported cipher algorithm: {}",
            envelope.algorithm
        ));
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
        ciphertext.zeroize();
        return Err("Ciphertext is shorter than authentication tag".to_string());
    }

    let iterations = envelope.iterations;
    let mut derived_key = if envelope.version == ENVELOPE_VERSION {
        if envelope.kdf != KDF_HKDF_SHA256 {
            ciphertext.zeroize();
            return Err(format!("Unsupported key derivation: {}", envelope.kdf));
        }
        if iterations != 0 {
            ciphertext.zeroize();
            return Err(format!(
                "Invalid iteration count {iterations} for {KDF_HKDF_SHA256} envelope"
            ));
        }
        match derive_hkdf_aes_key(master_secret, &salt, label) {
            Ok(k) => k,
            Err(e) => {
                ciphertext.zeroize();
                return Err(e);
            }
        }
    } else {
        if envelope.kdf != KDF_PBKDF2_SHA256 {
            ciphertext.zeroize();
            return Err(format!("Unsupported key derivation: {}", envelope.kdf));
        }
        match derive_aes_key(master_secret, &salt, iterations) {
            Ok(k) => k,
            Err(e) => {
                ciphertext.zeroize();
                return Err(e);
            }
        }
    };

    let unbound_key = match UnboundKey::new(&AES_256_GCM, &derived_key) {
        Ok(k) => {
            derived_key.zeroize();
            k
        }
        Err(_) => {
            derived_key.zeroize();
            ciphertext.zeroize();
            return Err("Failed to initialize AES-256-GCM key".to_string());
        }
    };

    let key = LessSafeKey::new(unbound_key);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let aad = associated_data(envelope.version, label, iterations, &salt);

    let plaintext = match key.open_in_place(nonce, Aad::from(aad.as_slice()), &mut ciphertext) {
        Ok(pt) => pt.to_vec(),
        Err(_) => {
            ciphertext.zeroize();
            return Err(
                "Vault authentication failed: wrong key, tampered envelope, or wrong vault label"
                    .to_string(),
            );
        }
    };
    ciphertext.zeroize();

    Ok(plaintext)
}

pub fn encrypt_credentials_with_secret(
    creds: &StoredCredentials,
    master_secret: &[u8],
) -> Result<EncryptedEnvelope, String> {
    let mut plaintext =
        serde_json::to_vec(creds).map_err(|e| format!("Failed to serialize credentials: {e}"))?;
    let res = encrypt_bytes_with_secret(&plaintext, master_secret, AAD_LABEL_CREDENTIALS);
    plaintext.zeroize();
    res
}

pub fn decrypt_credentials_with_secret(
    envelope: &EncryptedEnvelope,
    master_secret: &[u8],
) -> Result<StoredCredentials, String> {
    let mut plaintext =
        decrypt_bytes_with_secret(envelope, master_secret, AAD_LABEL_CREDENTIALS)?;
    let res = serde_json::from_slice::<StoredCredentials>(&plaintext)
        .map_err(|e| format!("Failed to parse decrypted credentials JSON: {e}"));
    plaintext.zeroize();
    res
}

// ---------------------------------------------------------------------------
// Master-secret provider
// ---------------------------------------------------------------------------

/// Signature of a master-secret provider: returns the 32-byte key used to wrap the vaults.
pub type SecretProvider = fn() -> Result<Zeroizing<Vec<u8>>, String>;

static SECRET_PROVIDER: OnceLock<SecretProvider> = OnceLock::new();

/// Installs the platform-specific master-secret provider (Windows DPAPI). Front-ends call
/// this once at startup before touching any vault. Without it, the file + keyring
/// ([`load_or_create_master_secret`]) path is used.
pub fn set_secret_provider(provider: SecretProvider) {
    let _ = SECRET_PROVIDER.set(provider);
}

/// Returns the master secret for health-data vaults (glucose history, medications).
pub fn phi_secret() -> Result<Zeroizing<Vec<u8>>, String> {
    if let Some(provider) = SECRET_PROVIDER.get() {
        provider()
    } else {
        load_or_create_master_secret(&master_key_file_path())
    }
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
fn try_keyring_lookup() -> Option<Zeroizing<String>> {
    use std::process::Command;

    let candidates: &[(&str, &[&str])] = &[
        (
            "secret-tool",
            &[
                "lookup",
                "application",
                crate::paths::APP_ID,
                "key",
                "vault-master-secret",
            ],
        ),
        (
            "kwallet-query",
            &["-r", "vault-master-secret", "-f", "LibreLinkUp", "kdewallet"],
        ),
        (
            "flatpak-spawn",
            &[
                "--host",
                "secret-tool",
                "lookup",
                "application",
                crate::paths::APP_ID,
                "key",
                "vault-master-secret",
            ],
        ),
    ];

    for (program, args) in candidates {
        if *program == "flatpak-spawn" && !Path::new("/.flatpak-info").exists() {
            continue;
        }
        if let Ok(mut out) = Command::new(program).args(*args).output() {
            let valid = if out.status.success() {
                let mut raw = String::from_utf8_lossy(&out.stdout).into_owned();
                let s = Zeroizing::new(raw.trim().to_string());
                raw.zeroize();
                out.stdout.zeroize();
                out.stderr.zeroize();
                if s.len() == KEY_LEN * 2 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
                    Some(s)
                } else {
                    None
                }
            } else {
                out.stdout.zeroize();
                out.stderr.zeroize();
                None
            };
            if let Some(s) = valid {
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
        if let Some(mut stdin) = child.stdin.take()
            && stdin.write_all(hex_secret.as_bytes()).is_err() {
                let _ = child.kill();
                let _ = child.wait();
                return false;
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

    if Path::new("/.flatpak-info").exists() {
        let mut host_secret_tool = Command::new("flatpak-spawn");
        host_secret_tool.args([
            "--host",
            "secret-tool",
            "store",
            "--label=LibreLinkUp Rust Desktop Vault Key",
            "application",
            crate::paths::APP_ID,
            "key",
            "vault-master-secret",
        ]);
        if try_pipe(host_secret_tool) {
            return true;
        }
    }

    let mut kwallet = Command::new("kwallet-query");
    kwallet.args(["-w", "vault-master-secret", "-f", "LibreLinkUp", "kdewallet"]);
    try_pipe(kwallet)
}

#[cfg(unix)]
fn try_keyring_store_cached(hex_secret: &str) -> bool {
    static MIRRORED_DIGEST: OnceLock<Mutex<Option<[u8; 32]>>> = OnceLock::new();
    let d = ring::digest::digest(&ring::digest::SHA256, hex_secret.as_bytes());
    let mut digest_arr = [0u8; 32];
    digest_arr.copy_from_slice(d.as_ref());

    let lock = MIRRORED_DIGEST.get_or_init(|| Mutex::new(None));
    {
        let guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        if *guard == Some(digest_arr) {
            return true;
        }
    }
    let ok = try_keyring_store(hex_secret);
    if ok {
        let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(digest_arr);
    }
    ok
}

fn cached_machine_id() -> &'static str {
    static MACHINE_ID: OnceLock<String> = OnceLock::new();
    MACHINE_ID.get_or_init(|| {
        fs::read_to_string("/etc/machine-id")
            .unwrap_or_else(|_| "librelinkup-kde-fallback-machine-id".to_string())
            .trim()
            .to_string()
    })
}

/// Loads (or creates on first run) the file/keyring master secret used by the vaults.
///
/// **Never silently rotates the key.** If a vault already exists but no key can be
/// recovered, this returns an error instead of minting a new key, because a new key would
/// make every existing vault permanently undecryptable.
pub fn load_or_create_master_secret(key_path: &Path) -> Result<Zeroizing<Vec<u8>>, String> {
    load_or_create_master_secret_guarded(key_path, None)
}

/// Same as [`load_or_create_master_secret`] but refuses to mint a new key when
/// `vault_path` (or any default vault when using the default master key path) exists.
pub fn load_or_create_master_secret_guarded(
    key_path: &Path,
    vault_path: Option<&Path>,
) -> Result<Zeroizing<Vec<u8>>, String> {
    if key_path == master_key_file_path() {
        let creds = credentials_file_path();
        let meds = crate::paths::medications_file_path();
        let hist = crate::paths::glucose_history_file_path();
        if let Some(p) = vault_path {
            load_or_create_master_secret_for_vaults(key_path, &[p, &creds, &meds, &hist])
        } else {
            load_or_create_master_secret_for_vaults(key_path, &[&creds, &meds, &hist])
        }
    } else if let Some(p) = vault_path {
        load_or_create_master_secret_for_vaults(key_path, &[p])
    } else {
        load_or_create_master_secret_for_vaults(key_path, &[])
    }
}

/// Same as [`load_or_create_master_secret`] but refuses to mint a new key when any path
/// in `guarded_vaults` exists.
pub fn load_or_create_master_secret_for_vaults(
    key_path: &Path,
    guarded_vaults: &[&Path],
) -> Result<Zeroizing<Vec<u8>>, String> {
    let machine_id = cached_machine_id();

    #[cfg(unix)]
    let use_keyring = should_use_desktop_keyring(key_path);

    let vault_exists = guarded_vaults.iter().any(|p| p.exists());

    let parse_valid_key_hex = |hex: &str| -> Result<Vec<u8>, String> {
        let bytes = hex_to_bytes(hex)?;
        if bytes.len() != KEY_LEN {
            return Err(format!(
                "Invalid master key length: expected {KEY_LEN} bytes, got {}",
                bytes.len()
            ));
        }
        Ok(bytes)
    };

    let file_secret: Zeroizing<Vec<u8>> = if key_path.exists() {
        let mut raw_hex = fs::read_to_string(key_path)
            .map_err(|e| format!("Failed to read master key file: {e}"))?;
        let trimmed = Zeroizing::new(raw_hex.trim().to_string());
        raw_hex.zeroize();
        match parse_valid_key_hex(&trimmed) {
            Ok(bytes) => {
                // Keep the 0600 master.key file on disk so both host and Flatpak launches (and
                // early login autostart before keyring unlock) share the exact same key, while
                // mirroring into Secret Service / KWallet once per key digest.
                #[cfg(unix)]
                if use_keyring {
                    let _ = try_keyring_store_cached(&trimmed);
                }
                Zeroizing::new(bytes)
            }
            Err(file_err) => {
                // Never overwrite the desktop keyring with a corrupted/truncated key file; try
                // recovering from keyring if available, otherwise fail closed.
                #[cfg(unix)]
                let recovered = if use_keyring {
                    try_keyring_lookup()
                } else {
                    None
                };
                #[cfg(not(unix))]
                let recovered: Option<Zeroizing<String>> = None;

                if let Some(kr_hex) = recovered
                    && let Ok(secret) = parse_valid_key_hex(&kr_hex)
                {
                    let _ = write_file_atomic_0600(key_path, kr_hex.as_bytes());
                    Zeroizing::new(secret)
                } else {
                    return Err(format!(
                        "Master key file is invalid or corrupted ({file_err}). Refusing to overwrite or mirror an invalid key."
                    ));
                }
            }
        }
    } else {
        #[cfg(unix)]
        let keyring_hex = if use_keyring {
            try_keyring_lookup()
        } else {
            None
        };
        #[cfg(not(unix))]
        let keyring_hex: Option<Zeroizing<String>> = None;

        if let Some(hex_str) = keyring_hex {
            if let Ok(secret) = parse_valid_key_hex(&hex_str) {
                let _ = write_file_atomic_0600(key_path, hex_str.as_bytes());
                Zeroizing::new(secret)
            } else {
                return Err("Corrupted master key in desktop keyring".to_string());
            }
        } else if vault_exists {
            // Fail closed: an existing vault must never be orphaned by rotating the key.
            return Err(
                "An encrypted vault exists but its master key is not available (keyring locked, \
                 secret service unreachable, or the key file was deleted). Refusing to create a \
                 new key, which would make the existing credentials permanently unreadable. \
                 Unlock your keyring and retry, or sign in again to replace the vault."
                    .to_string(),
            );
        } else {
            let mut random_key = Zeroizing::new(generate_random_key()?);
            let hex_str = Zeroizing::new(bytes_to_hex(&*random_key));
            random_key.zeroize();

            write_file_atomic_0600(key_path, hex_str.as_bytes())
                .map_err(|e| format!("Failed to write master key file: {e}"))?;
            #[cfg(unix)]
            if use_keyring {
                let _ = try_keyring_store_cached(&hex_str);
            }
            Zeroizing::new(parse_valid_key_hex(&hex_str)?)
        }
    };

    let mut combined = Vec::with_capacity(file_secret.len() + machine_id.len() + 32);
    combined.extend_from_slice(&file_secret);
    combined.extend_from_slice(b":librelinkup-kde:");
    combined.extend_from_slice(machine_id.as_bytes());
    Ok(Zeroizing::new(combined))
}

/// Generic encrypted JSON vault bound to an AAD context label (`credentials`, `glucose-history`,
/// or `medications`) using [`encrypt_bytes_with_secret`] / [`decrypt_bytes_with_secret`].
pub struct EncryptedVault<T> {
    label: &'static str,
    pretty: bool,
    _marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> EncryptedVault<T> {
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            pretty: false,
            _marker: std::marker::PhantomData,
        }
    }

    pub const fn with_pretty(mut self, pretty: bool) -> Self {
        self.pretty = pretty;
        self
    }
}

impl<T: Serialize> EncryptedVault<T> {
    pub fn save_with_secret(
        &self,
        value: &T,
        path: &Path,
        master_secret: &[u8],
    ) -> Result<(), String> {
        let mut json = serde_json::to_vec(value)
            .map_err(|e| format!("Failed to serialize {} vault: {e}", self.label))?;
        let res = encrypt_bytes_with_secret(&json, master_secret, self.label).and_then(|envelope| {
            let serialized = if self.pretty {
                serde_json::to_string_pretty(&envelope)
            } else {
                serde_json::to_string(&envelope)
            }
            .map_err(|e| format!("Failed to serialize {} envelope: {e}", self.label))?;
            write_file_atomic_0600(path, serialized.as_bytes())
                .map_err(|e| format!("Failed to write {} vault: {e}", self.label))
        });
        json.zeroize();
        res
    }
}

impl<T: DeserializeOwned> EncryptedVault<T> {
    pub fn load_with_secret(
        &self,
        path: &Path,
        master_secret: &[u8],
    ) -> Result<Option<T>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {} vault: {e}", self.label))?;
        let envelope: EncryptedEnvelope = serde_json::from_str(&raw)
            .map_err(|e| format!("Corrupted {} envelope: {e}", self.label))?;
        let mut plaintext = decrypt_bytes_with_secret(&envelope, master_secret, self.label)?;
        let parsed = serde_json::from_slice::<T>(&plaintext)
            .map_err(|e| format!("Corrupted {} payload: {e}", self.label));
        plaintext.zeroize();
        parsed.map(Some)
    }
}

fn credentials_vault() -> EncryptedVault<StoredCredentials> {
    EncryptedVault::new(AAD_LABEL_CREDENTIALS).with_pretty(true)
}

pub fn save_credentials_to_path(
    creds: &StoredCredentials,
    creds_path: &Path,
    key_path: &Path,
) -> Result<(), String> {
    let mut secret = load_or_create_master_secret_guarded(key_path, Some(creds_path))?;
    let res = credentials_vault().save_with_secret(creds, creds_path, &secret);
    secret.zeroize();
    res
}

/// Encrypts and saves credentials using a caller-supplied master secret (e.g. a key
/// protected by Windows DPAPI) instead of the file + machine-id derived secret.
pub fn save_credentials_with_secret(
    creds: &StoredCredentials,
    creds_path: &Path,
    master_secret: &[u8],
) -> Result<(), String> {
    credentials_vault().save_with_secret(creds, creds_path, master_secret)
}

/// Loads and decrypts credentials using a caller-supplied master secret.
pub fn load_credentials_with_secret(
    creds_path: &Path,
    master_secret: &[u8],
) -> Result<Option<StoredCredentials>, String> {
    credentials_vault().load_with_secret(creds_path, master_secret)
}

pub fn load_credentials_from_path(
    creds_path: &Path,
    key_path: &Path,
) -> Result<Option<StoredCredentials>, String> {
    if !creds_path.exists() {
        return Ok(None);
    }
    let mut secret = load_or_create_master_secret_guarded(key_path, Some(creds_path))?;
    let res = credentials_vault().load_with_secret(creds_path, &secret);
    secret.zeroize();
    res
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
        let Ok(mut bytes) = fs::read(&path) else {
            continue;
        };
        let mut lossy = String::from_utf8_lossy(&bytes).into_owned();
        bytes.zeroize();
        for chunk in lossy.split("{\"state\":{") {
            if let Some(end_idx) = chunk.find("},\"version\"") {
                let mut json_str = format!("{{{}}}", &chunk[..end_idx]);
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
                            ha_token: String::new(),
                        });
                    }
                }
                json_str.zeroize();
            }
        }
        lossy.zeroize();
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_creds() -> StoredCredentials {
        StoredCredentials {
            username: "secret.user@example.com".into(),
            password: "MySuperSecretPassword!".into(),
            token: "eyJhbGciOiJIUzI1NiJ9.secret".into(),
            account_id: "acc-uuid-9999".into(),
            country: "us".into(),
            language: "en".into(),
            patient_id: "pat-uuid-8888".into(),
            first_name: "SecretFirst".into(),
            last_name: "SecretLast".into(),
            ha_token: "ha-long-lived-secret-token".into(),
        }
    }

    #[test]
    fn stored_credentials_and_envelope_debug_redact_secrets() {
        let creds = sample_creds();
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
    fn envelope_round_trips_and_binds_its_metadata() {
        let secret = b"envelope-test-secret";
        let env = encrypt_bytes_with_secret(b"hello vault", secret, AAD_LABEL_MEDICATIONS).unwrap();
        assert_eq!(env.version, ENVELOPE_VERSION);
        assert_eq!(env.kdf, KDF_HKDF_SHA256);
        assert_eq!(env.iterations, 0);
        assert_eq!(
            decrypt_bytes_with_secret(&env, secret, AAD_LABEL_MEDICATIONS).unwrap(),
            b"hello vault"
        );

        // Wrong label (vault cross-use) must fail both HKDF info and AAD tag checks.
        assert!(decrypt_bytes_with_secret(&env, secret, AAD_LABEL_GLUCOSE_HISTORY).is_err());

        // Tampering with KDF or iterations on v3 envelopes must fail.
        let mut tampered_iter = env.clone();
        tampered_iter.iterations = 1;
        assert!(decrypt_bytes_with_secret(&tampered_iter, secret, AAD_LABEL_MEDICATIONS).is_err());

        let mut tampered_kdf = env.clone();
        tampered_kdf.kdf = KDF_PBKDF2_SHA256.to_string();
        assert!(decrypt_bytes_with_secret(&tampered_kdf, secret, AAD_LABEL_MEDICATIONS).is_err());

        // Wrong key must fail.
        assert!(decrypt_bytes_with_secret(&env, b"other-secret", AAD_LABEL_MEDICATIONS).is_err());
    }

    #[test]
    fn legacy_v2_envelopes_still_decrypt_and_bind_metadata() {
        let secret = b"legacy-v2-secret";
        let salt = [5u8; SALT_LEN];
        let nonce = [11u8; NONCE_LEN];
        let iterations = 10_000u32;
        let mut derived = derive_aes_key(secret, &salt, iterations).unwrap();
        let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &derived).unwrap());
        derived.zeroize();
        let mut ct = b"v2-plaintext".to_vec();
        let aad = associated_data(ENVELOPE_VERSION_V2, AAD_LABEL_MEDICATIONS, iterations, &salt);
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(aad.as_slice()),
            &mut ct,
        )
        .unwrap();
        let env = EncryptedEnvelope {
            version: ENVELOPE_VERSION_V2,
            algorithm: "AES-256-GCM".into(),
            kdf: KDF_PBKDF2_SHA256.into(),
            iterations,
            salt_hex: bytes_to_hex(&salt),
            nonce_hex: bytes_to_hex(&nonce),
            ciphertext_hex: bytes_to_hex(&ct),
        };
        assert_eq!(
            decrypt_bytes_with_secret(&env, secret, AAD_LABEL_MEDICATIONS).unwrap(),
            b"v2-plaintext"
        );
        assert!(decrypt_bytes_with_secret(&env, secret, AAD_LABEL_GLUCOSE_HISTORY).is_err());

        let mut tampered = env.clone();
        tampered.iterations += 1;
        assert!(decrypt_bytes_with_secret(&tampered, secret, AAD_LABEL_MEDICATIONS).is_err());

        let mut downgraded = env;
        downgraded.iterations = 0;
        assert!(decrypt_bytes_with_secret(&downgraded, secret, AAD_LABEL_MEDICATIONS).is_err());
    }

    #[test]
    fn legacy_v1_envelopes_still_decrypt() {
        // A v1 envelope is the same ciphertext but with the static AAD context and the
        // historical iteration count; simulate one by re-encrypting with the v1 AAD.
        let secret = b"legacy-secret";
        let salt = [7u8; SALT_LEN];
        let nonce = [9u8; NONCE_LEN];
        let mut derived = derive_aes_key(secret, &salt, 100_000).unwrap();
        let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &derived).unwrap());
        derived.zeroize();
        let mut ct = b"legacy-plaintext".to_vec();
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(AAD_CONTEXT_V1),
            &mut ct,
        )
        .unwrap();
        let envelope = EncryptedEnvelope {
            version: ENVELOPE_VERSION_V1,
            algorithm: "AES-256-GCM".into(),
            kdf: "PBKDF2-HMAC-SHA256".into(),
            iterations: 100_000,
            salt_hex: bytes_to_hex(&salt),
            nonce_hex: bytes_to_hex(&nonce),
            ciphertext_hex: bytes_to_hex(&ct),
        };
        assert_eq!(
            decrypt_bytes_with_secret(&envelope, secret, AAD_LABEL_CREDENTIALS).unwrap(),
            b"legacy-plaintext"
        );
    }

    #[test]
    fn missing_key_never_rotates_when_a_vault_exists() {
        let dir = std::env::temp_dir().join(format!("llu-keyguard-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let key_path = dir.join("master.key");
        let vault_path = dir.join("credentials.enc.json");
        fs::write(&vault_path, b"{}").unwrap();

        // Key file absent + vault present -> refuse, and do not create a key.
        let err = load_or_create_master_secret_guarded(&key_path, Some(&vault_path)).unwrap_err();
        assert!(err.contains("Refusing to create a new key"), "{err}");
        assert!(!key_path.exists(), "must not mint a replacement key");

        // No vault -> a key is created normally.
        fs::remove_file(&vault_path).unwrap();
        let secret = load_or_create_master_secret_guarded(&key_path, Some(&vault_path)).unwrap();
        assert!(!secret.is_empty());
        assert!(key_path.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_key_refuses_rotation_when_only_phi_vaults_exist_after_signout() {
        let dir = std::env::temp_dir().join(format!("llu-phiguard-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let key_path = dir.join("master.key");
        let creds_path = dir.join("credentials.enc.json");
        let meds_path = dir.join("medications.enc.json");
        let history_path = dir.join("glucose_history.enc.json");

        // Simulate signed-out state where credentials were wiped but 90-day glucose history remains.
        fs::write(&history_path, b"{}").unwrap();
        let err = load_or_create_master_secret_for_vaults(
            &key_path,
            &[&creds_path, &meds_path, &history_path],
        )
        .unwrap_err();
        assert!(err.contains("Refusing to create a new key"), "{err}");
        assert!(!key_path.exists(), "must not mint a replacement key");

        // Repeat with only medications vault present.
        fs::remove_file(&history_path).unwrap();
        fs::write(&meds_path, b"{}").unwrap();
        let err = load_or_create_master_secret_for_vaults(
            &key_path,
            &[&creds_path, &meds_path, &history_path],
        )
        .unwrap_err();
        assert!(err.contains("Refusing to create a new key"), "{err}");
        assert!(!key_path.exists(), "must not mint a replacement key");

        // Once no vault exists, a fresh master key may be created.
        fs::remove_file(&meds_path).unwrap();
        let secret = load_or_create_master_secret_for_vaults(
            &key_path,
            &[&creds_path, &meds_path, &history_path],
        )
        .unwrap();
        assert!(!secret.is_empty());
        assert!(key_path.exists());

        let _ = fs::remove_dir_all(&dir);
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

    #[test]
    fn rejects_corrupted_or_wrong_length_master_key_file() {
        let dir = std::env::temp_dir().join(format!("llu-badkey-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let key_path = dir.join("master.key");

        // Empty key file -> fails closed
        fs::write(&key_path, b"").unwrap();
        assert!(load_or_create_master_secret_for_vaults(&key_path, &[]).is_err());

        // Short hex (16 bytes instead of 32 bytes) -> fails closed
        fs::write(&key_path, "00112233445566778899aabbccddeeff").unwrap();
        assert!(load_or_create_master_secret_for_vaults(&key_path, &[]).is_err());

        // Non-hex garbage -> fails closed
        fs::write(&key_path, "not-valid-hex").unwrap();
        assert!(load_or_create_master_secret_for_vaults(&key_path, &[]).is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn v1_envelope_only_accepted_for_credentials_label() {
        let secret = vec![0x42u8; 32];
        let mut env = encrypt_bytes_with_secret(b"{}", &secret, AAD_LABEL_CREDENTIALS).unwrap();
        env.version = ENVELOPE_VERSION_V1;
        let err = decrypt_bytes_with_secret(&env, &secret, AAD_LABEL_MEDICATIONS).unwrap_err();
        assert!(err.contains("Legacy v1 envelopes"), "{err}");
    }

    #[test]
    fn v3_hkdf_envelope_round_trip_tamper_resistance_and_encrypted_vault() {
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct TestPayload {
            note: String,
            count: u32,
        }

        let secret = vec![0x7Au8; 32];
        let env = encrypt_bytes_with_secret(b"hello-hkdf-v3", &secret, AAD_LABEL_MEDICATIONS).unwrap();
        assert_eq!(env.version, 3);
        assert_eq!(env.kdf, KDF_HKDF_SHA256);
        assert_eq!(env.iterations, 0);

        let dec = decrypt_bytes_with_secret(&env, &secret, AAD_LABEL_MEDICATIONS).unwrap();
        assert_eq!(dec, b"hello-hkdf-v3");

        // Label / AAD mismatch must fail
        assert!(decrypt_bytes_with_secret(&env, &secret, AAD_LABEL_CREDENTIALS).is_err());
        // Tampered salt / nonce / ciphertext must fail
        let mut bad_salt = env.clone();
        bad_salt.salt_hex = bytes_to_hex(&[0xFFu8; SALT_LEN]);
        assert!(decrypt_bytes_with_secret(&bad_salt, &secret, AAD_LABEL_MEDICATIONS).is_err());

        let mut bad_nonce = env.clone();
        bad_nonce.nonce_hex = bytes_to_hex(&[0x00u8; NONCE_LEN]);
        assert!(decrypt_bytes_with_secret(&bad_nonce, &secret, AAD_LABEL_MEDICATIONS).is_err());

        let mut bad_ct = env;
        let mut ct_bytes = hex_to_bytes(&bad_ct.ciphertext_hex).unwrap();
        ct_bytes[0] ^= 0x01;
        bad_ct.ciphertext_hex = bytes_to_hex(&ct_bytes);
        assert!(decrypt_bytes_with_secret(&bad_ct, &secret, AAD_LABEL_MEDICATIONS).is_err());

        // EncryptedVault<T> file round-trip
        let dir = std::env::temp_dir().join(format!("llu-vault-gen-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_vault.enc.json");
        let vault = EncryptedVault::<TestPayload>::new("test-vault");
        assert!(vault.load_with_secret(&path, &secret).unwrap().is_none());

        let item = TestPayload {
            note: "encrypted-item".into(),
            count: 9,
        };
        vault.save_with_secret(&item, &path, &secret).unwrap();
        let loaded = vault.load_with_secret(&path, &secret).unwrap().unwrap();
        assert_eq!(loaded, item);
        let _ = fs::remove_dir_all(&dir);
    }
}
