#!/usr/bin/env bash
# Sign release artifacts with the LibreLinkUp release key (minisign).
#
# The public key is compiled into the application (see
# `crates/librelinkup-core/src/updater.rs::UPDATE_SIGNING_PUBLIC_KEY`), so a signature
# produced here is the only thing that lets an installed copy auto-update. Treat the secret
# key as the most sensitive asset in the project.
#
# Usage:
#   scripts/sign-release.sh <secret-key-file> <artifact> [artifact...]
#
# Environment:
#   MINISIGN_KEY_FILE   alternative to passing the key path as the first argument.
#
# The secret key is NOT part of this repository. Keep an offline backup; if it is lost,
# every installed copy must be updated manually before auto-updates work again.
set -euo pipefail
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

if [[ -n "${MINISIGN_KEY_FILE:-}" ]]; then
  KEY_FILE="$MINISIGN_KEY_FILE"
else
  KEY_FILE="${1:-}"
  if [[ -z "$KEY_FILE" ]]; then
    echo "usage: $0 <secret-key-file> <artifact> [artifact...]" >&2
    echo "   or: MINISIGN_KEY_FILE=<secret-key-file> $0 <artifact> [artifact...]" >&2
    exit 2
  fi
  shift
fi

if (( $# == 0 )); then
  echo "error: no artifacts given" >&2
  exit 2
fi

if [[ ! -f "$KEY_FILE" ]]; then
  echo "error: secret key file not found: $KEY_FILE" >&2
  exit 1
fi

if ! command -v minisign >/dev/null 2>&1; then
  echo "error: minisign is not installed (try: sudo pacman -S minisign / apt install minisign)" >&2
  exit 1
fi

for artifact in "$@"; do
  if [[ ! -f "$artifact" ]]; then
    echo "error: artifact not found: $artifact" >&2
    exit 1
  fi
  echo "==> Signing $artifact"
  minisign -S -s "$KEY_FILE" -m "$artifact" \
    -c "LibreLinkUp Rust Desktop release artifact" \
    -t "artifact:$(basename "$artifact")"
done

echo
echo "==> Signatures:"
printf '%s.minisig\n' "$@"
echo
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
echo "Verify with:"
echo "  minisign -V -P $(grep -v '^untrusted comment' "$REPO_ROOT/distribution/update-signing.pub") -m <artifact>"
