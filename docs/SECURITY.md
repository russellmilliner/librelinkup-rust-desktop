# Security Model — LibreLinkUp Rust Desktop

> **Audience:** maintainers, auditors, and AI agents. Read
> [`ARCHITECTURE.md`](ARCHITECTURE.md) first for the module map.

---

## 1. What this app holds

A LibreLinkUp follower account can see **another person's (often a child's) continuous glucose
data**. That makes the data in this app unusually sensitive:

| Data | Sensitivity | Where it lives |
|---|---|---|
| LibreView bearer token + account id + optional Home Assistant token | full account access | `credentials.enc.json` (AES-256-GCM) |
| Optional account password | full account access | same vault, only if the user opts in |
| Patient name, patient id | PII | same vault |
| 90-day glucose history | **PHI** | `glucose_history.enc.json` (AES-256-GCM) |
| Medication schedule + dose log | **PHI** | `medications.enc.json` (AES-256-GCM) |
| Alert thresholds | infers a diagnosis | `settings.json` (`0600`, plaintext) |

---

## 2. Trust boundaries

```
        ┌─────────────────┐   HTTPS (rustls, hostname-verified)
        │ LibreView API   │◄──────────────────────────────────┐
        └─────────────────┘                                   │
        ┌─────────────────┐   HTTPS + minisign signature      │
        │ GitHub Releases │◄─────────────────────────────┐    │
        └─────────────────┘                              │    │
                                            ┌────────────▼────▼──────────┐
   user session ──────────────────────────►│  LibreLinkUp Desktop       │
                                            │  (untrusted-input parser)  │
   local files (0600 / keyring / DPAPI) ───►└────────────┬───────────────┘
   OS notification + audio services ◄────────────────────┘
```

**Trusted:** the LibreView base URL allowlist, the embedded minisign public key, the OS
keyring/DPAPI, the user's own config directory.

**Untrusted:** every byte from the API (JSON is parsed defensively and every id used in a URL
path is validated), every release asset, every local file that is not ours.

---

## 3. Credential vault

```
master secret ──PBKDF2-HMAC-SHA256(210k, salt)──► AES-256 key
plaintext (JSON) ──AES-256-GCM(nonce, AAD)──────► envelope JSON (hex) ──► 0600 file
```

* **Envelope v2** binds `label ‖ iterations ‖ salt` into the AEAD associated data, so the
  credential vault, the glucose-history vault and the medication vault are mutually
  non-interchangeable, and tampering with the recorded iteration count fails the tag.
  Envelope **v1** (static AAD) is still readable for upgrades and is re-written as v2 on the
  next save.
* The KDF input is a **256-bit CSPRNG key**, never a user password. PBKDF2 is therefore
  domain separation, not password hardening — the real boundary is *where the key lives*.
* Key storage: freedesktop Secret Service (`secret-tool`) or KDE KWallet (`kwallet-query`) when
  a session bus exists; otherwise a `0600` file. Windows always uses **DPAPI**
  (`CryptProtectData`, current-user scope), so a copied vault cannot be decrypted elsewhere.
* `/etc/machine-id` is mixed into the file-key path as a speed bump only. It is world-readable
  and provides **no** secrecy; do not rely on it.
* Secrets are piped to `secret-tool`/`kwallet-query` over **stdin**, never argv.

### Failure behaviour

| Situation | Behaviour |
|---|---|
| Vault exists, key missing/unreadable | **refuse to start the vault** with an actionable error; never mint a new key |
| Wrong key / tampered ciphertext | GCM tag failure, credentials discarded, error surfaced |
| Keyring locked | falls back to the `0600` file if present, otherwise the error above |

---

## 4. Auto-update trust chain

```
check  → api.github.com/repos/.../releases/latest      (HTTPS, JSON)
verify → browser_download_url is github.com/.../releases/download/<tag>/<file>
         (no userinfo, no port, no query, no fragment, no traversal)
verify → SHA-256 sidecar matches the artifact           (integrity / truncation)
verify → minisign signature over the artifact matches  (authenticity)  ◄── required
execute→ Windows: run installer; Linux: `flatpak install --user --reinstall`
```

