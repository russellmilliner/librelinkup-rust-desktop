# LibreLinkUp Rust Desktop for Windows (10 / 11 x64)

A native, lightweight **Rust (`eframe` / `egui`)** desktop application for **Windows 10 and Windows 11 (x64)** that monitors continuous blood glucose readings from **LibreLinkUp**. It shares the platform-independent `librelinkup-core` crate with the Linux KDE Plasma 6 edition.

> **🛡️ Zero Tracking & Zero Advertisements — Forever:** LibreLinkUp Rust Desktop contains **no advertisements, no analytics, no telemetry beacons, no crash-reporting SDKs, and no third-party tracking — and never will.** Network communication is strictly limited to your allowlisted regional LibreView API endpoint and GitHub Releases (when checking for signed updates). All credentials, 90-day glucose history, and medication logs remain on your PC inside DPAPI-backed AES-256-GCM encrypted vaults.

---

## Downloads & Installation

| Package | Download Link | Description |
| :--- | :--- | :--- |
| **Windows Installer (`x64`)** | [**`LibreLinkUp-Setup-0.5.0-x64.exe`**](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.5.0/LibreLinkUp-Setup-0.5.0-x64.exe) ([SHA-256](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.5.0/LibreLinkUp-Setup-0.5.0-x64.exe.sha256) • [Minisign](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.5.0/LibreLinkUp-Setup-0.5.0-x64.exe.minisig)) | Per-user Inno Setup installer (no admin prompt required), Start Menu shortcut with Toast `AppUserModelID`, optional start at Windows sign-in |
| **Portable Zip (`x64`)** | [**`LibreLinkUp-0.5.0-windows-x64-portable.zip`**](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.5.0/LibreLinkUp-0.5.0-windows-x64-portable.zip) ([SHA-256](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.5.0/LibreLinkUp-0.5.0-windows-x64-portable.zip.sha256) • [Minisign](https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.5.0/LibreLinkUp-0.5.0-windows-x64-portable.zip.minisig)) | Standalone `LibreLinkUp.exe` executable — extract anywhere and run |

### PowerShell Quick Install

```powershell
Invoke-WebRequest -Uri "https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/v0.5.0/LibreLinkUp-Setup-0.5.0-x64.exe" -OutFile "$env:TEMP\LibreLinkUp-Setup-0.5.0-x64.exe"
Start-Process -FilePath "$env:TEMP\LibreLinkUp-Setup-0.5.0-x64.exe"
```

> **Windows SmartScreen Note**: Because the binary is not code-signed with a commercial EV certificate, Windows SmartScreen may display *"Windows protected your PC"* on first launch. Click **More info → Run anyway**.

---

## Features

### 1. Live Glucose Dashboard & 12-Hour Interactive Graph
- Displays current blood glucose reading (`mg/dL` or `mmol/L`), vector-drawn trend arrow, range status pill (**In Range**, **High**, **Low**, **Urgent Low**, or **STALE**), patient name, and reading age (`"2 min ago"`).
- **12-Hour Interactive Trend Graph**:
  - Shaded green target range band between your configured Low and High thresholds.
  - Color-coded measurement dots connected by a continuous trend line.
  - Hover over any point on the chart to inspect the exact glucose value, status, and timestamp.
- Quick action buttons on the Dashboard to toggle the **Floating Overlay** or **Snooze Alerts (30m)** when an alert is active.

