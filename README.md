# LibreLinkUp Rust Desktop

[![Ko-fi](https://img.shields.io/badge/Ko--fi-Buy%20Me%20a%20Coffee-ff5e5b?logo=ko-fi&logoColor=white&style=for-the-badge)](https://ko-fi.com/russellmilliner)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg?style=for-the-badge)](https://www.apache.org/licenses/LICENSE-2.0)

A native **Rust + Qt 6 / KDE Plasma 6 Kirigami** desktop application for monitoring continuous blood glucose readings from **LibreLinkUp**, based on and inspired by [LibreLinkUpDesktop](https://github.com/Crazy-Marvin/LibreLinkUpDesktop).

---

## Key Features

- **Native KDE Plasma 6 UI**: Built with `cxx-qt`, Qt 6 Quick Controls (`org.kde.desktop`), `org.kde.kirigami`, and `KColorScheme` (`KF6ColorScheme`).
- **Light, Dark & System Theme Support**: Dynamically switches between Breeze Light, Breeze Dark, or your system's KDE color scheme at runtime via `KColorSchemeManager`.
- **Single System Tray Icon & True Start-Minimized**:
  - Uses KDE's native `StatusNotifierWatcher` protocol via `QSystemTrayIcon`.
  - Hides the window cleanly when minimized or closed to tray so you never get duplicate taskbar + tray icons.
  - Supports `--minimized` / `--autostart` / `START_MINIMIZED=1` to launch silently in the tray on login.
- **Tray Glucose Badge & Hover Tooltip**:
  - **Show Glucose Level in Tray Icon**: Dynamically renders a high-contrast color-coded glucose badge directly on the system tray icon.
  - **Show Glucose Details in Tray Hover Tooltip**: Displays patient name, current reading (`mg/dL` or `mmol/L`), trend arrow, range status, and last update timestamp when hovering the tray icon.
- **12-Hour Interactive Glucose Graph**:
  - Interactive Canvas chart with target range bands and point hover inspection (displaying exact reading, status badge, and timestamp when hovering over any graph point).
- **Floating Desktop Overlay Widget (with Dark Theme & Transparency)**:
  - Borderless always-on-top desktop widget showing live blood sugar, trend arrow, and range status.
  - **Overlay Dark Theme**: Switch between a sleek dark slate card with color-coded status accents (`#0f172a`) or a classic full-status-color card—either from Settings, the Dashboard quick toggle, or directly on the overlay widget.
  - **Adjustable Overlay Transparency**: Configure opacity from `20%` to `100%` in Settings or by scrolling the mouse wheel directly over the floating overlay.
- **AES-256-GCM Encrypted Credential Vault**:
  - All stored credentials and session tokens are encrypted at rest in `~/.local/share/librelinkup-kde/credentials.enc` using **AES-256-GCM** with a 100,000-iteration **PBKDF2-HMAC-SHA256** key derived from machine identity (`ring`) and strict `0600` permissions.
  - Sensitive strings are wiped from memory on drop via `zeroize`.
  - Automatically migrates active tokens from the legacy Electron Flatpak (`rocks.poopjournal.librelinkupdesktop`) on first launch.
- **Diagnostic Log Viewer**:
  - Built-in Log Viewer dialog with category filtering (`AUTH`, `API`, `CRYPTO`, `TRAY`, `ALERT`, `APP`), minimum severity filtering (`DEBUG`, `INFO`, `WARN`, `ERROR`), real-time search, clipboard copy, and `.log` file export.

---

## Attribution & License

- **Author**: [Russell Milliner](https://github.com/russellmilliner)
- **Repository**: [https://github.com/russellmilliner/librelinkup-rust-desktop](https://github.com/russellmilliner/librelinkup-rust-desktop)
- **License**: Licensed under the [Apache License, Version 2.0](LICENSE) (see [NOTICE](NOTICE)).
- **Upstream Attribution**: This application is a native Rust + KDE Plasma 6 rewrite based on and inspired by [LibreLinkUpDesktop](https://github.com/Crazy-Marvin/LibreLinkUpDesktop) by [Crazy-Marvin](https://github.com/Crazy-Marvin) and contributors (licensed under the [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0)). Tray icon vector assets in `src/qml/icons/` originate from `LibreLinkUpDesktop`.

---

## Support the Project

If you find **LibreLinkUp Rust Desktop** helpful, consider buying me a coffee on Ko-fi:

[![Buy Me a Coffee on Ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/russellmilliner)

- **Ko-fi**: [https://ko-fi.com/russellmilliner](https://ko-fi.com/russellmilliner)
- **GitHub**: [https://github.com/russellmilliner/librelinkup-rust-desktop](https://github.com/russellmilliner/librelinkup-rust-desktop)

---

## Building & Testing

```bash
# Run the unit, integration, and headless QML UI layout test suites
cargo test -- --test-threads=1

# Build release binary
cargo build --release

# Run normally or minimized to tray
./target/release/librelinkup-kde
./target/release/librelinkup-kde --minimized
```

## Building & Installing the Flatpak

```bash
chmod +x scripts/build-flatpak.sh
./scripts/build-flatpak.sh

# Install the generated Flatpak bundle
flatpak install --user -y target/release/librelinkup-kde.flatpak
```
