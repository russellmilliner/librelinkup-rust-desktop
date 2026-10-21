#!/usr/bin/env bash
# Interactive End-to-End Release Builder & GitHub Releases Publisher
# for LibreLinkUp Rust Desktop (Linux KDE Plasma 6 Flatpak + Windows 10/11 x64).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

BOLD="\033[1m"
CYAN="\033[1;36m"
GREEN="\033[1;32m"
YELLOW="\033[1;33m"
RED="\033[1;31m"
RESET="\033[0m"

BUMP_FLAG=""
TARGET_VERSION=""
NOTES_FILE=""
AUTO_YES=false

usage() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Interactive end-to-end release builder and GitHub Releases publisher for LibreLinkUp Rust Desktop.

Options:
  --patch                 Increment patch version before releasing
  --minor                 Increment minor version before releasing
  --major                 Increment major version before releasing
  --version <X.Y.Z>       Set explicit version before releasing
  --no-bump               Release the current version in Cargo.toml without bumping
  --notes-file <path>     Path to custom markdown release notes file
  -y, --yes               Non-interactive mode (accept defaults)
  -h, --help              Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --patch) BUMP_FLAG="--patch"; shift ;;
    --minor) BUMP_FLAG="--minor"; shift ;;
    --major) BUMP_FLAG="--major"; shift ;;
    --version) BUMP_FLAG="--version"; TARGET_VERSION="${2:-}"; shift 2 ;;
    --no-bump) BUMP_FLAG="none"; shift ;;
    --notes-file) NOTES_FILE="${2:-}"; shift 2 ;;
    -y|--yes) AUTO_YES=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo -e "${RED}Unknown argument: $1${RESET}" >&2; usage; exit 1 ;;
  esac
done

CURRENT_VERSION="$(grep -E '^version\s*=' "$REPO_ROOT/Cargo.toml" | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/')"

echo -e "${CYAN}============================================================${RESET}"
echo -e "${BOLD}  LibreLinkUp Rust Desktop — Interactive Release Deployer${RESET}"
echo -e "${CYAN}============================================================${RESET}"
echo -e "Current version in Cargo.toml: ${YELLOW}v${CURRENT_VERSION}${RESET}"
echo ""

# Step 1: Optional Version Bump
if [[ -z "$BUMP_FLAG" ]]; then
  if $AUTO_YES; then
    BUMP_FLAG="none"
  else
    read -r -p "Would you like to increment the version before building? [y/N]: " bump_ans
    bump_ans="${bump_ans:-N}"
    if [[ "$bump_ans" =~ ^[Yy] ]]; then
      "$REPO_ROOT/scripts/bump-version.sh"
    fi
  fi
elif [[ "$BUMP_FLAG" != "none" ]]; then
  BUMP_ARGS=("$BUMP_FLAG")
  if [[ -n "$TARGET_VERSION" ]]; then
    BUMP_ARGS+=("$TARGET_VERSION")
  fi
  if $AUTO_YES; then
    BUMP_ARGS+=("--yes")
  fi
  "$REPO_ROOT/scripts/bump-version.sh" "${BUMP_ARGS[@]}"
fi

VERSION="$(grep -E '^version\s*=' "$REPO_ROOT/Cargo.toml" | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/')"
TAG="v${VERSION}"
ARCH="$(uname -m)"

echo ""
echo -e "${BOLD}==> Step 1/4: Running core & integration tests for ${GREEN}${TAG}${RESET}..."
cargo test -p librelinkup-core
cargo test --workspace -- --test-threads=1
echo -e "${GREEN}  [OK] All Linux & core tests passed.${RESET}"

echo ""
echo -e "${BOLD}==> Step 2/4: Building Linux Flatpak & Windows 10/11 release artifacts...${RESET}"
"$REPO_ROOT/scripts/build-flatpak.sh" --no-install

if [[ -f "$HOME/.local/share/llu-wintoolchain/env.sh" ]]; then
  # shellcheck disable=SC1090
  source "$HOME/.local/share/llu-wintoolchain/env.sh"
fi
"$REPO_ROOT/windows/scripts/build-windows.sh" --self-test

FLATPAK_SRC="$REPO_ROOT/target/release/librelinkup-kde-${TAG}-${ARCH}.flatpak"
FLATPAK_DEST="$REPO_ROOT/target/release/librelinkup-rust-desktop-${TAG}-${ARCH}.flatpak"
cp "$FLATPAK_SRC" "$FLATPAK_DEST"
(cd "$REPO_ROOT/target/release" && sha256sum "$(basename "$FLATPAK_DEST")" > "$(basename "$FLATPAK_DEST").sha256")

