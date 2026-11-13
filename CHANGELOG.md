# Changelog — librelinkup-kde

All notable changes and bumped dependencies for `russellmilliner/librelinkup-rust-desktop` are documented in this file.

## [v0.5.14](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.14) — 2026-11-13

### Highlights
- Harden DPAPI 3-vault rotation guard, Home Assistant entity and cleartext HTTP checks, KWin D-Bus scriptable slot scoping, secret zeroization, and --ui-test CLI suite

### Commits
- fix(security): harden DPAPI vault guard, HA entity validation, KWin D-Bus slots, and test suite (21b1b99)

## [v0.5.13](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.13) — 2026-11-13

### Highlights
- Add KDE System Settings notification categories (.notifyrc) and customizable global shortcuts (KGlobalAccel, desktop actions, and single-instance D-Bus CLI flags)

### Commits
- feat(kde): add KDE System Settings notifications (.notifyrc) and KGlobalAccel global shortcuts (6f2f251)
- fix(clippy): resolve Rust 1.100.0 needless_bool lint in updater URL validation (69660cd)
- chore(deps): update dependencies (cc 1.11.2, naga 30.0.2, naga-types 30.0.2, rustls-webpki 0.103.17, wgpu 30.0.2, wgpu-core 30.0.2 (+10 more)) (01d099f)
- chore(deps): update dependencies (arc-swap 1.9.5, crc32fast 1.5.5, errno 0.3.15, rustls-pki-types 1.16.0, webbrowser 1.2.7, webpki-roots 1.0.12) (17af142)

### 📦 Bumped Dependencies (22 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `arc-swap` | `1.9.4` | **`1.9.5`** | Lockfile (`windows/Cargo.lock`) |
| `cc` | `1.11.1` | **`1.11.2`** | Lockfile (`windows/Cargo.lock`) |
| `crc32fast` | `1.5.4` | **`1.5.5`** | Lockfile (`windows/Cargo.lock`) |
| `errno` | `0.3.14` | **`0.3.15`** | Lockfile (`windows/Cargo.lock`) |
| `naga` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `naga-types` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `rustls-pki-types` | `1.15.1` | **`1.16.0`** | Lockfile (`windows/Cargo.lock`) |
| `rustls-webpki` | `0.103.16` | **`0.103.17`** | Lockfile (`windows/Cargo.lock`) |
| `webbrowser` | `1.2.6` | **`1.2.7`** | Lockfile (`windows/Cargo.lock`) |
| `webpki-roots` | `1.0.11` | **`1.0.12`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-core` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-core-deps-apple` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-core-deps-emscripten` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-core-deps-wasm` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-core-deps-windows-linux-android` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-hal` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-naga-bridge` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-types` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `zbus` | `6.2.1` | **`6.3.0`** | Lockfile (`windows/Cargo.lock`) |
| `zbus_macros` | `6.2.1` | **`6.3.0`** | Lockfile (`windows/Cargo.lock`) |
| `zruntime` | `0.2.2` | **`0.2.3`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.12-deps](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.12-deps) — 2026-11-13

