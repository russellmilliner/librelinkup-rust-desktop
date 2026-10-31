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
┌──────────────────────────────────────────────────────────────────────────┐
│ crates/librelinkup-core  (platform independent, no Qt, no Win32)          │
│  api          LibreView HTTP client, region allowlist, curl fallback      │
│  crypto       AES-256-GCM envelope + master-secret provider               │
│  session      polling, 401 re-auth + backoff, alert decision              │
│  models       AppSettings, GlucoseSnapshot, alert policy, formatting      │
│  analytics    eA1C / GMI / TIR, 90-day encrypted history vault            │
│  medications  schedule, dose log, reminders, encrypted medication vault   │
│  updater      GitHub release check, SHA-256 + minisign verification       │
│  paths        XDG / %APPDATA% resolution, atomic 0600 writes              │
│  event_log    in-memory ring buffer + PII masking                         │
│  alert_sound  generated alarm WAV                                         │
└───────────────┬───────────────────────────────────┬──────────────────────┘
                │                                   │
   ┌────────────▼─────────────┐        ┌────────────▼─────────────┐
   │ src/bridge (KDE, Qt)     │        │ windows/src (egui)        │
   │  state.rs   AppState     │        │  service.rs  worker thread│
   │  ops.rs     transitions  │        │  app.rs      UI           │
   │  qobject_bridge.rs       │        │  tray.rs, alerts.rs       │
   │  telemetry.rs            │        │  platform.rs DPAPI/registry│
   │  settings.rs             │        └───────────────────────────┘
   │  medications.rs          │
   │  autostart.rs            │
   └────────────┬─────────────┘
                │ C++ shims (src/cpp/*.h)
   ┌────────────▼─────────────────────────────────────────────┐
   │ tray_manager.h  QSystemTrayIcon, freedesktop notifications│
   │ window_manager.h overlay placement, KWin all-desktops     │
   │ audio_portal.h  paplay/pw-play fallback chain             │
   │ icon_renderer.h live glucose badge rendering              │
   │ ui_tests.h      headless QML layout/behaviour assertions  │
   └──────────────────────────────────────────────────────────┘
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
QML Timer(250 ms) ─► telemetry.tick() ────┤ ops::refresh_display / analytics / med views
                     settings.sync()      │ ops::persist_history_if_due
                     medications.sync()   ▼
                                    properties pushed (only when changed)
```

* **`bridge/state.rs`** holds `AppState` in a `thread_local RefCell`. This is deliberate: all
  access happens on the Qt GUI thread, and a `RefCell` turns any accidental re-entrancy into a
  loud panic instead of a deadlock. **Never touch it from a background thread.**
* **`bridge/ops.rs`** contains every state transition that is not a property setter:
  display refresh, analytics recompute, medication view models (with `last_graph_markers_key`
  caching so `GraphPoint` timestamps are not re-parsed on unchanged 250 ms ticks), event-log
  refresh gated by `EVENT_LOG_SEQ`, history persistence throttle, and `current_settings()`
  (which clones `AppState::base_settings` in memory for zero disk I/O per tick, while
  `persist_settings()` preserves any externally managed fields from `settings.json`).
* **`bridge/telemetry.rs`** owns all async work (login, refresh, update check/install) and
  funnels results back with `qt_thread().queue(...)`. It is also the object the C++ tray holds
  a pointer to (`m_controller`), so tray actions are dispatched there and delegated into shared
  state via short one-line wrappers, and tray state (including `overlay_only_mode` via
  `TrayPayload::flags & 0x40`) is pushed via `TrayPayload`.
* Cross-controller refresh is **pull-based**: the QML timer calls `sync()` on each controller.
  There is no cross-object signal. If you need a value visible in three places, put it in
  `AppState` and mirror it in each `sync()`.
* Headless tests must drive this explicitly (`ui_tests.h` has a `sync_all()` helper); do not
  paper over it with longer sleeps.

### Adding a new setting (checklist)

1. `AppSettings` in `crates/librelinkup-core/src/models.rs`: field + `Default` + serde.
2. `AppState` in `src/bridge/state.rs`: mirror field, read it in `Default`.
3. `ops::current_settings()`: copy the field into the persisted struct.
4. `qobject_bridge.rs`: `#[qproperty]` + the matching field in `*ControllerRust`, plus a
   `#[qinvokable]` setter.
5. `settings.rs`: setter body (mutate state → `ops::persist_settings` → `self.sync()`) and add
   it to `sync()`.
6. `src/qml/Main.qml`: bind the control; **no Save button** (see invariant I-6).
7. `windows/src/app.rs` + `service.rs` if the Windows front-end should honour it too.

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
| **I-12** | `OverlayWidget` tooltips must render in external top-level `librelinkupOverlayTooltipWindow` (never in-scene `QQC2.ToolTip`) | in-scene `QQuickPopup` tooltips clip to the `276×88` overlay window and obscure the action buttons |

---

## 7. Testing

```bash
# Core logic (fast, no Qt)
cargo test -p librelinkup-core

# Whole workspace incl. headless Qt/QML tests
QT_QPA_PLATFORM=offscreen cargo test --workspace -- --test-threads=1

# Lint gate used by CI
cargo clippy --workspace --all-targets -- -D warnings
```

* `tests/integration_tests.rs` spins up loopback mock LibreView servers. Plaintext loopback
  endpoints are refused unless `LIBRELINKUP_ALLOW_INSECURE_TEST_ENDPOINT=1` **or** the build
  has `debug_assertions` (i.e. never in a release binary).
* `src/cpp/ui_tests.h` loads the real `Main.qml` offscreen and asserts: no layout overflow at
  780×540 / 980×680 / 1280×800, on all 7 tabs, in both themes; overlay placement/clamping;
  `Qt::Tool`+`BypassWindowManagerHint` flags; tray always visible; snooze works.
  **Any change to QML or the controller surface must keep this green.**
* `test_live_libreview_waf_no_403_forbidden` is `#[ignore]`d: it performs real network I/O.
  Run it manually with `cargo test -- --ignored`. It never uses real credentials.