WIN_DIST="$REPO_ROOT/windows/dist"
WIN_SETUP="$WIN_DIST/LibreLinkUp-Setup-${VERSION}-x64.exe"
WIN_ZIP="$WIN_DIST/LibreLinkUp-${VERSION}-windows-x64-portable.zip"

echo -e "${GREEN}  [OK] Built Linux & Windows release artifacts.${RESET}"

echo ""
echo -e "${BOLD}==> Step 3/4: Committing, tagging ${TAG}, and pushing to GitHub...${RESET}"
git add -A
if ! git diff --cached --quiet; then
  git commit -m "release: LibreLinkUp Rust Desktop ${TAG}"
fi
if ! git rev-parse "$TAG" >/dev/null 2>&1; then
  git tag -a "$TAG" -m "LibreLinkUp Rust Desktop ${TAG}"
fi

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
git push origin "$BRANCH"
git push origin "$TAG"
echo -e "${GREEN}  [OK] Pushed ${BRANCH} and ${TAG} to GitHub.${RESET}"

echo ""
echo -e "${BOLD}==> Step 4/4: Publishing GitHub Release ${TAG}...${RESET}"
if ! $AUTO_YES; then
  read -r -p "Publish/update GitHub Release ${TAG} with Linux + Windows artifacts now? [Y/n]: " rel_ans
  rel_ans="${rel_ans:-Y}"
  if [[ ! "$rel_ans" =~ ^[Yy] ]]; then
    echo "Exiting without publishing GitHub release."
    exit 0
  fi
fi

NOTES_TMP="$(mktemp)"
trap 'rm -f "$NOTES_TMP"' EXIT

if [[ -n "$NOTES_FILE" && -f "$NOTES_FILE" ]]; then
  cp "$NOTES_FILE" "$NOTES_TMP"
elif [[ -f "$REPO_ROOT/.github/release-notes/${TAG}.md" ]]; then
  cp "$REPO_ROOT/.github/release-notes/${TAG}.md" "$NOTES_TMP"
else
  prev_tag="$(git describe --tags --abbrev=0 "${TAG}^" 2>/dev/null || echo "")"
  changelog=""
  if [[ -n "$prev_tag" ]]; then
    changelog="$(git log --pretty=format:"- %s (%h)" "${prev_tag}..${TAG}" || true)"
  else
    changelog="$(git log -n 10 --pretty=format:"- %s (%h)" || true)"
  fi

  cat > "$NOTES_TMP" <<EOF
## LibreLinkUp Rust Desktop ${TAG}

### Changes
${changelog}

### Downloads
| Platform | Package | Description |
| :--- | :--- | :--- |
| **Windows 10/11 (\`x64\`)** | \`LibreLinkUp-Setup-${VERSION}-x64.exe\` | Per-user Inno Setup installer |
| **Windows 10/11 (\`x64\`)** | \`LibreLinkUp-${VERSION}-windows-x64-portable.zip\` | Standalone portable executable |
| **Linux (\`${ARCH}\`)** | \`librelinkup-rust-desktop-${TAG}-${ARCH}.flatpak\` | KDE Plasma 6 Flatpak bundle (\`org.kde.Platform//6.11\`) |
EOF
fi

ASSETS=(
  "$FLATPAK_DEST"
  "${FLATPAK_DEST}.sha256"
  "$WIN_SETUP"
  "${WIN_SETUP}.sha256"
  "$WIN_ZIP"
  "${WIN_ZIP}.sha256"
)

if gh release view "$TAG" >/dev/null 2>&1; then
  echo -e "Release ${YELLOW}${TAG}${RESET} already exists — uploading clobbered assets and updating notes..."
  gh release upload "$TAG" "${ASSETS[@]}" --clobber
  gh release edit "$TAG" --title "LibreLinkUp Rust Desktop ${TAG}" --notes-file "$NOTES_TMP"
else
  gh release create "$TAG" "${ASSETS[@]}" \
    --title "LibreLinkUp Rust Desktop ${TAG}" \
    --notes-file "$NOTES_TMP"
fi

echo ""
echo -e "${GREEN}============================================================${RESET}"
echo -e "${GREEN}  Release ${TAG} deployed successfully!${RESET}"
echo -e "  URL: https://github.com/russellmilliner/librelinkup-rust-desktop/releases/tag/${TAG}"
echo -e "${GREEN}============================================================${RESET}"
