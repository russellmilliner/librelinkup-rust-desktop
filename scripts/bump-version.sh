#!/usr/bin/env bash
# Interactive Version Incrementer for LibreLinkUp Rust Desktop
# Updates Cargo.toml, crates/librelinkup-core/Cargo.toml, windows/Cargo.toml,
# src/qml/AboutDialog.qml, README.md, windows/README.md, and AppStream metainfo.xml.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

BOLD="\033[1m"
CYAN="\033[1;36m"
GREEN="\033[1;32m"
YELLOW="\033[1;33m"
RED="\033[1;31m"
RESET="\033[0m"

CURRENT_VERSION="$(grep -E '^version\s*=' "$REPO_ROOT/Cargo.toml" | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/')"
IFS='.' read -r CUR_MAJOR CUR_MINOR CUR_PATCH <<< "${CURRENT_VERSION%%-*}"

NEXT_PATCH="${CUR_MAJOR}.${CUR_MINOR}.$((CUR_PATCH + 1))"
NEXT_MINOR="${CUR_MAJOR}.$((CUR_MINOR + 1)).0"
NEXT_MAJOR="$((CUR_MAJOR + 1)).0.0"

NEW_VERSION=""
RELEASE_SUMMARY=""
AUTO_YES=false

usage() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS] [NEW_VERSION]

Interactive script to increment the LibreLinkUp Rust Desktop version across all crates and docs.

Options:
  --patch                Increment patch version ($CURRENT_VERSION -> $NEXT_PATCH)
  --minor                Increment minor version ($CURRENT_VERSION -> $NEXT_MINOR)
  --major                Increment major version ($CURRENT_VERSION -> $NEXT_MAJOR)
  --version <X.Y.Z>      Set explicit version
  --summary "<text>"     Release summary for AppStream metainfo.xml
  -y, --yes              Non-interactive mode (accept defaults)
  -h, --help             Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --patch) NEW_VERSION="$NEXT_PATCH"; shift ;;
    --minor) NEW_VERSION="$NEXT_MINOR"; shift ;;
    --major) NEW_VERSION="$NEXT_MAJOR"; shift ;;
    --version) NEW_VERSION="${2:-}"; shift 2 ;;
    --summary) RELEASE_SUMMARY="${2:-}"; shift 2 ;;
    -y|--yes) AUTO_YES=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *)
      if [[ -z "$NEW_VERSION" && "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        NEW_VERSION="$1"
        shift
      else
        echo -e "${RED}Unknown argument: $1${RESET}" >&2
        usage
        exit 1
      fi
      ;;
  esac
done

echo -e "${CYAN}============================================================${RESET}"
echo -e "${BOLD}  LibreLinkUp Rust Desktop — Version Incrementer${RESET}"
echo -e "${CYAN}============================================================${RESET}"
echo -e "Current version: ${YELLOW}v${CURRENT_VERSION}${RESET}"
echo ""

if [[ -z "$NEW_VERSION" ]]; then
  if $AUTO_YES; then
    NEW_VERSION="$NEXT_PATCH"
  else
    echo -e "${BOLD}Select version bump type:${RESET}"
    echo -e "  ${GREEN}1)${RESET} Patch  -> ${BOLD}v${NEXT_PATCH}${RESET} (bug fixes / minor tweaks)"
    echo -e "  ${GREEN}2)${RESET} Minor  -> ${BOLD}v${NEXT_MINOR}${RESET} (new features / backwards-compatible)"
    echo -e "  ${GREEN}3)${RESET} Major  -> ${BOLD}v${NEXT_MAJOR}${RESET} (major release)"
    echo -e "  ${GREEN}4)${RESET} Custom version"
    echo -e "  ${GREEN}q)${RESET} Cancel"
    echo ""
    read -r -p "Choice [1-4, default=1]: " choice
    choice="${choice:-1}"
    case "$choice" in
      1) NEW_VERSION="$NEXT_PATCH" ;;
      2) NEW_VERSION="$NEXT_MINOR" ;;
      3) NEW_VERSION="$NEXT_MAJOR" ;;
      4)
        read -r -p "Enter custom semantic version (e.g. 0.5.0): " custom_ver
        custom_ver="${custom_ver#v}"
        if [[ ! "$custom_ver" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
          echo -e "${RED}Invalid version format: '$custom_ver' (expected X.Y.Z)${RESET}" >&2
          exit 1
        fi
        NEW_VERSION="$custom_ver"
        ;;
      q|Q) echo "Cancelled."; exit 0 ;;
      *) echo -e "${RED}Invalid selection.${RESET}" >&2; exit 1 ;;
    esac
  fi