### 2. Floating Always-on-Top Desktop Overlay
A borderless, translucent, always-on-top desktop widget (`276×88` default, configurable from `200×70` to `600×260`) that stays visible over other windows without cluttering the Windows Taskbar:
- **Scroll Wheel Transparency Control**: Hover anywhere over the overlay and scroll the mouse wheel up or down to adjust opacity between **`20%`** and **`100%`** in **`5%`** increments. Changes take effect immediately and persist automatically.
- **On-Overlay Control Buttons (2-Row Cluster)**:
  - **`🌙` / `🎨` Theme Toggle**: Switch between the **Dark Slate** card (`#0f172a` with a left status-color accent bar) and **Status Color Fill** (card background colored by your glucose range status). The hover tooltip displays the current opacity percentage.
  - **`⊞` On All Desktops Toggle (`overlay_on_all_desktops`)**: Toggle whether the floating overlay appears across all Windows 10/11 Virtual Desktops (`WS_EX_TOOLWINDOW` active, highlighted in green) or stays on the current virtual desktop only (also toggleable from the **Settings** tab and the overlay `☰` / right-click context menu).
  - **`↗` Open Main Window**: Restores and focuses the main LibreLinkUp Rust Desktop window.
  - **`☰` Context Menu**: Opens a popup menu directly anchored to the button with **Show main window**, **Refresh now**, **Switch to color/dark theme**, **Show on all virtual desktops**, and **Hide overlay**.
  - **`✕` Hide Overlay**: Hides the floating overlay and saves visibility state (highlights in soft red on hover).
- **Drag & Mouse Interactions**:
  - **Left-click + drag** anywhere on the card background to reposition; the exact screen coordinates (`overlay_x`, `overlay_y`) are saved automatically when dropped.
  - **Double-click** the card background to open the main window.
  - **Right-click** anywhere on the card background to open the overlay context menu.

### 3. System Tray Icon & Live Glucose Badge
- Dynamically renders a 32×32 high-contrast, color-coded badge in the Windows notification area showing either your live glucose reading (`mg/dL` or `mmol/L`) or a blood-drop icon.
- **Hover Tooltip**: Shows patient name, current reading, trend description, range status, and reading age.
- **Left-click** the tray icon to restore and bring the main window to the foreground.
- **Right-click** the tray icon for the native Windows menu:
  - **Refresh Now**
  - **Check for Updates**
  - **Show / Hide Floating Overlay**
  - **Snooze Alerts — 30 min / 1 hour / 2 hours** & **Clear Alert Snooze**
  - **Open LibreLinkUp**
  - **Quit**
- Closing the main window (`✕`) hides it cleanly to the system tray while background polling, alerts, and the desktop overlay continue running. Use **Quit** in the tray menu to exit completely.

### 4. Stale-Reading Detection & Alert Engine
- **Stale-Reading Detection**: Every reading displays its age (`"3 min ago"`). Readings older than your configurable threshold (default **15 min**, range `5`–`60` min) turn gray (`#64748b`), display **STALE**, and optionally trigger a signal-loss alert.
- **Multi-Tier Alert Engine**:
  - **Urgent Low** (default `55 mg/dL`): **Cannot be snoozed** and repeats at least every 5 minutes.
  - **Low** (default `70 mg/dL`), **High** (default `180 mg/dL`), and **Stale Data**: Follow your configurable repeat interval (`5`–`120` min, default `15` min).
  - Escalating to a more severe state (e.g., Low → Urgent Low) triggers an alert immediately regardless of the repeat timer or active snooze.
- **Snooze**: Snooze non-urgent alerts for **30 minutes**, **1 hour**, or **2 hours** directly from the desktop alert toast notification buttons (`Snooze 30 min`, `Snooze 1 hr`, `Snooze 2 hr`), the Dashboard, the Alerts tab, or the System Tray menu. Snooze expiration timestamps persist across app restarts.
- **Audible Alarm & Windows Toast Notifications**:
  - Plays a built-in two-tone (`880 Hz` / `660 Hz`) urgent alarm synthesized in memory via `rodio`, or a custom `.wav`, `.mp3`, or `.ogg` audio file chosen in the **Alerts** tab.
  - Sends native Windows Toast notifications registered under `AppUserModelID` `io.github.russellmilliner.LibreLinkUpDesktop` with interactive **Snooze 30 min**, **Snooze 1 hr**, and **Snooze 2 hr** action buttons.
  - Includes a **Test Alert Sound** button in the **Alerts** tab.

### 5. Medications Tab & Glycemic Analytics (`eA1C` / `GMI` / `TIR%`)
- **Estimated A1C (`eA1C`), `GMI` & Time in Range (`TIR %`)**:
  - Switch between `12h`, `7d`, `14d`, `30d`, and `90d` analytics windows on either the **Dashboard** or the **Medications** tab (saved automatically in `settings.json`).
  - Computes **ADAG `eA1C`** (`%` and IFCC `mmol/mol`), **Bergenstal `GMI`** (`%` and `mmol/mol`), **Time in Range (`TIR %`)**, `% Below`, `% Above`, Mean Glucose, `Min–Max`, and `CV %` from your encrypted local 90-day history vault (`%LOCALAPPDATA%\LibreLinkUpDesktop\glucose_history.enc.json`).
