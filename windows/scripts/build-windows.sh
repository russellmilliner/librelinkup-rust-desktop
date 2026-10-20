#!/usr/bin/env bash
# Cross-compiles LibreLinkUp for Windows on Linux and packages it:
#   dist/LibreLinkUp-Setup-<ver>-x64.exe            (Inno Setup installer, built via Wine)
#   dist/LibreLinkUp-<ver>-windows-x64-portable.zip  (exe + license)
#   dist/*.sha256
#
# Requirements (one-time):
#   - rustup with the x86_64-pc-windows-gnu target (e.g. the isolated toolchain sourced from
#     $LLU_WIN_TOOLCHAIN_ENV, default ~/.local/share/llu-wintoolchain/env.sh)
#   - mingw-w64-gcc (linker / C compiler for ring) and Wine
#   - Inno Setup 6 installed in $WINEPREFIX (default ~/.local/share/llu-wintoolchain/wineprefix)
#
# Usage: windows/scripts/build-windows.sh [--self-test] [--no-installer]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WIN_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$WIN_DIR/.." && pwd)"
TARGET=x86_64-pc-windows-gnu
TOOLCHAIN_ENV="${LLU_WIN_TOOLCHAIN_ENV:-$HOME/.local/share/llu-wintoolchain/env.sh}"
export WINEPREFIX="${WINEPREFIX:-$HOME/.local/share/llu-wintoolchain/wineprefix}"
export WINEDEBUG="${WINEDEBUG:--all}"
export WINEDLLOVERRIDES="${WINEDLLOVERRIDES:-mscoree,mshtml=}"
ISCC="$WINEPREFIX/drive_c/InnoSetup/ISCC.exe"

RUN_SELF_TEST=0
BUILD_INSTALLER=1
for arg in "$@"; do
  case "$arg" in
    --self-test) RUN_SELF_TEST=1 ;;
    --no-installer) BUILD_INSTALLER=0 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

if [[ -f "$TOOLCHAIN_ENV" ]]; then
  # shellcheck source=/dev/null
  . "$TOOLCHAIN_ENV"
fi

VERSION="$(grep -E '^version\s*=' "$WIN_DIR/Cargo.toml" | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/')"
EXE="$WIN_DIR/target/$TARGET/release/LibreLinkUp.exe"
DIST="$WIN_DIR/dist"

echo "==> Building LibreLinkUp for Windows v$VERSION ($TARGET)"
cargo build --release --target "$TARGET" --manifest-path "$WIN_DIR/Cargo.toml"

if [[ "$RUN_SELF_TEST" == 1 ]]; then
  echo "==> Running --self-test under Wine"
  wine "$EXE" --self-test | tr -d '\r'
fi

rm -rf "$DIST"
mkdir -p "$DIST/portable"

if [[ "$BUILD_INSTALLER" == 1 ]]; then
  if [[ ! -f "$ISCC" ]]; then
    echo "Inno Setup not found at $ISCC (install it into the Wine prefix or pass --no-installer)" >&2
    exit 1
  fi
  echo "==> Building installer with Inno Setup (Wine)"
  (cd "$WIN_DIR/installer" && wine "$ISCC" /Q \
      "/DAppVersion=$VERSION" \
      "/DSourceExe=$(winepath -w "$EXE")" \
      "/DOutputDir=$(winepath -w "$DIST")" \
      LibreLinkUp.iss)
fi

echo "==> Packaging portable zip"
cp "$EXE" "$DIST/portable/LibreLinkUp.exe"
cp "$WIN_DIR/README.md" "$DIST/portable/README.md"
cp "$REPO_ROOT/LICENSE" "$DIST/portable/LICENSE.txt"
cp "$REPO_ROOT/NOTICE" "$DIST/portable/NOTICE.txt"
(cd "$DIST/portable" && zip -q -9 "../LibreLinkUp-$VERSION-windows-x64-portable.zip" LibreLinkUp.exe README.md LICENSE.txt NOTICE.txt)
rm -rf "$DIST/portable"

(cd "$DIST" && for f in *.exe *.zip; do [[ -f "$f" ]] && sha256sum "$f" > "$f.sha256"; done)
echo "==> Artifacts:"
ls -la "$DIST"
