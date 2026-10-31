# Changelog — librelinkup-kde

All notable changes and bumped dependencies for `russellmilliner/librelinkup-rust-desktop` are documented in this file.

## [v0.5.1](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.1) — 2026-10-31

### Highlights
- Ploopy hi-res wheel scrolling, wide high-contrast scrollbars across all tabs, primary-screen centering & window placement persistence, header overflow fix on tray restore, and Medications documentation

### Commits
- fix(ui): use Flickable with right-anchored HighContrastScrollBar so scrollbars are always visible when scrollable (4e9533e)
- fix(ui): add Ploopy hi-res wheel scroll filter and wide HighContrastScrollBar across all views (85ead55)
- fix(windows): disable tray-icon default libappindicator/gtk3 feature to drop unsound glib 0.18.5 (RUSTSEC-2024-0429) (9ef3606)
- chore(deps): update dependencies (encoding_rs 0.8.44, js-sys 0.3.107, log 0.4.35, ron 0.12.3, rustix 1.1.6, unicode-width 0.2.3 (+9 more)) (f00233e)
- fix(ui): prevent minimize-to-tray header overflow on restore and center window on primary screen when no saved position exists (f72ebd2)
- docs: add Medications tab user guide, zero-tracking/zero-ads guarantee, and updated screenshots (5838679)
- chore(flathub): pin manifest to v0.5.0 (b67740eac93d6fed8696e3522d43c1af3eab9e61) (ddfd643)

### 📦 Bumped Dependencies (15 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `encoding_rs` | `0.8.42` | **`0.8.44`** | Lockfile (`windows/Cargo.lock`) |
| `js-sys` | `0.3.106` | **`0.3.107`** | Lockfile (`windows/Cargo.lock`) |
| `log` | `0.4.34` | **`0.4.35`** | Lockfile (`windows/Cargo.lock`) |
| `ron` | `0.12.2` | **`0.12.3`** | Lockfile (`windows/Cargo.lock`) |
| `rustix` | `1.1.5` | **`1.1.6`** | Lockfile (`windows/Cargo.lock`) |
| `unicode-width` | `0.2.2` | **`0.2.3`** | Lockfile (`windows/Cargo.lock`) |
| `utf8_iter` | `1.0.4` | **`1.1.0`** | Lockfile (`windows/Cargo.lock`) |
| `wasm-bindgen` | `0.2.129` | **`0.2.130`** | Lockfile (`windows/Cargo.lock`) |
| `wasm-bindgen-futures` | `0.4.79` | **`0.4.80`** | Lockfile (`windows/Cargo.lock`) |
| `wasm-bindgen-macro` | `0.2.129` | **`0.2.130`** | Lockfile (`windows/Cargo.lock`) |
| `wasm-bindgen-macro-support` | `0.2.129` | **`0.2.130`** | Lockfile (`windows/Cargo.lock`) |
| `wasm-bindgen-shared` | `0.2.129` | **`0.2.130`** | Lockfile (`windows/Cargo.lock`) |
| `web-sys` | `0.3.106` | **`0.3.107`** | Lockfile (`windows/Cargo.lock`) |
| `webbrowser` | `1.2.5` | **`1.2.6`** | Lockfile (`windows/Cargo.lock`) |
| `zcheapstr` | `1.1.0` | **`1.2.0`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.0-deps](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.0-deps) — 2026-10-31

### 📦 Bumped Dependencies (15 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `encoding_rs` | `0.8.42` | **`0.8.44`** | Lockfile (`windows/Cargo.lock`) |
| `js-sys` | `0.3.106` | **`0.3.107`** | Lockfile (`windows/Cargo.lock`) |
| `log` | `0.4.34` | **`0.4.35`** | Lockfile (`windows/Cargo.lock`) |
| `ron` | `0.12.2` | **`0.12.3`** | Lockfile (`windows/Cargo.lock`) |
| `rustix` | `1.1.5` | **`1.1.6`** | Lockfile (`windows/Cargo.lock`) |
| `unicode-width` | `0.2.2` | **`0.2.3`** | Lockfile (`windows/Cargo.lock`) |
| `utf8_iter` | `1.0.4` | **`1.1.0`** | Lockfile (`windows/Cargo.lock`) |
| `wasm-bindgen` | `0.2.129` | **`0.2.130`** | Lockfile (`windows/Cargo.lock`) |
| `wasm-bindgen-futures` | `0.4.79` | **`0.4.80`** | Lockfile (`windows/Cargo.lock`) |
| `wasm-bindgen-macro` | `0.2.129` | **`0.2.130`** | Lockfile (`windows/Cargo.lock`) |
| `wasm-bindgen-macro-support` | `0.2.129` | **`0.2.130`** | Lockfile (`windows/Cargo.lock`) |
| `wasm-bindgen-shared` | `0.2.129` | **`0.2.130`** | Lockfile (`windows/Cargo.lock`) |
| `web-sys` | `0.3.106` | **`0.3.107`** | Lockfile (`windows/Cargo.lock`) |
| `webbrowser` | `1.2.5` | **`1.2.6`** | Lockfile (`windows/Cargo.lock`) |
| `zcheapstr` | `1.1.0` | **`1.2.0`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.0](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.0) — 2026-10-29

### Highlights
- Security hardening release: signed auto-updates (minisign), encryption at rest for glucose history and medication data, fail-closed staleness detection, re-auth backoff, and a three-controller architecture split.

## [v0.4.5](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.4.5) — 2026-10-28

