# Releasing — LibreLinkUp Rust Desktop

> Follow this in order. The update chain is signed and fails closed, so a missed step means
> **no user can auto-update** (or, worse, a bad artifact gets published).

---

## 0. One-time setup

1. Generate (or restore) the release key:
   ```bash
   minisign -G -p distribution/update-signing.pub -s ~/.config/librelinkup-release/release-signing.key
   ```
   The **public** key must equal `UPDATE_SIGNING_PUBLIC_KEY` in
   `crates/librelinkup-core/src/updater.rs` and `distribution/update-signing.pub`.
   * Rotating the key requires shipping a manual update first — every installed copy trusts
     only the compiled-in key.
2. Add the **secret** key contents to the GitHub repository secret `RELEASE_MINISIGN_KEY`
   (Settings → Secrets → Actions). Keep an offline backup; there is no recovery.

---

## 1. Pre-flight

```bash
cargo test -p librelinkup-core
QT_QPA_PLATFORM=offscreen cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --manifest-path windows/Cargo.toml --all-targets -- -D warnings
cargo audit
```

---

## 2. Bump the version

Update **all** of these to the same `X.Y.Z`:

| File | Field |
|---|---|
| `Cargo.toml` | `[package] version` |
| `crates/librelinkup-core/Cargo.toml` | `version` |
| `windows/Cargo.toml` | `version` |
| `data/io.github.russellmilliner.LibreLinkUpDesktop.metainfo.xml` | new `<release version=…date=…>` |
| `CHANGELOG.md` | new section at the top |
| `README.md` | version badge, if present |

Then `cargo build --workspace` so each lockfile picks up the new version.

---

## 3. Flatpak metadata (only when shipping to Flathub)

1. Regenerate the vendored crate list — **required** whenever dependencies change
   (e.g. `minisign-verify` was added in 0.5.0):
   ```bash
   python3 flatpak-cargo-generator.py Cargo.lock -o flathub/cargo-sources.json
   ```
2. Bump `flathub/io.github.russellmilliner.LibreLinkUpDesktop.yml` `tag:`/`commit:` to the
   release commit (full 40-char SHA).

---

## 4. Tag and publish

```bash
git commit -am "release: LibreLinkUp Rust Desktop vX.Y.Z"
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin main --tags
```

The `CI & Release` workflow then:

| Job | Produces |
|---|---|
| `security-audit` | `cargo audit` gate |
| `linux-test` | clippy + headless Qt/QML tests |
| `build` | `LibreLinkUp-Setup-X.Y.Z-x64.exe`, portable zip, `.sha256` |
| `linux-flatpak` | `librelinkup-kde-vX.Y.Z-x86_64.flatpak` + `.sha256` |
| `release` | **signs every artifact with minisign** and attaches everything |

The `release` job fails if `RELEASE_MINISIGN_KEY` is missing — unsigned artifacts are never
published.

---

## 5. Verify after publishing

```bash
gh release download vX.Y.Z --pattern '*.flatpak' --pattern '*.flatpak.minisig' --pattern 'distribution*' -D /tmp/rel
minisign -V -P "$(grep -v '^untrusted comment' distribution/update-signing.pub)" \
  -m /tmp/rel/librelinkup-kde-vX.Y.Z-x86_64.flatpak
```

Also check, on an existing install:

```bash
librelinkup-kde --check-update      # must report the new version
```

and click **Install** once to confirm the signed artifact passes verification and installs.

---

## 6. Local (non-CI) signing

```bash
./scripts/build-flatpak.sh --no-install
./scripts/sign-release.sh ~/.config/librelinkup-release/release-signing.key \
  target/release/librelinkup-kde-vX.Y.Z-x86_64.flatpak
```

`scripts/sign-release.sh` prints the exact `minisign -V` command to verify.
