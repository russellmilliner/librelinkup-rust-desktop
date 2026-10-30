# AGENTS.md — rules for AI agents and contributors

This file is the contract for automated contributors. Read
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and [`docs/SECURITY.md`](docs/SECURITY.md)
before editing. If your change conflicts with anything here, stop and ask.

---

## 1. Commands you must run before declaring work done

```bash
# 1. Core logic tests (fast)
cargo test -p librelinkup-core

# 2. Full workspace, including the headless QML layout/behaviour suite
QT_QPA_PLATFORM=offscreen cargo test --workspace -- --test-threads=1

# 3. Lint gate (CI fails on any warning)
cargo clippy --workspace --all-targets -- -D warnings

# 4. Windows front-end (only if you touched windows/)
cargo clippy --manifest-path windows/Cargo.toml --all-targets -- -D warnings
```

Never mark a task complete with a red test, a clippy warning, or a skipped/ignored test you
added yourself.

---

## 2. Hard rules

1. **Staleness fails closed.** Unknown reading age ⇒ treat as stale. Never make an unknown
   age fall through to "fresh".
2. **No new plaintext PHI on disk.** New health data goes into an AES-256-GCM envelope via
   `crypto::encrypt_bytes_with_secret` with its own AAD label, and `write_file_atomic_0600`.
3. **No secrets in argv.** Use stdin or `--config -` for tokens/passwords/keys.
4. **Never weaken the update chain.** Both the SHA-256 *and* the minisign signature must
   verify before anything is executed. Do not add a "skip verification" path.
5. **Never rotate or mint a master key silently.** If a vault exists and its key is gone,
   return an error.
6. **No `unsafe` in Rust except Win32 FFI** in `windows/src/platform.rs` and the message
   pump in `windows/src/tray.rs`, each with a comment justifying it.
7. **Redact new PHI/secrets.** Any struct that holds a token, password, id, name, glucose
   value or medication must have a manual `Debug` impl printing `[REDACTED]`, plus a test.
8. **Keep `settings` application instant.** No "Save" button; persist inside every setter.
9. **QML must never overflow.** Use `Layout.fillWidth`, `elide`, `wrapMode`, `ScrollView`.
   The headless suite catches regressions — run it.
10. **Use `QQC2.TabBar`/`TabButton` for tabs**, never buttons-as-tabs or buttons-as-labels.
11. **Zoom/scale controls live in the bottom status bar** and are persisted (see the project
    style rules); read fonts/colours from `Kirigami.Theme`, never hardcode pixels.
12. **Never commit build artifacts** (`target/`, `windows/dist/`, `*.flatpak`), keys
    (`*.key`, `*.minisig` secret halves) or real credentials.

---

## 3. Where things go

| You want to… | Edit |
|---|---|
| add API parsing / network behaviour | `crates/librelinkup-core/src/api.rs` |
| change alert policy | `models.rs` (`classify_alert`, `should_fire_alert`) + `session.rs` |
| add a persisted setting | follow the 7-step checklist in ARCHITECTURE.md §2 |
| add a QML page/control | `src/qml/Main.qml` (+ its own file if > ~200 lines) |
| add a tray/window action | `qobject_bridge.rs` declaration → `telemetry.rs` delegate → `src/cpp/*.h` |
| change overlay/flags behaviour | `src/cpp/window_manager.h` + `ui_tests.h` assertions |
| add a notification action | `models::ALERT_NOTIFICATION_SNOOZE_ACTIONS`, `tray_manager.h`, `handle_notification_action` |
| change the updater | `updater.rs` (+ tests) and `.github/workflows/windows.yml` signing steps |

---

## 4. Definition of done for a feature

- [ ] Core logic in `librelinkup-core`, not duplicated per front-end.
- [ ] Unit test for the pure logic; QML/layout coverage via the headless suite.
- [ ] State change reachable from all three controllers if it is user-visible.
- [ ] Windows front-end honours it (or an explicit note why not).
- [ ] `CHANGELOG.md` entry under the next version.
- [ ] Docs updated: `docs/ARCHITECTURE.md` (flow), `docs/SECURITY.md` (threat surface),
      `README.md` (user-visible behaviour).
- [ ] No new clippy warnings; no new `unsafe`; no new plaintext file.

---

## 5. Things that look like a good idea but are not

| Tempting change | Why it is wrong here |
|---|---|
| "Treat unknown age as fresh to avoid false alarms" | hides sensor failure — the exact failure mode the app exists to catch |
| "Skip signature check when offline/dev" | ships an RCE path; use the loopback test gate instead |
| "Store history as plain JSON for easier debugging" | PHI on disk |
| "Auto-regenerate the key if decryption fails" | destroys the user's vault permanently |
| "Poll every 15 s for fresher data" | clamped to 60 s: LibreView rate-limits and the sensor updates every 1–5 min |
| "Add a Save button so users can review changes" | the style contract requires instant apply+persist |
| "Put all controllers back into one object" | it was split for a reason; the god object is what this refactor removed |