### Changes
- feat(security): notification snooze actions, SHA-256 updater verification, keyring/opt-in vault, region allowlist, PII redaction, and modular core/bridge/cpp split (e0f8b4e)
- chore(deps): update dependencies (accesskit 0.25.2, accesskit_atspi_common 0.21.1, accesskit_consumer 0.39.2, accesskit_ios 0.2.2, accesskit_macos 0.27.2, accesskit_unix 0.24.1 (+3 more)) (3de3d00)
- chore(deps): update dependencies (cc 1.10.0, clipboard-win 5.4.3, smallvec 1.16.4, uuid 1.29.0, webpki-roots 1.0.11) (e06d46e)

### 📦 Bumped Dependencies (14 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `accesskit` | `0.25.1` | **`0.25.2`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_atspi_common` | `0.21.0` | **`0.21.1`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_consumer` | `0.39.1` | **`0.39.2`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_ios` | `0.2.1` | **`0.2.2`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_macos` | `0.27.1` | **`0.27.2`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_unix` | `0.24.0` | **`0.24.1`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_windows` | `0.35.1` | **`0.35.2`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_winit` | `0.34.1` | **`0.34.2`** | Lockfile (`windows/Cargo.lock`) |
| `cc` | `1.9.0` | **`1.10.0`** | Lockfile (`windows/Cargo.lock`) |
| `clipboard-win` | `5.4.1` | **`5.4.3`** | Lockfile (`windows/Cargo.lock`) |
| `ordered-float` | `5.5.0` | **`5.5.1`** | Lockfile (`windows/Cargo.lock`) |
| `smallvec` | `1.16.3` | **`1.16.4`** | Lockfile (`windows/Cargo.lock`) |
| `uuid` | `1.28.0` | **`1.29.0`** | Lockfile (`windows/Cargo.lock`) |
| `webpki-roots` | `1.0.10` | **`1.0.11`** | Lockfile (`windows/Cargo.lock`) |

## [v0.4.4-deps](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.4.4-deps) — 2026-10-26

### 📦 Bumped Dependencies (9 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `accesskit` | `0.25.1` | **`0.25.2`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_atspi_common` | `0.21.0` | **`0.21.1`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_consumer` | `0.39.1` | **`0.39.2`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_ios` | `0.2.1` | **`0.2.2`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_macos` | `0.27.1` | **`0.27.2`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_unix` | `0.24.0` | **`0.24.1`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_windows` | `0.35.1` | **`0.35.2`** | Lockfile (`windows/Cargo.lock`) |
| `accesskit_winit` | `0.34.1` | **`0.34.2`** | Lockfile (`windows/Cargo.lock`) |
| `ordered-float` | `5.5.0` | **`5.5.1`** | Lockfile (`windows/Cargo.lock`) |

## [v0.4.4](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.4.4) — 2026-10-23

## [v0.4.3](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.4.3) — 2026-10-22

### Changes
- docs(changelog): add v0.4.2 dependency changelog (2631c0a)

## [v0.4.2](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.4.2) — 2026-10-22

### 📦 Bumped Dependencies (23 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `tokio` | `1.54.0` | **`1.54.1`** | Direct (`Cargo.toml`) |
| `async-compression` | `0.4.41` | **`0.4.51`** | Lockfile (`Cargo.lock`) |
| `cc` | `1.8.0` | **`1.9.0`** | Lockfile (`Cargo.lock`) |
| `compression-codecs` | `0.4.37` | **`0.4.46`** | Lockfile (`Cargo.lock`) |
| `compression-core` | `0.4.31` | **`0.4.34`** | Lockfile (`Cargo.lock`) |
| `crc32fast` | `1.5.2` | **`1.5.3`** | Lockfile (`windows/Cargo.lock`) |
| `find-msvc-tools` | `0.1.14` | **`0.1.15`** | Lockfile (`Cargo.lock`) |
| `icu_collections` | `2.3.0` | **`2.4.0`** | Lockfile (`windows/Cargo.lock`) |
| `icu_locale_core` | `2.3.0` | **`2.4.0`** | Lockfile (`windows/Cargo.lock`) |
| `icu_normalizer` | `2.3.0` | **`2.4.0`** | Lockfile (`windows/Cargo.lock`) |
| `icu_normalizer_data` | `2.3.0` | **`2.4.0`** | Lockfile (`windows/Cargo.lock`) |
| `icu_properties` | `2.3.0` | **`2.4.0`** | Lockfile (`windows/Cargo.lock`) |
| `icu_properties_data` | `2.3.0` | **`2.4.0`** | Lockfile (`windows/Cargo.lock`) |
| `icu_provider` | `2.3.1` | **`2.4.0`** | Lockfile (`windows/Cargo.lock`) |
| `memmap2` | `0.9.11` | **`0.9.12`** | Lockfile (`windows/Cargo.lock`) |
| `rustls-webpki` | `0.103.15` | **`0.103.16`** | Lockfile (`Cargo.lock`) |
| `tinystr` | `0.8.4` | **`0.8.5`** | Lockfile (`windows/Cargo.lock`) |
| `tokio-util` | `0.7.18` | **`0.7.20`** | Lockfile (`Cargo.lock`) |
| `webbrowser` | `1.2.4` | **`1.2.5`** | Lockfile (`windows/Cargo.lock`) |
| `webpki-roots` | `1.0.9` | **`1.0.10`** | Lockfile (`Cargo.lock`) |
| `yoke` | `0.8.3` | **`0.8.4`** | Lockfile (`windows/Cargo.lock`) |
| `zerovec` | `0.11.8` | **`0.11.9`** | Lockfile (`windows/Cargo.lock`) |
| `zerovec-derive` | `0.11.6` | **`0.11.7`** | Lockfile (`windows/Cargo.lock`) |