### 📦 Bumped Dependencies (16 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `cc` | `1.11.1` | **`1.11.2`** | Lockfile (`windows/Cargo.lock`) |
| `naga` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `naga-types` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `rustls-webpki` | `0.103.16` | **`0.103.17`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-core` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-core-deps-apple` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-core-deps-emscripten` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-core-deps-wasm` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-core-deps-windows-linux-android` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-hal` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-naga-bridge` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `wgpu-types` | `30.0.1` | **`30.0.2`** | Lockfile (`windows/Cargo.lock`) |
| `zbus` | `6.2.1` | **`6.3.0`** | Lockfile (`windows/Cargo.lock`) |
| `zbus_macros` | `6.2.1` | **`6.3.0`** | Lockfile (`windows/Cargo.lock`) |
| `zruntime` | `0.2.2` | **`0.2.3`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.12](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.12) — 2026-11-10

### Highlights
- Fix Wayland desktop overlay KDE Task Manager icon exclusion and multi-monitor hide/restore placement, and add dedicated widget & taskbar regression test suites

### Commits
- fix(wayland,overlay): exclude desktop overlay from KDE Task Manager and restore screen/position across hide/restore (f655f22)

## [v0.5.11](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.11) — 2026-11-10

### Highlights
- Zero-churn resident KWin shortcut IPC (SyncOverlay, SyncMain, QueryOverlay, QueryMain) and Script.stop slot cleanup

### Commits
- chore(deps): update dependencies (notify-rust 4.19.0, arc-swap 1.9.4, itoa 1.0.19, syn 3.0.10, uuid 1.29.2) (782bfa6)

### 📦 Bumped Dependencies (7 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `notify-rust` | `4.18.2` | **`4.19.0`** | Direct (`Cargo.toml`) |
| `arc-swap` | `1.9.3` | **`1.9.4`** | Lockfile (`windows/Cargo.lock`) |
| `itoa` | `1.0.18` | **`1.0.19`** | Lockfile (`windows/Cargo.lock`) |
| `syn` | `3.0.9` | **`3.0.10`** | Lockfile (`windows/Cargo.lock`) |
| `uuid` | `1.29.1` | **`1.29.2`** | Lockfile (`windows/Cargo.lock`) |
| `zbus` | `5.19.0` | **`6.2.1`** | Lockfile (`windows/Cargo.lock`) |
| `zbus_macros` | `5.19.0` | **`6.2.1`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.10-deps](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.10-deps) — 2026-11-09

### 📦 Bumped Dependencies (7 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `notify-rust` | `4.18.2` | **`4.19.0`** | Direct (`Cargo.toml`) |
| `arc-swap` | `1.9.3` | **`1.9.4`** | Lockfile (`windows/Cargo.lock`) |
| `itoa` | `1.0.18` | **`1.0.19`** | Lockfile (`windows/Cargo.lock`) |
| `syn` | `3.0.9` | **`3.0.10`** | Lockfile (`windows/Cargo.lock`) |
| `uuid` | `1.29.1` | **`1.29.2`** | Lockfile (`windows/Cargo.lock`) |
| `zbus` | `5.19.0` | **`6.2.1`** | Lockfile (`windows/Cargo.lock`) |
| `zbus_macros` | `5.19.0` | **`6.2.1`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.10](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.10) — 2026-11-08

### Highlights
- Fix Wayland desktop overlay startup monitor/position restoration and compositor hover/drag lag, consolidate same-time medication due reminders into a single notification per slot, and add KDE Plasma 6 system tray out-of-range relevance mode (KStatusNotifierItem Passive when In Range)

### Commits
- feat(wayland,tray,meds): fix Wayland overlay startup placement, consolidate same-time med reminders & add KDE tray relevance mode (e2d43c5)

## [v0.5.9](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.9) — 2026-11-07

### Highlights
- Keep 0600 master.key synchronized with the desktop keyring (secret-tool / KWallet, including flatpak-spawn --host fallback) and guard phi_secret() against rotating keys when any encrypted vault exists on disk

### Commits
- fix(crypto): keep 0600 master.key synced with keyring across host/Flatpak launches & guard phi_secret() (e273255)
- chore(deps): update dependencies (smallvec 1.16.5, uuid 1.29.1) (763b719)

### 📦 Bumped Dependencies (2 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `smallvec` | `1.16.4` | **`1.16.5`** | Lockfile (`windows/Cargo.lock`) |
| `uuid` | `1.29.0` | **`1.29.1`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.8-deps](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.8-deps) — 2026-11-07

### 📦 Bumped Dependencies (2 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `smallvec` | `1.16.4` | **`1.16.5`** | Lockfile (`windows/Cargo.lock`) |
| `uuid` | `1.29.0` | **`1.29.1`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.8](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.8) — 2026-11-07

### Highlights
- Ensure Bazzite Flatpak installation via Bazaar/KDE Discover (embedded AppStream catalog & 64x64/128x128 icons, --system/--user scope detection, host-visible update staging) and enforce persistent resident KWin skipTaskbar/skipPager/skipSwitcher rules so the floating desktop overlay never shows an icon in the KDE Plasma Task Manager

### Commits
- fix(flatpak,wayland): ensure Bazzite Bazaar/Discover Flatpak install & hide overlay from KDE Task Manager (82c49f1)
- chore(deps): update dependencies (cc 1.11.1, flate2 1.1.11, serde_spanned 1.1.3, toml 1.2.1+spec-1.1.0, toml_datetime 1.1.3+spec-1.1.0, toml_edit 0.25.19+spec-1.1.0) (d665633)
- chore(deps): update dependencies (winnow 1.0.5) (2441a75)
- chore(deps): update dependencies (toml 1.2.0+spec-1.1.0, toml_edit 0.25.18+spec-1.1.0, toml_writer 1.2.0+spec-1.1.0) (5b594fe)

### 📦 Bumped Dependencies (8 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `cc` | `1.11.0` | **`1.11.1`** | Lockfile (`windows/Cargo.lock`) |
| `flate2` | `1.1.10` | **`1.1.11`** | Lockfile (`windows/Cargo.lock`) |
| `serde_spanned` | `1.1.2` | **`1.1.3`** | Lockfile (`windows/Cargo.lock`) |
| `toml` | `1.1.8+spec-1.1.0` | **`1.2.1+spec-1.1.0`** | Lockfile (`windows/Cargo.lock`) |
| `toml_datetime` | `1.1.2+spec-1.1.0` | **`1.1.3+spec-1.1.0`** | Lockfile (`windows/Cargo.lock`) |
| `toml_edit` | `0.25.17+spec-1.1.0` | **`0.25.19+spec-1.1.0`** | Lockfile (`windows/Cargo.lock`) |
| `toml_writer` | `1.1.3+spec-1.1.0` | **`1.2.0+spec-1.1.0`** | Lockfile (`windows/Cargo.lock`) |
| `winnow` | `1.0.4` | **`1.0.5`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.7-deps](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.7-deps) — 2026-11-06

### 📦 Bumped Dependencies (6 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `cc` | `1.11.0` | **`1.11.1`** | Lockfile (`windows/Cargo.lock`) |
| `flate2` | `1.1.10` | **`1.1.11`** | Lockfile (`windows/Cargo.lock`) |
| `serde_spanned` | `1.1.2` | **`1.1.3`** | Lockfile (`windows/Cargo.lock`) |
| `toml` | `1.2.0+spec-1.1.0` | **`1.2.1+spec-1.1.0`** | Lockfile (`windows/Cargo.lock`) |
| `toml_datetime` | `1.1.2+spec-1.1.0` | **`1.1.3+spec-1.1.0`** | Lockfile (`windows/Cargo.lock`) |
| `toml_edit` | `0.25.18+spec-1.1.0` | **`0.25.19+spec-1.1.0`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.7](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.7) — 2026-11-03

### Highlights
- Scale the entire desktop overlay widget (background, border, text, trend badge, and action buttons) uniformly via FBO layer opacity and ui.set_opacity() when adjusting overlay transparency

### Commits
- fix(overlay): apply overlayOpacityPercent to entire overlay widget layer (cdd7734)

## [v0.5.6](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.6) — 2026-11-03

### Highlights
- Update dependencies: async-compression 0.4.52, async-executor 1.14.1, async-lock 3.5.0, cc 1.11.0, compression-codecs 0.4.47, crc32fast 1.5.4

### 📦 Bumped Dependencies (6 packages)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `async-compression` | `0.4.51` | **`0.4.52`** | Lockfile (`windows/Cargo.lock`) |
| `async-executor` | `1.14.0` | **`1.14.1`** | Lockfile (`windows/Cargo.lock`) |
| `async-lock` | `3.4.2` | **`3.5.0`** | Lockfile (`windows/Cargo.lock`) |
| `cc` | `1.10.0` | **`1.11.0`** | Lockfile (`windows/Cargo.lock`) |
| `compression-codecs` | `0.4.46` | **`0.4.47`** | Lockfile (`windows/Cargo.lock`) |
| `crc32fast` | `1.5.3` | **`1.5.4`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.5](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.5) — 2026-11-01

### Highlights
- Run natively on Wayland via KWin D-Bus scripting + startSystemMove, harden frameless desktop overlay against KWin Breeze decorations, and always render the blood drop logo in the system tray glucose badge icon

### Commits
- feat(wayland,tray): run natively on Wayland via KWin scripting and keep blood drop logo in tray badge (03cbd67)
- chore(deps): update dependencies (unicode-properties 0.1.5) (42ccd3e)

### 📦 Bumped Dependencies (1 package)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `unicode-properties` | `0.1.4` | **`0.1.5`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.4-deps](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.4-deps) — 2026-11-01

### 📦 Bumped Dependencies (1 package)

| Package | Previous | Updated | Scope |
| :--- | :--- | :--- | :--- |
| `unicode-properties` | `0.1.4` | **`0.1.5`** | Lockfile (`windows/Cargo.lock`) |

## [v0.5.4](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.4) — 2026-10-31

### Highlights
- Render desktop overlay tooltips in an external top-level window so they never get cropped by the 276x88 overlay bounds or cover the overlay buttons

### Commits
- docs(architecture): document external overlay tooltip window and tray/tick invariants (6489137)
- fix(overlay): render overlay button tooltips in external top-level Window so they never crop or block buttons (2e39d23)

## [v0.5.3](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.3) — 2026-10-31

### Highlights
- Fix infinite overlayOnlyMode event loop and 100% CPU lockup when main window is closed with desktop overlay visible; eliminate per-tick disk reads and redundant JSON/D-Bus updates

### Commits
- fix(tray,overlay): eliminate infinite overlayOnlyMode queued-event loop and per-tick disk/JSON overhead (f612f2f)

## [v0.5.2](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.5.2) — 2026-10-31

### Highlights
- Release v0.5.2 of librelinkup-kde.

### Commits
- feat(ha,ui,docs): add Home Assistant multi-patient selector, status bar connection badge, updated screenshots & documentation (79af1b6)

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