- **Scheduled & As-Needed (`PRN`) Medication Tracking**:
  - Add medications with **Name**, **Dosage**, **Schedule** (`Daily`, `Twice Daily`, `Weekly` with a **Day of Week** `Monday`–`Sunday` selector, or `As Needed`), **24-hour Time(s)** (`HH:MM`), and optional desktop due-dose reminders.
  - **In-Place Editing (`✏`) & Removal (`🗑`)**: Click **`✏`** on any scheduled medication row to edit its schedule, dosage, or times in place, or **`🗑`** to delete it.
  - **One-Click Dose Logging (`✓ Taken` / `⏭ Skipped` / `Log Dose`)**: Log taken or skipped doses with one click; taken doses automatically record your concurrent blood glucose reading (`mg/dL` or `mmol/L`) into `%APPDATA%\LibreLinkUpDesktop\medications.enc.json`.
  - **12-Hour Glucose Graph Markers (`💊`)**: Doses logged within the last 12 hours are plotted directly on the 12-hour glucose trend graph as purple diamond markers (`#a855f7`) with vertical guide lines and medication labels.
  - **Recent Dose History**: View and manage your recent dose logs (with timestamps, dosages, and concurrent glucose values).

---

## Security, Privacy & Windows Data Paths

- **Zero Tracking & Zero Advertisements — Forever**: There are **no advertisements, no analytics, no telemetry beacons, no crash-reporting SDKs, and no third-party tracking** in LibreLinkUp Rust Desktop — and **never will be**.

| Resource | Windows Location / Mechanism |
| :--- | :--- |
| **Encrypted Credentials Vault** | `%APPDATA%\LibreLinkUpDesktop\credentials.enc.json` (`AES-256-GCM` + `PBKDF2-HMAC-SHA256`, 210,000 rounds) |
| **Encrypted Glucose History** | `%LOCALAPPDATA%\LibreLinkUpDesktop\glucose_history.enc.json` — 90 days of readings, `AES-256-GCM` |
| **Encrypted Medication Vault** | `%APPDATA%\LibreLinkUpDesktop\medications.enc.json` — schedule + dose log, `AES-256-GCM` |
| **DPAPI Master Key** | `%APPDATA%\LibreLinkUpDesktop\master.dpapi` — 32 random bytes encrypted via Windows **DPAPI (`CryptProtectData` / `CryptUnprotectData`)** bound to your Windows user account |
| **Application Settings** | `%APPDATA%\LibreLinkUpDesktop\settings.json` |
| **Diagnostic Event Log** | In-memory ring buffer (1000 entries); exports are written where you choose, mode-restricted |
| **Crash Log** | `%LOCALAPPDATA%\LibreLinkUpDesktop\crash.log` |
| **Login Autostart Registry Key** | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` → `LibreLinkUpDesktop` (`"C:\...\LibreLinkUp.exe" --minimized`) |
| **Toast Notification Identity** | `HKCU\Software\Classes\AppUserModelId\io.github.russellmilliner.LibreLinkUpDesktop` |
| **Single-Instance Mutex** | `Local\LibreLinkUpRustDesktop_SingleInstance` (launching a second instance brings the existing window to the foreground) |

- **Opt-In Password Storage**: By default (`remember_password = false`), only the encrypted LibreView session JWT and account metadata are stored in `credentials.enc.json`. Enable **Remember password in encrypted vault (auto re-login)** on the sign-in screen if you want automatic re-authentication when a session token expires.
- **Signed, Verified In-App Updater**: When installing an update, `LibreLinkUp.exe` validates that the asset URLs belong to `https://github.com/russellmilliner/librelinkup-rust-desktop/releases/download/...`, downloads the installer into a private folder under `%LOCALAPPDATA%\LibreLinkUpDesktop\`, and verifies **both** the cryptographic **SHA-256 digest** *and* a **minisign signature** against the public key compiled into the binary. Unsigned or tampered installers are never executed. (The SHA-256 sidecar comes from the same origin as the installer, so the signature is the check that actually proves authorship.)
- **Fail-closed key handling**: if `credentials.enc.json` exists but `master.dpapi` cannot be unwrapped (for example after copying the vault to another PC), the app refuses to create a replacement key — that would make your data permanently unreadable — and asks you to sign in again.
- **Encrypted health data**: glucose history and medication data are encrypted with the same DPAPI-protected master key as the credential vault, and plaintext `glucose_history.json` / `medications.json` from earlier versions are migrated and deleted on first launch.
- **Memory & Log Redaction**: Sensitive credential strings and session tokens are wiped from memory on drop using `zeroize`, custom `Debug` implementations redact secrets and glucose readings, and email addresses are masked in `%LOCALAPPDATA%\LibreLinkUpDesktop\events.log`.
- During uninstallation, Inno Setup stops any running instance, removes the autostart registry entry, and optionally offers to remove `%APPDATA%\LibreLinkUpDesktop` and `%LOCALAPPDATA%\LibreLinkUpDesktop`.