fi

NEW_VERSION="${NEW_VERSION#v}"
if [[ ! "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo -e "${RED}Error: Version '$NEW_VERSION' is not valid X.Y.Z format.${RESET}" >&2
  exit 1
fi

if [[ -z "$RELEASE_SUMMARY" ]]; then
  if ! $AUTO_YES; then
    echo ""
    read -r -p "Enter one-line release summary for AppStream metainfo (optional, Enter for default): " RELEASE_SUMMARY
  fi
  if [[ -z "$RELEASE_SUMMARY" ]]; then
    RELEASE_SUMMARY="Release v${NEW_VERSION} of LibreLinkUp Rust Desktop."
  fi
fi

echo ""
echo -e "Updating version: ${YELLOW}v${CURRENT_VERSION}${RESET} -> ${GREEN}v${NEW_VERSION}${RESET}"

if ! $AUTO_YES; then
  read -r -p "Proceed with file updates? [Y/n]: " confirm
  confirm="${confirm:-Y}"
  if [[ ! "$confirm" =~ ^[Yy] ]]; then
    echo "Aborted."
    exit 0
  fi
fi

TODAY="$(date +%Y-%m-%d)"

python3 - "$CURRENT_VERSION" "$NEW_VERSION" "$TODAY" "$RELEASE_SUMMARY" << 'PYEOF'
import re
import sys
import xml.sax.saxutils as saxutils
from pathlib import Path

old_ver, new_ver, today, summary = sys.argv[1:5]

for toml_path in ["Cargo.toml", "crates/librelinkup-core/Cargo.toml", "windows/Cargo.toml"]:
    p = Path(toml_path)
    if p.is_file():
        text = p.read_text(encoding="utf-8")
        text = re.sub(r'(?m)^version\s*=\s*"[^"]+"', f'version = "{new_ver}"', text, count=1)
        p.write_text(text, encoding="utf-8")
        print(f"  [OK] Updated {toml_path} -> {new_ver}")

about_qml = Path("src/qml/AboutDialog.qml")
if about_qml.is_file():
    text = about_qml.read_text(encoding="utf-8")
    text = text.replace(f'"{old_ver}"', f'"{new_ver}"')
    about_qml.write_text(text, encoding="utf-8")
    print(f"  [OK] Updated src/qml/AboutDialog.qml -> {new_ver}")

for doc_path in ["README.md", "windows/README.md"]:
    p = Path(doc_path)
    if p.is_file():
        text = p.read_text(encoding="utf-8")
        text = text.replace(f"v{old_ver}", f"v{new_ver}").replace(old_ver, new_ver)
        p.write_text(text, encoding="utf-8")
        print(f"  [OK] Updated {doc_path} -> v{new_ver}")

metainfo = Path("data/io.github.russellmilliner.LibreLinkUpDesktop.metainfo.xml")
if metainfo.is_file():
    text = metainfo.read_text(encoding="utf-8")
    if f'version="{new_ver}"' not in text:
        esc_summary = saxutils.escape(summary)
        new_release_block = (
            f'  <releases>\n'
            f'    <release version="{new_ver}" date="{today}">\n'
            f'      <description>\n'
            f'        <p>{esc_summary}</p>\n'
            f'      </description>\n'
            f'    </release>'
        )
        text = text.replace("  <releases>", new_release_block, 1)
        metainfo.write_text(text, encoding="utf-8")
        print(f"  [OK] Added <release version=\"{new_ver}\" date=\"{today}\"> to metainfo.xml")
    else:
        print(f"  [OK] metainfo.xml already contains release {new_ver}")
PYEOF

echo ""
echo -e "${GREEN}Successfully bumped version to v${NEW_VERSION}!${RESET}"
