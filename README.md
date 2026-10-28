# LibreLinkUp Rust Desktop

[![Version v0.4.4](https://img.shields.io/badge/Version-v0.4.4-0284c7?style=for-the-badge&logo=github)](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.4.4)
[![Windows Installer](https://img.shields.io/badge/Windows-Installer_v0.4.4-0078d4?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-Setup-0.4.4-x64.exe)
[![Flatpak Bundle](https://img.shields.io/badge/Flatpak-Download_v0.4.4-4a90d9?style=for-the-badge&logo=flatpak&logoColor=white)](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/librelinkup-rust-desktop-v0.4.4-x86_64.flatpak)
[![Platform: Linux x86_64](https://img.shields.io/badge/Platform-Linux%20x86__64-fcc624?style=for-the-badge&logo=linux&logoColor=black)](https://www.kernel.org/)
[![Platform: Windows 10/11 x64](https://img.shields.io/badge/Platform-Windows%2010%2F11%20x64-0078d4?style=for-the-badge&logo=windows&logoColor=white)](windows/README.md)
[![Desktop: KDE Plasma 6](https://img.shields.io/badge/KDE-Plasma%206%20%2F%20Kirigami-1d99f3?style=for-the-badge&logo=kde&logoColor=white)](https://kde.org/plasma-desktop/)
[![Built with Rust](https://img.shields.io/badge/Rust-2021_Edition-dea584?style=for-the-badge&logo=rust&logoColor=black)](https://www.rust-lang.org/)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg?style=for-the-badge)](https://www.apache.org/licenses/LICENSE-2.0)
[![Ko-fi](https://img.shields.io/badge/Ko--fi-Buy%20Me%20a%20Coffee-ff5e5b?logo=ko-fi&logoColor=white&style=for-the-badge)](https://ko-fi.com/russellmilliner)

Native **Rust** desktop applications for monitoring continuous blood glucose readings from **LibreLinkUp** on **Linux (KDE Plasma 6 / Kirigami / Qt 6)** and **Windows 10/11 (`egui` / `eframe`)**, built on a shared `librelinkup-core` crate and inspired by [LibreLinkUpDesktop](https://github.com/Crazy-Marvin/LibreLinkUpDesktop).

---

## Release, Flatpak & Windows Links

| Item | Link / Details |
| :--- | :--- |
| **Current Version** | [**v0.4.4** (GitHub Release)](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/v0.4.4) • [All Releases](https://github.com/russellmilliner/librelinkup-rust-desktop/releases) |
| **Windows Installer (`x64`)** | [**`LibreLinkUp-Setup-0.4.4-x64.exe`**](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-Setup-0.4.4-x64.exe) ([SHA-256](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-Setup-0.4.4-x64.exe.sha256)) |
| **Windows Portable (`x64`)** | [**`LibreLinkUp-0.4.4-windows-x64-portable.zip`**](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-0.4.4-windows-x64-portable.zip) ([SHA-256](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-0.4.4-windows-x64-portable.zip.sha256)) |
| **Windows Documentation** | [**`windows/README.md`**](windows/README.md) — Full Windows installation, overlay controls, DPAPI security & build guide |
| **Linux Flatpak Bundle (`x86_64`)** | [**`librelinkup-rust-desktop-v0.4.4-x86_64.flatpak`**](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/librelinkup-rust-desktop-v0.4.4-x86_64.flatpak) ([SHA-256](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/librelinkup-rust-desktop-v0.4.4-x86_64.flatpak.sha256)) |
| **Flatpak Runtime** | [`org.kde.Platform//6.11`](https://flathub.org/) via [Flatpak](https://flatpak.org/) |
| **Supported Platforms** | [Linux (`x86_64`)](https://www.kernel.org/) ([KDE Plasma 6](https://kde.org/plasma-desktop/) / [Kirigami](https://develop.kde.org/frameworks/kirigami/) / [Qt 6](https://www.qt.io/)) • [**Windows 10/11 (`x64`)**](windows/README.md) ([egui](https://github.com/emilk/egui) / [eframe](https://github.com/emilk/egui/tree/master/crates/eframe)) |
| **Core Stack** | [Rust](https://www.rust-lang.org/) (`librelinkup-core`) + [KDAB CXX-Qt](https://github.com/KDAB/cxx-qt) (Linux) + [eframe/egui](https://github.com/emilk/egui) (Windows) + [Ring (AES-256-GCM)](https://github.com/briansmith/ring) |

### Quick Flatpak Install (Linux)

```bash
curl -LO https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/librelinkup-rust-desktop-v0.4.4-x86_64.flatpak
flatpak install --user -y librelinkup-rust-desktop-v0.4.4-x86_64.flatpak
flatpak run io.github.russellmilliner.LibreLinkUpDesktop
```

### Quick Windows 10 / 11 Install

Download and run [**`LibreLinkUp-Setup-0.4.4-x64.exe`**](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-Setup-0.4.4-x64.exe) (or extract the [**portable zip**](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-0.4.4-windows-x64-portable.zip)):

```powershell
Invoke-WebRequest -Uri "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.4.4/LibreLinkUp-Setup-0.4.4-x64.exe" -OutFile "$env:TEMP\LibreLinkUp-Setup-0.4.4-x64.exe"
Start-Process -FilePath "$env:TEMP\LibreLinkUp-Setup-0.4.4-x64.exe"
```

- **Per-user install**: No administrator prompt is required. Creates a Start Menu shortcut (with Toast notification `AppUserModelID`) and an optional task to launch at Windows sign-in (`--minimized` to the system tray).
- **SmartScreen note**: The installer is not signed with a commercial EV certificate yet. If Windows SmartScreen displays *"Windows protected your PC"*, click **More info → Run anyway**.
- **Feature parity with KDE**: Live dashboard and 12-hour interactive graph, dynamic 32×32 glucose badge in the Windows notification area, floating always-on-top desktop overlay with **scroll-wheel transparency (`20%`–`100%`)** and **5 on-overlay action buttons (`🌙`/`🎨`, `⊞` On All Desktops, `↗`, `☰`, `✕`)**, stale-reading detection, urgent-low/repeat/snooze alerts with built-in alarm audio (`rodio`) and Windows Toast notifications.
- **Windows DPAPI security**: Credentials are encrypted with **AES-256-GCM** (`%APPDATA%\LibreLinkUpDesktop\credentials.enc.json`), and the 256-bit master key (`master.dpapi`) is protected by **Windows DPAPI (`CryptProtectData`)** bound to your Windows user login. Diagnostic logs are written to `%LOCALAPPDATA%\LibreLinkUpDesktop\events.log`.
- See [**`windows/README.md`**](windows/README.md) for the complete Windows user and developer guide.

---

## Screenshots

### Main Application — Live Glucose Dashboard & 12-Hour Interactive Graph
![Main Application Dashboard](docs/screenshots/app-dashboard.png)

### General, System Tray & Desktop Overlay Settings
![General & Overlay Settings](docs/screenshots/app-settings.png)

### Floating Desktop Overlay Widget (Dark Slate & Status Color Themes)
![Desktop Overlay Widget](docs/screenshots/overlay-widget.png)

### System Tray Icons (Default SVG Drop & Live Glucose Value Badges)
![System Tray Icons](docs/screenshots/tray-icons.png)

---

## Key Features

- **Cross-Platform Native Desktop UIs on a Shared Rust Core**:
  - **Linux (KDE Plasma 6)**: Built with `cxx-qt`, Qt 6 Quick Controls (`org.kde.desktop`), `org.kde.kirigami`, and `KColorScheme` (`KF6ColorScheme`).
  - **Windows 10 / 11 (`x64`)**: Built in pure Rust with `eframe` / `egui` (`wgpu` hardware acceleration with automatic `glow` OpenGL fallback), `tray-icon`, `rodio`, and `windows-sys`.
  - **Shared Core (`crates/librelinkup-core`)**: Unified LibreView API client (12 allowlisted regional endpoints + automatic validated regional redirect), shared session/polling state machine (`session.rs`), AES-256-GCM credential vault envelope (`crypto.rs`), settings model (`settings.rs`), staleness evaluator (`staleness.rs`), alert state machine (`alerts.rs`), and synthesized alarm generator (`audio.rs`).
- **Light, Dark & System Theme Support**:
  - Dynamically switches between Light, Dark, or your system's color scheme at runtime (`KColorSchemeManager` on KDE Plasma; `egui::Visuals` + dark/light Windows registry detection on Windows).
- **Single System Tray Icon & Task-Manager-Free Minimized Mode**:
  - Uses KDE's `StatusNotifierWatcher` on Linux and the Windows notification area (`tray-icon`) on Windows.
  - Hides the main window cleanly when minimized or closed to tray so no Task Manager / Taskbar entry remains when only the floating overlay or tray icon is active.
  - Supports `--minimized` / `--autostart` / `START_MINIMIZED=1` to launch silently in the tray at sign-in (via the XDG Background portal / autostart desktop entry on Linux, and `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` on Windows).
- **Tray Glucose Badge & Hover Tooltip**:
  - **Show Glucose Level in Tray Icon**: Dynamically renders a high-contrast color-coded glucose badge directly on the system tray icon.
  - **Show Glucose Details in Tray Hover Tooltip**: Displays patient name, current reading (`mg/dL` or `mmol/L`), trend arrow, range status, and reading age when hovering the tray icon.
- **12-Hour Interactive Glucose Graph**:
  - Interactive chart with target range bands and point hover inspection (displaying exact reading, status badge, and timestamp when hovering over any graph point).
- **Floating Desktop Overlay Widget (Control Buttons, Scroll-Wheel Opacity & Persistent Placement)**:
  - Borderless always-on-top desktop utility widget showing live blood sugar, vector trend arrow, and range status without cluttering the taskbar.
  - **On-Overlay Action Buttons (2-Row Cluster)**:
    - **`🌙` / `🎨` Theme Toggle**: Switch between the Dark Slate card (`#0f172a` with status-color accent bar) and full Status Color fill directly on the overlay.
    - **`⊞` On All Desktops Toggle (`onAllDesktops`)**: Toggle whether the floating overlay is shown across all virtual desktops (highlighted in green when active) or pinned only to the current virtual desktop (also configurable via Settings, the overlay `☰` menu, and the System Tray menu).
    - **`↗` Open Main Window**: Restore and focus the main desktop window.
    - **`☰` Context Menu**: Open the quick actions menu (`Show main window`, `Refresh now`, `Switch theme`, `Show on all virtual desktops`, `Hide overlay`) even when running in overlay-only mode.
    - **`✕` Hide Overlay**: Hide the floating overlay with one click.
  - **Scroll-Wheel Transparency Control**: Adjust opacity from **`20%`** to **`100%`** in **`5%`** steps by scrolling the mouse wheel directly over the floating overlay (or via the slider in Settings).
  - **Persistent Placement & State**: Automatically saves and restores the overlay's screen coordinates, dimensions, theme, opacity, virtual-desktop visibility (`overlay_on_all_desktops`), and visibility across restarts.
- **Stale-Reading Detection & Multi-Tier Alert Engine**:
  - **Stale-Reading Detection**: Readings display their age (`"3 min ago"`) and turn gray (`STALE`) when older than a configurable threshold (`5`–`60` min, default `15` min), with an optional signal-loss alert.
  - **Urgent-Low, Low, High & Stale Alerts**: Urgent-Low alerts (default `55 mg/dL`) **cannot be snoozed** and repeat every 5 minutes; Low, High, and Stale alerts follow a configurable repeat interval (`5`–`120` min) and fire immediately if the condition worsens.
  - **Alert Snooze**: Snooze non-urgent alerts for `30 min`, `1 hour`, or `2 hours` directly from interactive buttons on desktop alert notifications (`Snooze 30 min`, `Snooze 1 hr`, `Snooze 2 hr`), the Dashboard, the Alerts page, or the System Tray menu.
  - **Audible Alarm & Desktop Notifications**: Built-in synthesized two-tone alarm sound (plus custom `.wav`/`.mp3`/`.ogg` file support) paired with native desktop notifications (`org.freedesktop.Notifications` on Linux and WinRT Toast notifications on Windows) featuring interactive snooze actions.
- **AES-256-GCM Encrypted Credential Vault & Desktop Keyring Integration**:
  - **Linux**: Stored in `$XDG_CONFIG_HOME/librelinkup-kde/credentials.enc.json` using **AES-256-GCM** (`ring`) with atomic `0600` file writes (`write_file_atomic_0600`). The 256-bit vault master secret is stored in the desktop keyring (**`org.freedesktop.secrets` / Secret Service** via `secret-tool` or **KDE KWallet** via `kwallet-query`) with automatic fallback to a random `0600` keyfile (`$XDG_DATA_HOME/librelinkup-kde/vault.key`). Automatically migrates active tokens from the legacy Electron Flatpak (`rocks.poopjournal.librelinkupdesktop`) on first launch.
  - **Windows**: Stored in `%APPDATA%\LibreLinkUpDesktop\credentials.enc.json` using **AES-256-GCM**, with the 256-bit master key (`master.dpapi`) protected by **Windows DPAPI (`CryptProtectData`)** bound to the current Windows user account.
  - **Opt-In Password Storage**: By default (`remember_password = false`), only the encrypted LibreView session JWT and account metadata are persisted on disk; users can optionally opt in to **Remember password for automatic re-authentication** on the Login page.
  - **Memory & Log Redaction**: Sensitive strings are wiped from memory on drop via `zeroize`, custom `Debug` implementations redact secrets and health data, and email addresses are masked (`mask_email`) in diagnostic logs.
- **Diagnostic Log Viewer**:
  - Built-in Log Viewer with category filtering (`AUTH`, `API`, `CRYPTO`, `TRAY`, `ALERT`, `APP`), minimum severity filtering (`DEBUG`, `INFO`, `WARN`, `ERROR`), real-time search, clipboard copy, and `.log` file export.
- **SHA-256 Verified Built-in Update Checker & Installer**:
  - Automatically checks GitHub Releases on startup (configurable via the **Check for updates on startup** toggle in Settings) and provides a one-click **Check for Updates** / **Install Update** workflow in the Settings page, About dialog, System Tray menu (Windows), and CLI (`--check-update` / `-u`).
  - Enforces strict HTTPS host and repository owner allowlisting (`russellmilliner/librelinkup-rust-desktop`), downloads the companion `.sha256` checksum asset, and verifies the cryptographic **SHA-256 digest** in a private application data staging directory before launching the Windows Inno Setup installer or running sandbox-safe `flatpak-spawn --host flatpak install --user -y --reinstall` on Linux.

---

## Attribution & License

- **Author**: [Russell Milliner](https://github.com/russellmilliner)
- **Repository**: [https://github.com/russellmilliner/librelinkup-rust-desktop](https://github.com/russellmilliner/librelinkup-rust-desktop)
- **License**: Licensed under the [Apache License, Version 2.0](LICENSE) (see [NOTICE](NOTICE)).
- **Upstream Attribution**: This application is a native Rust rewrite (KDE Plasma 6 / Kirigami on Linux and `egui` / `eframe` on Windows) based on and inspired by [LibreLinkUpDesktop](https://github.com/Crazy-Marvin/LibreLinkUpDesktop) by [Crazy-Marvin](https://github.com/Crazy-Marvin) and contributors (licensed under the [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0)). Tray icon vector assets in `src/qml/icons/` originate from `LibreLinkUpDesktop`.

---

## Support the Project

If you find **LibreLinkUp Rust Desktop** helpful, consider buying me a coffee on Ko-fi:

[![Buy Me a Coffee on Ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/russellmilliner)

- **Ko-fi**: [https://ko-fi.com/russellmilliner](https://ko-fi.com/russellmilliner)
- **GitHub**: [https://github.com/russellmilliner/librelinkup-rust-desktop](https://github.com/russellmilliner/librelinkup-rust-desktop)

---

## Project Layout

| Path | Contents |
| :--- | :--- |
| [`crates/librelinkup-core/`](crates/librelinkup-core) | Platform-independent core shared by both apps: LibreView API client (`api.rs`), shared session/polling & alert orchestrator (`session.rs`), AES-256-GCM encrypted credential envelope & atomic `0600` file I/O (`crypto.rs`), settings (`settings.rs`), staleness evaluator (`staleness.rs`), alert state machine (`alerts.rs`), and alarm WAV generator (`audio.rs`) |
| `src/bridge/` | Modular Linux CXX-Qt bridge: core lifecycle & polling (`app_controller.rs`), settings & alert handlers (`settings_handlers.rs`), and XDG autostart portal/desktop entry management (`autostart.rs`) |
| `src/cpp/` | Modular Qt 6 / KDE Plasma C++ helpers: façade (`app_helper.h`), QPainter tray badge renderer (`icon_renderer.h`), KWin/X11/Wayland overlay manager (`window_manager.h`), StatusNotifierItem & `org.freedesktop.Notifications` snooze action handler (`tray_manager.h`), audio playback (`audio_portal.h`), and headless QML test runner (`ui_tests.h`) |
| [`windows/`](windows/README.md) | Windows 10/11 app (`eframe` / `egui`), DPAPI integration, WinRT Toast snooze activator, SHA-256 verified self-updater, Inno Setup installer script (`windows/installer/LibreLinkUp.iss`), and build tooling (`windows/scripts/build-windows.sh`). Standalone Cargo workspace so the Linux/Flathub build never vendors Windows-only crates |

---

## Building & Testing (Linux / KDE Plasma 6)

```bash
# Run the unit, integration, and headless QML UI layout test suites
cargo test -- --test-threads=1

# Build release binary
cargo build --release

# Run normally or minimized to tray
./target/release/librelinkup-kde
./target/release/librelinkup-kde --minimized
```

## Building & Installing the Flatpak (Linux)

```bash
chmod +x scripts/build-flatpak.sh
./scripts/build-flatpak.sh

# Install the generated Flatpak bundle
flatpak install --user -y target/release/librelinkup-kde.flatpak
```

## Building the Windows App (`x64`)

See [**`windows/README.md`**](windows/README.md) for full details.

On **Windows** (MSVC toolchain):

```powershell
cargo test --release --manifest-path crates/librelinkup-core/Cargo.toml
cargo build --release --manifest-path windows/Cargo.toml
windows\target\release\LibreLinkUp.exe --self-test
```

**Cross-compiling from Linux** (MinGW + Wine; also builds the Inno Setup installer and portable `.zip`):

```bash
rustup target add x86_64-pc-windows-gnu      # plus mingw-w64-gcc and wine from your distro
windows/scripts/build-windows.sh --self-test  # -> windows/dist/
```

GitHub Actions ([`.github/workflows/windows.yml`](.github/workflows/windows.yml)) builds, runs `--self-test`, and packages the Windows installer and portable `.zip` on `windows-latest`, attaching them automatically to GitHub Releases on `v*` tags.

---

## Interactive Release & GitHub Sync Scripts

Interactive multi-project release and sync scripts are located in the parent workspace directory (`/home/millinr/Builds/`) and let you choose which project to work on interactively:

| Script | Purpose & Usage |
| :--- | :--- |
| **`../release-menu.sh`** | Unified interactive menu: choose which project in `~/Builds` to work on, then choose whether to bump version, sync to GitHub, deploy a release, or run tests. |
| **`../bump-version.sh`** | Select a project and interactively increment Patch, Minor, Major, or Custom version across `Cargo.toml`, `crates/librelinkup-core/Cargo.toml`, `windows/Cargo.toml`, `src/qml/AboutDialog.qml`, `README.md`, `windows/README.md`, and AppStream `metainfo.xml`. |
| **`../sync-github.sh`** | Select a project and interactively stage files (`all`, `tracked`, or `patch`), commit, optionally tag `vX.Y.Z`, and push to GitHub. |
| **`../deploy-release.sh`** | Select a project and run the end-to-end release pipeline (optional version bump → core & integration tests → Linux Flatpak bundle + Windows 10/11 Inno Setup installer & portable `.zip` (`--self-test`) + SHA-256 checksums → git tag/push → GitHub Release creation via `gh`). |
