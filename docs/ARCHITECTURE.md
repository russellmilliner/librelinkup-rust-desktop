# Architecture — LibreLinkUp Rust Desktop

> **Audience:** maintainers and AI agents contributing to this repository.
> **Read this before changing code.** The invariants at the end are not style preferences —
> several of them are safety-critical for a glucose monitor.

---

## 1. What the app is

A desktop CGM (continuous glucose monitor) client for Abbott's **LibreLinkUp** service and the **Home Assistant LibreLink integration**. It can either log in as a LibreLinkUp *follower* account (polling `/llu/connections/<patientId>/graph`) or connect to a user-configured Home Assistant instance (polling `/api/states` and `/api/history/period/...`, with automatic multi-patient discovery, sensor warmup/expiry countdowns, two-way medication/dose sync via `sensor.librelinkup_medications`, and cross-device alert snooze sync via `sensor.librelinkup_alert_snooze`), rendering the latest reading plus a 12-hour trend and raising desktop alerts and medication reminders.

There are **two front-ends** over one shared core:

| Front-end | Stack | Location |
|---|---|---|
| KDE Plasma 6 (primary) | CXX-Qt 0.7 + Qt 6 QML + Kirigami + KDE Frameworks 6 | `src/`, `src/qml/`, `src/cpp/` |
| Windows 10/11 | egui/eframe + tray-icon + WinRT toasts + DPAPI | `windows/` |

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│ crates/librelinkup-core  (platform independent, no Qt, no Win32)                  │
│  api/{mod,http,libreview,home_assistant}.rs  LibreView & HA HTTP clients          │
│  crypto.rs        AES-256-GCM Envelope v3 (HKDF-SHA256) + EncryptedVault<T>       │
│  session.rs       polling, re-auth backoff, shared HA sync & full alert cycle     │
│  models/{mod,glucose,settings,alerts,a11y}.rs  settings, snapshot, alert policy   │
│  analytics.rs     eA1C / GMI / TIR (partition_point slicing), 90d history vault   │
│  medications.rs   schedule, dose log, O(N) dedup, reminders, medication vault     │
│  updater.rs       GitHub release check, SHA-256 + minisign artifact verification  │
│  paths.rs         XDG / %APPDATA% resolution, atomic 0600 writes                  │
│  event_log.rs     in-memory ring buffer + PII masking                             │
│  alert_sound.rs   generated alarm WAV                                             │
└───────────────┬───────────────────────────────────────────┬──────────────────────┘
                │                                           │
   ┌────────────▼──────────────┐               ┌────────────▼──────────────┐
   │ src/bridge (KDE, Qt)      │               │ windows/src (egui)        │
   │  macros.rs  sync_str/val  │               │  service/  async runtime  │
   │  state.rs   AppState      │               │   {mod,commands,sync,alerts}│
   │  ops.rs     transitions   │               │  ui/       eframe views   │
   │  qobject_bridge.rs        │               │   {mod,login,dashboard,   │
   │  telemetry.rs             │               │    chart,overlay,settings,│
   │  settings.rs              │               │    medications,alerts,    │
   │  medications.rs           │               │    dialogs}               │
   │  autostart.rs             │               │  tray.rs, alerts.rs       │
   └────────────┬──────────────┘               │  platform.rs DPAPI/Win32  │
                │                              └───────────────────────────┘
                │ C++ modules (src/cpp/*.{h,cpp}) + QRC resources
   ┌────────────▼──────────────────────────────────────────────────────────┐
   │ app_helper.h           NotificationActionWatcher Q_OBJECT umbrella    │
   │ tray_manager.{h,cpp}   KStatusNotifierItem, knotifications6, KGlobalAccel│
   │ window_manager.{h,cpp} Wayland KWin async D-Bus + kwin-resident.js QRC│
   │ audio_portal.{h,cpp}   Wayland platform init + paplay / pw-play       │
   │ icon_renderer.{h,cpp}  live glucose badge + blood drop watermark      │
   │ ui_tests.{h,cpp} / ui_tests_stub.cpp (`ui-tests` feature-gated)       │
   └───────────────────────────────────────────────────────────────────────┘
   QML views (src/qml/*.qml): Main, HighContrastScrollBar, GlucoseChart,
     LoginView, DashboardView, MedicationsView, SettingsView, AlertsView,
     AccountView, LogViewerDialog, OverlayWidget, AboutDialog
```

---

## 2. KDE front-end: the three-controller split

`AppController` used to be a single object with ~70 properties and ~35 invokables; it was
split (v0.5.0) into three QML-visible objects that share one state store.

| QML object | Class | Owns |
|---|---|---|
| `telemetry` | `TelemetryController` | auth, CGM snapshot, display strings, analytics, **service host** |
| `settings` | `SettingsController` | preferences, overlay/window placement, alert config, updater *status*, log viewer |
| `medications` | `MedicationsController` | medication schedule + dose log |

### Data flow

```
QML action ──► controller invokable ──► bridge::state (AppState)
                                          │
Coalesced mark_state_dirty()              │ ops::refresh_display / analytics / med views
  └─► sharedStateChanged signal ──────────┤ ops::persist_history_if_due (async offload)
        ├─► telemetry.sync()              ▼
        ├─► settings.sync()         properties pushed (only when changed via sync_str!/sync_val!)
        └─► medications.sync()
```

* **`bridge/macros.rs`** provides shared `sync_str!` and `sync_val!` property-change macros across
  `telemetry.rs`, `settings.rs`, and `medications.rs`.
* **`bridge/state.rs`** holds `AppState` in a `thread_local RefCell` on the Qt GUI thread plus an
  atomic `mark_state_dirty()` flag that coalesces updates and emits `TelemetryController::shared_state_changed`.
  A `RefCell` turns any accidental cross-thread re-entrancy into a loud panic instead of a deadlock.
  **Never touch it from a background thread.**
* **`bridge/ops.rs`** contains every state transition that is not a property setter:
  display refresh, analytics recompute, medication view models (with `last_graph_markers_key`
  caching so `GraphPoint` timestamps are not re-parsed on unchanged updates), event-log
  refresh gated by `EVENT_LOG_SEQ`, history persistence throttle, and `current_settings()`
  (which clones `AppState::base_settings` in memory for zero disk I/O per sync, while
  `persist_settings()` preserves any externally managed fields from `settings.json`).
* **`bridge/telemetry.rs`** owns all async work (login, refresh, update check/install) and
  funnels results back with `qt_thread().queue(...)`. It also serves as the tray's service host for
  glucose refresh, snooze, updates, and notifications (`m_controller`), while `TrayManager::settingsController()`
  routes settings-mutating tray menu actions directly to `SettingsController`, and tray state
  (including `overlay_only_mode` via `TrayPayload::flags & 0x40`) is pushed via `TrayPayload`.
* Cross-controller refresh is **event-driven**: state mutations trigger `mark_state_dirty()`, which
  coalesces into `sharedStateChanged` in `Main.qml` so `settings.sync()` and `medications.sync()`
  run immediately with zero idle 250 ms timer polling.
* Headless tests must drive this explicitly (`ui_tests.cpp` has a `sync_all()` helper); do not
  paper over it with longer sleeps.

### Adding a new setting (checklist)

1. `AppSettings` in `crates/librelinkup-core/src/models/settings.rs`: field + `Default` + serde.
2. `AppState` in `src/bridge/state.rs`: mirror field, read it in `Default`.
3. `ops::current_settings()`: copy the field into the persisted struct.
4. `qobject_bridge.rs`: `#[qproperty]` + the matching field in `*ControllerRust`, plus a
   `#[qinvokable]` setter.
5. `settings.rs`: setter body (mutate state → `ops::persist_settings` → `self.sync()` → `mark_state_dirty()`)
   and add it to `sync()`.
6. `src/qml/SettingsView.qml` / `src/qml/AlertsView.qml`: bind the control with instant-save on
   interaction; **no Save button** (see invariant I-6).
7. `windows/src/ui/` + `windows/src/service/` if the Windows front-end should honour it too.

---

## 3. Threading model

| Thread | Owns | Rules |
|---|---|---|
| Qt GUI thread | `AppState`, all QObjects, all property writes | only place `with_state` may be called |
| `librelinkup-net` (tokio, 2 workers) | HTTP, crypto, file writes for async results | no `AppState` access; results go through `qt_thread().queue` |
| `llu-service` (Windows) | same role as above, service loop | guarded by `Arc<Mutex<Shared>>` |
| `llu-tray` (Windows) | Win32 message pump for the tray | sends `TrayView` over an mpsc channel |

Async results are tagged with `session_generation` (and a refresh `seq`) so a logout/re-login
that happens mid-request can never resurrect a stale session or recreate a deleted vault.

---

## 4. On-disk format

| Path (`~/.config/librelinkup-kde` / `%APPDATA%\LibreLinkUpDesktop`) | Contents | Protection |
|---|---|---|
| `credentials.enc.json` | token, account id, optional password | AES-256-GCM, label `credentials` |
| `glucose_history.enc.json` (`data_dir`) | 90 days of readings | AES-256-GCM, label `glucose-history` |
| `medications.enc.json` | medication schedule + dose log | AES-256-GCM, label `medications` |
| `master.key` (Linux, if no keyring) | 256-bit key, hex | `0600`; migrated to the keyring when available |
| `master.dpapi` (Windows) | DPAPI-wrapped key | bound to the Windows user |
| `settings.json` | preferences | `0600`, plaintext (no PHI beyond thresholds) |

Directories are created `0700`. Writes always go through
`paths::write_file_atomic_0600` (sibling temp file with `create_new` → `sync_all` → rename).

Pre-0.5 plaintext `glucose_history.json` / `medications.json` are migrated into the encrypted
vaults on first run and then deleted (see `load_default_glucose_history`,
`load_default_medications`).

---

## 5. Alert pipeline

```
fetch → GlucoseSnapshot ──► session::effective_reading_age_minutes
                              (reading clock + last-successful-fetch clock, min of both)
                          ──► AppSettings::classify_alert   (stale > urgent-low > low > high)
                          ──► AppSettings::should_fire_alert (snooze, escalation, repeat)
                          ──► AlertDecision { kind, title, body, critical }
                          ──► C++: freedesktop notification (+ snooze actions) / WinRT toast
```

`AlertKind::is_critical()` decides notification urgency: **UrgentLow and Low are critical;
High and Stale are not.** Signal loss must not raise an un-dismissable critical toast forever.

---

## 6. Invariants — do not break these

| # | Invariant | Why |
|---|---|---|
| **I-1** | Staleness **fails closed**: unknown age ⇒ stale | showing an old value as live is the worst failure mode for a CGM |
| **I-2** | Health data never hits disk in plaintext | it is PHI; a 90-day series is more sensitive than a token |
| **I-3** | No secrets in `Command` arguments (pipelines/stdin only) | `ps`/`/proc/*/cmdline` are world-readable |
| **I-4** | Every auto-update is signature-verified before execution | a checksum from the same origin proves nothing about authorship |
| **I-5** | A missing master key must **never** mint a replacement | silent key rotation permanently destroys the vault |
| **I-6** | Settings apply and persist on interaction — no "Save" button | a lost setting in a medical app is a real hazard |
| **I-7** | `Test::Debug` on anything holding PII/secrets must redact | logs are exported and pasted into bug reports |
| **I-8** | Outbound URLs are allowlisted (LibreView regions, GitHub release assets) | prevents token exfiltration via a spoofed redirect |
| **I-9** | Background results are generation-tagged before touching state | prevents resurrecting a signed-out session |
| **I-10** | Polling is clamped to ≥60 s **and** jittered | LibreView rate-limits; lock-step instances trip it |
| **I-11** | `TrayManager` reads state from `TrayPayload` (never `SettingsController` properties on `m_controller`) | `m_controller` is `TelemetryController`; reading missing properties returns `false` and can trigger queued-invocation feedback loops |
| **I-12** | `OverlayWidget` tooltips must render in external top-level `librelinkupOverlayTooltipWindow` (`transientParent == overlayWindow`, `Qt::ToolTip \| Qt::FramelessWindowHint \| Qt::WindowTransparentForInput \| Qt::WindowDoesNotAcceptFocus`, never in-scene `QQC2.ToolTip`) | in-scene `QQuickPopup` tooltips clip to the `276×88` overlay window, while anchoring the tooltip window as a `Wayland` `xdg_popup` on `overlayWindow` prevents `kwin_wayland` focus-stealing and one-shot script positioning stalls |
| **I-13** | Native Wayland (`QT_QPA_PLATFORM=wayland`) uses KWin D-Bus scripting (`org.kde.KWin` `/Scripting`) + `QWindow::startSystemMove()` + `/LibreLinkUpKWin` callbacks | Wayland clients cannot set or read global screen coordinates directly; KWin scripts enforce `noBorder`, `keepAbove`, `skipTaskbar/Pager/Switcher`, `onAllDesktops`, and multi-monitor coordinate persistence without XWayland |
| **I-14** | System tray glucose badge (`build_glucose_badge_icon`) always renders the blood drop + ECG logo behind dark-outlined white text | keeps the application's signature blood drop emblem recognizable at all times while preserving high-contrast numeral legibility |
| **I-15** | Desktop overlay opacity (`overlayOpacityPercent`, `20%`–`100%`) is applied to the whole widget container (`overlayPill.opacity` + `layer.enabled: opacityAlpha < 0.999` on KDE; `ui.set_opacity()` on Windows) | applying alpha only to the background/border leaves foreground text and buttons at 100% and makes dark-slate fill changes invisible over dark desktop windows |
| **I-16** | KDE notifications pass `x-kde-appname` + `x-kde-eventId` hints matching `data/io.github.russellmilliner.LibreLinkUpDesktop.notifyrc`, and internal KWin helper shortcuts are prefixed `LibreLinkUp (internal):` while user shortcuts register under `io.github.russellmilliner.LibreLinkUpDesktop.desktop` | keeps all 9 alert/reminder events configurable in KDE System Settings → Notifications (with Urgent Low fail-safe) and keeps user vs. internal shortcuts clearly separated in KDE System Settings → Shortcuts |

---

## 7. Testing

```bash
# Core logic (fast, no Qt)
cargo test -p librelinkup-core

# Whole workspace incl. headless Qt/QML tests
QT_QPA_PLATFORM=offscreen cargo test --workspace -- --test-threads=1

# Headless UI/overflow suite directly
QT_QPA_PLATFORM=offscreen cargo run -- --ui-test

# Production release build without UI test harness (Flatpak / Flathub)
cargo build --release --no-default-features

# Lint gate used by CI (default and --no-default-features)
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --no-default-features --all-targets -- -D warnings

# Flatpak SDK offline compatibility (rustc 1.98.1)
flatpak run --command=sh --env=CARGO_TARGET_DIR=/tmp/llu-sdk-check --env=QMAKE=/usr/bin/qmake \
  --env=QT_VERSION_MAJOR=6 --env=LIBCLANG_PATH=/usr/lib/sdk/llvm22/lib org.kde.Sdk//6.11 \
  -c "cargo check --offline --no-default-features"

# Windows tests & MinGW cross-clippy
cargo test --manifest-path windows/Cargo.toml
cargo clippy --manifest-path windows/Cargo.toml --target x86_64-pc-windows-gnu -- -D warnings
```

* `tests/integration_tests.rs` spins up loopback mock LibreView servers. Plaintext loopback
  endpoints are refused unless `LIBRELINKUP_ALLOW_INSECURE_TEST_ENDPOINT=1` **or** the build
  has `debug_assertions` (i.e. never in a release binary).
* `src/cpp/ui_tests.cpp` (enabled by the default `ui-tests` Cargo feature; stubbed by
  `src/cpp/ui_tests_stub.cpp` when `--no-default-features` is used for release Flatpak builds)
  loads `Main.qml` and modular views offscreen and asserts: no layout overflow at
  780×540 / 980×680 / 1280×800, on all 7 tabs, in both themes, across 80%/100%/130% zoom levels;
  overlay placement/clamping; `Qt::Tool`+`BypassWindowManagerHint` flags; tray always visible; snooze works.
  **Any change to QML or the controller surface must keep this green.**
* `test_live_libreview_waf_no_403_forbidden` is `#[ignore]`d: it performs real network I/O.
  Run it manually with `cargo test -- --ignored`. It never uses real credentials.
