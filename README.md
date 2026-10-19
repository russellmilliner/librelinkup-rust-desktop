# LibreLinkUp Desktop (KDE Plasma 6 Edition)

A native **Rust + Qt 6 / KDE Kirigami** rewrite of [LibreLinkUpDesktop](https://github.com/Crazy-Marvin/LibreLinkUpDesktop) designed specifically for **KDE Plasma 6**.

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
- **AES-256-GCM Encrypted Credential Vault**:
  - All stored credentials and session tokens are encrypted at rest in `~/.local/share/librelinkup-kde/credentials.enc` using **AES-256-GCM** with a 100,000-iteration **PBKDF2-HMAC-SHA256** key derived from machine identity (`ring`) and strict `0600` permissions.
  - Sensitive strings are wiped from memory on drop via `zeroize`.
  - Automatically migrates active tokens from the legacy Electron Flatpak (`rocks.poopjournal.librelinkupdesktop`) on first launch.
- **Diagnostic Log Viewer**:
  - Built-in Log Viewer dialog with category filtering (`AUTH`, `API`, `CRYPTO`, `TRAY`, `ALERT`, `APP`), minimum severity filtering (`DEBUG`, `INFO`, `WARN`, `ERROR`), real-time search, clipboard copy, and `.log` file export.
- **12-Hour Interactive Glucose Graph & Floating Overlay Widget**:
  - Interactive Canvas chart with target range bands, hover inspection, and an optional borderless always-on-top desktop overlay widget.

## Building & Testing

```bash
# Run the unit and integration test suites
cargo test

# Build release binary
cargo build --release

# Run normally or minimized to tray
./target/release/librelinkup-kde
./target/release/librelinkup-kde --minimized
```

## Building the Flatpak

```bash
chmod +x scripts/build-flatpak.sh
./scripts/build-flatpak.sh

# Install the generated Flatpak bundle
flatpak install --user -y target/release/librelinkup-kde.flatpak
```
