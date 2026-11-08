#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_ID="io.github.russellmilliner.LibreLinkUpDesktop"
VERSION="$(grep -E '^version\s*=' "$REPO_ROOT/Cargo.toml" | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/')"
ARCH="$(uname -m)"
WORK_DIR="${FLATPAK_WORK_DIR:-$REPO_ROOT/target/flatpak-work}"
OUT_DIR="${FLATPAK_OUT_DIR:-$REPO_ROOT/target/release}"
BUNDLE_NAME="librelinkup-kde-v${VERSION}-${ARCH}.flatpak"

echo "==> Building LibreLinkUp Desktop v${VERSION} Flatpak (${ARCH})..."
cargo metadata --format-version 1 --manifest-path "$REPO_ROOT/Cargo.toml" > /dev/null
rm -rf "$WORK_DIR/build-dir" "$WORK_DIR/repo"
mkdir -p "$WORK_DIR" "$OUT_DIR"

flatpak build-init \
  --sdk-extension=org.freedesktop.Sdk.Extension.rust-stable \
  --sdk-extension=org.freedesktop.Sdk.Extension.llvm22 \
  "$WORK_DIR/build-dir" \
  "$APP_ID" \
  org.kde.Sdk \
  org.kde.Platform \
  6.11

CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"

flatpak build \
  --share=network \
  --filesystem="$REPO_ROOT":ro \
  --filesystem="$CARGO_HOME_DIR":ro \
  --filesystem="$WORK_DIR":rw \
  --env=PATH=/usr/lib/sdk/rust-stable/bin:/usr/lib/sdk/llvm22/bin:/usr/bin:/bin \
  --env=CARGO_HOME="$CARGO_HOME_DIR" \
  --env=CARGO_TARGET_DIR="$WORK_DIR/target" \
  --env=LIBCLANG_PATH=/usr/lib/sdk/llvm22/lib \
  --env=QMAKE=/usr/bin/qmake \
  --env=QT_VERSION_MAJOR=6 \
  "$WORK_DIR/build-dir" \
  sh -c "
    set -euo pipefail
    rm -rf \"\$CARGO_TARGET_DIR\"/release/build/librelinkup-kde-* \"\$CARGO_TARGET_DIR\"/release/.fingerprint/librelinkup-kde-*
    cargo build --release --manifest-path '$REPO_ROOT/Cargo.toml'
    install -Dm755 \"\$CARGO_TARGET_DIR/release/librelinkup-kde\" -t /app/bin/
    install -Dm644 '$REPO_ROOT/data/${APP_ID}.desktop' -t /app/share/applications/
    install -Dm644 '$REPO_ROOT/data/${APP_ID}.metainfo.xml' -t /app/share/metainfo/
    install -Dm644 '$REPO_ROOT/data/icons/${APP_ID}.svg' -t /app/share/icons/hicolor/scalable/apps/
    install -Dm644 '$REPO_ROOT/LICENSE' '$REPO_ROOT/NOTICE' -t /app/share/licenses/${APP_ID}/
  "

echo "==> Composing AppStream catalog and bundle icons for Bazaar / KDE Discover..."
flatpak build "$WORK_DIR/build-dir" sh -c "
  set -euo pipefail
  IMG_FMT_ARG=''
  if appstreamcli compose --help 2>&1 | grep -q -- '--image-format'; then
    IMG_FMT_ARG='--image-format=png'
  fi
  appstreamcli compose \
    --prefix=/ \
    --origin=flatpak \
    --components='${APP_ID}' \
    --result-root=/app/share/app-info \
    --data-dir=/app/share/app-info/xmls \
    --icons-dir=/app/share/app-info/icons/flatpak \
    \$IMG_FMT_ARG \
    --no-net \
    /app
  if [ -f /app/share/app-info/xmls/flatpak.xml.gz ]; then
    mv /app/share/app-info/xmls/flatpak.xml.gz /app/share/app-info/xmls/${APP_ID}.xml.gz
  fi
  for sz in 64x64 128x128; do
    if [ -f \"/app/share/app-info/icons/flatpak/\${sz}/${APP_ID}.png\" ]; then
      install -Dm644 \
        \"/app/share/app-info/icons/flatpak/\${sz}/${APP_ID}.png\" \
        \"/app/share/icons/hicolor/\${sz}/apps/${APP_ID}.png\"
    fi
  done
"

flatpak build-finish "$WORK_DIR/build-dir" \
  --command=librelinkup-kde \
  --share=ipc \
  --share=network \
  --socket=wayland \
  --socket=fallback-x11 \
  --socket=pulseaudio \
  --device=dri \
  --talk-name=org.kde.StatusNotifierWatcher \
  --talk-name=org.freedesktop.Notifications \
  --talk-name=org.freedesktop.Flatpak \
  --talk-name=org.kde.KWin \
  --filesystem=xdg-config/kdeglobals:ro \
  --filesystem=xdg-config/home-assistant.env:ro \
  --filesystem=xdg-config/librelinkup-kde:create \
  --filesystem=xdg-data/librelinkup-kde:create \
  --filesystem=xdg-run/librelinkup-kde:create \
  --filesystem=~/.var/app/rocks.poopjournal.librelinkupdesktop:ro

flatpak build-export "$WORK_DIR/repo" "$WORK_DIR/build-dir" stable
flatpak build-bundle "$WORK_DIR/repo" "$OUT_DIR/$BUNDLE_NAME" "$APP_ID" stable --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo
RELEASE_BUNDLE_NAME="librelinkup-rust-desktop-v${VERSION}-${ARCH}.flatpak"
(
  cd "$OUT_DIR"
  sha256sum "$BUNDLE_NAME" > "${BUNDLE_NAME}.sha256"
  cp -f "$BUNDLE_NAME" "$RELEASE_BUNDLE_NAME"
  sha256sum "$RELEASE_BUNDLE_NAME" > "${RELEASE_BUNDLE_NAME}.sha256"
  ln -sf "$BUNDLE_NAME" librelinkup-kde.flatpak
)
rm -rf "$WORK_DIR/build-dir" "$WORK_DIR/repo"

echo "==> Built Flatpak bundle: $OUT_DIR/$BUNDLE_NAME"

if [[ "${1:-}" != "--no-install" ]]; then
  echo "==> Installing Flatpak bundle..."
  if ! flatpak install --user -y --reinstall "$OUT_DIR/$BUNDLE_NAME"; then
    echo "==> User scope install failed (e.g. Bazzite system-scoped Flathub runtime); retrying with --system..."
    flatpak install --system -y --reinstall "$OUT_DIR/$BUNDLE_NAME"
  fi
fi