* The **SHA-256 alone is not a security control**: it is fetched from the same origin as the
  artifact, so it only detects corruption. A compromised repo/token can publish both.
* The **minisign signature** is verified against `UPDATE_SIGNING_PUBLIC_KEY`
  (`crates/librelinkup-core/src/updater.rs`), which is compiled into the binary and is the
  update trust root. Signing requires the secret key (`RELEASE_MINISIGN_KEY` CI secret /
  offline backup); it is never in the repo.
* Version strings are re-parsed into `(major, minor, patch)` before being used in file names,
  so a hostile tag cannot inject a path or shell metacharacter.
* Unsigned releases are **never executed**: the updater fails closed with an explicit error.
* Inside a Flatpak the installer uses `flatpak-spawn --host flatpak install`, which requires
  the `org.freedesktop.Flatpak` permission. Flathub builds do not have it, so the in-app
  installer cannot work there — updates for the Flathub build must come from the store.

---

## 5. Network surface

* Only `https://api[-<region>].libreview.io/llu` (12-entry allowlist) and `github.com`
  release assets. A login response that redirects to any other region is **rejected**, not
  followed.
* `http://127.0.0.1:<port>/llu` is accepted **only** when `debug_assertions` is on or
  `LIBRELINKUP_ALLOW_INSECURE_TEST_ENDPOINT=1` is set. Shipped (release) binaries refuse it,
  so a stored "region" can never turn into a cleartext password POST to a local port.
* `patientId` from `/connections` is validated against `[A-Za-z0-9_-]{1,128}` before being
  interpolated into `/connections/<id>/graph` (path-traversal defence).
* `curl` is used only as a fallback when Cloudflare blocks the rustls TLS fingerprint.
  Sensitive headers and bodies are supplied through `--config -` (stdin), never argv.
* Polling is clamped to ≥60 s and jittered by ±10%; failed re-authentication backs off
  exponentially (2 min → 60 min cap) so a wrong stored password cannot hammer the API.

---

## 6. Logging, privacy, and zero-tracking guarantee

* **No tracking and no advertisements — ever:** LibreLinkUp Rust Desktop contains **no
  advertisements, no analytics, no telemetry beacons, no crash-reporting SDKs, and no
  third-party tracking**, and never will. Network requests go exclusively to the allowlisted
  LibreView regional API and GitHub Releases (for signed update checks).
* Logs are an in-memory ring buffer (1000 entries) with categories `SYSTEM/AUTH/API/CRYPTO/
  GLUCOSE/ALERT/MEDS/TRAY`. Exports are `0600`.
* Email addresses are masked (`p***@example.com`); `Debug` impls for `StoredCredentials`,
  `AuthSuccess`, `GlucoseSnapshot`, `HistorySample`, `MedicationDef`, `MedicationDoseLog` and
  `EncryptedEnvelope` print `[REDACTED]`.
* **Never log** a token, password, account id, patient id, glucose value together with a
  name, or medication names. Add a redaction test when you add a struct that holds PHI.

---

## 7. Known limitations (accepted)

| Limitation | Rationale / mitigation |
|---|---|
| `settings.json` is plaintext | contains thresholds only, no identifiers; documented in the README |
| Live data in process memory is not mlocked | `zeroize` on drop is best-effort; a debugger with the same UID can read it |
| Keyring availability is environment-dependent | falls back to a `0600` file; the migration path is one-way and logged |
| Flathub build cannot self-update | by design (store-managed); the app shows the release page instead |
| No certificate pinning | rustls + platform-independent webpki roots; pinning would break on rotation |
| `flatpak-spawn --host` grants host execution in local (non-Flathub) builds | required for self-update; Flathub does not grant it |

---

## 8. Reporting a vulnerability

Open a private advisory via GitHub Security Advisories on
`russellmilliner/librelinkup-rust-desktop`, or email the maintainer listed in
`data/io.github.russellmilliner.LibreLinkUpDesktop.metainfo.xml`. Please do not file a public
issue for anything that could expose another person's glucose data.

This app is **not a medical device** and must not be relied on as one; the LibreView app and
the sensor's own alarms remain the primary safety mechanism.