---

## Command-Line Options

```text
LibreLinkUp.exe [OPTIONS]

Options:
  --minimized, --autostart   Start hidden in the system tray (or floating overlay if enabled)
  --demo                     Load realistic sample 12-hour CGM data without signing in
  --self-test                Run headless verification of paths, DPAPI, AES-256-GCM vault,
                             autostart registry, tray badge renderer, audio synthesizer,
                             and LibreView HTTPS connectivity
  --check-update, -u         Check GitHub Releases for a newer version and exit
  --version, -V              Print version and exit
```

You can also set the environment variable `START_MINIMIZED=1` to launch minimized to the system tray.

---

## Building from Source

The Windows crate (`windows/Cargo.toml`) is a standalone Cargo workspace so that Linux/Flathub builds never vendor Windows-only dependencies.

### 1. Native Build on Windows (MSVC)

Prerequisites:
- [Rust stable toolchain](https://rustup.rs/) (`x86_64-pc-windows-msvc`)
- [Inno Setup 6](https://jrsoftware.org/isinfo.php) (optional, for building the installer)

```powershell
# Run shared core unit tests
cargo test --release --manifest-path crates/librelinkup-core/Cargo.toml

# Build optimized release binary
cargo build --release --manifest-path windows/Cargo.toml

# Run headless self-test
.\windows\target\release\LibreLinkUp.exe --self-test

# Build the Inno Setup installer (produces windows\dist\LibreLinkUp-Setup-0.5.0-x64.exe)
& "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" `
    /DAppVersion=0.4.5 `
    /DSourceExe=..\target\release\LibreLinkUp.exe `
    /DOutputDir=..\dist `
    windows\installer\LibreLinkUp.iss
```

### 2. Cross-Compiling from Linux (MinGW + Wine)

Prerequisites:
- `rustup target add x86_64-pc-windows-gnu`
- `x86_64-w64-mingw32-gcc` and `x86_64-w64-mingw32-windres` (MinGW-w64)
- `wine` (for running `--self-test` and Inno Setup `ISCC.exe`)

```bash
# Build LibreLinkUp.exe, run --self-test under Wine, and package installer + portable zip
./windows/scripts/build-windows.sh --self-test
```

Generated artifacts are placed in `windows/dist/`:
- `windows/dist/LibreLinkUp-Setup-0.5.0-x64.exe` (+ `.sha256`)
- `windows/dist/LibreLinkUp-0.4.5-windows-x64-portable.zip` (+ `.sha256`)

---

## Graphics & Renderer Compatibility

`LibreLinkUp.exe` attempts to initialize hardware-accelerated rendering via **`wgpu`** (DirectX 12 / Vulkan) first. If a compatible GPU driver is unavailable (for example in virtual machines, Remote Desktop sessions, or older integrated graphics), it automatically falls back to **`glow`** (OpenGL) so the UI and transparent overlay remain functional.
