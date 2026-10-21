#!/usr/bin/env bash
# Interactive Git & GitHub Sync Script for LibreLinkUp Rust Desktop
# Stages, commits, optionally tags, and pushes changes to GitHub.

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
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
COMMIT_MSG=""
CREATE_TAG=""
AUTO_YES=false

usage() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Interactive script to stage, commit, tag, and sync changes with GitHub.

Options:
  -m, --message "<msg>"   Commit message
  -t, --tag               Create and push annotated tag v${CURRENT_VERSION}
  --no-tag                Do not create a git tag
  -y, --yes               Non-interactive mode (stage all, commit, and push)
  -h, --help              Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -m|--message) COMMIT_MSG="${2:-}"; shift 2 ;;
    -t|--tag) CREATE_TAG="yes"; shift ;;
    --no-tag) CREATE_TAG="no"; shift ;;
    -y|--yes) AUTO_YES=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo -e "${RED}Unknown argument: $1${RESET}" >&2; usage; exit 1 ;;
  esac
done

echo -e "${CYAN}============================================================${RESET}"
echo -e "${BOLD}  LibreLinkUp Rust Desktop — Git & GitHub Sync${RESET}"
echo -e "${CYAN}============================================================${RESET}"
echo -e "Branch:  ${GREEN}${BRANCH}${RESET}"
echo -e "Version: ${YELLOW}v${CURRENT_VERSION}${RESET}"
echo ""

STATUS_OUT="$(git status -s)"
if [[ -n "$STATUS_OUT" ]]; then
  echo -e "${BOLD}Working tree changes:${RESET}"
  git status -s
  echo ""

  STAGE_ACTION="all"
  if ! $AUTO_YES; then
    echo -e "${BOLD}How would you like to stage changes?${RESET}"
    echo -e "  ${GREEN}1)${RESET} Stage all modified & untracked files (git add -A)"
    echo -e "  ${GREEN}2)${RESET} Stage tracked files only (git add -u)"
    echo -e "  ${GREEN}3)${RESET} Interactive patch staging (git add -p)"
    echo -e "  ${GREEN}4)${RESET} Skip staging & committing (only push existing commits)"
    echo -e "  ${GREEN}q)${RESET} Cancel"
    read -r -p "Choice [1-4, default=1]: " stage_choice
    stage_choice="${stage_choice:-1}"
    case "$stage_choice" in
      1) STAGE_ACTION="all" ;;
      2) STAGE_ACTION="tracked" ;;
      3) STAGE_ACTION="patch" ;;
      4) STAGE_ACTION="skip" ;;
      q|Q) echo "Cancelled."; exit 0 ;;
      *) echo -e "${RED}Invalid choice.${RESET}" >&2; exit 1 ;;
    esac
  fi

  if [[ "$STAGE_ACTION" != "skip" ]]; then
    case "$STAGE_ACTION" in
      all) git add -A ;;
      tracked) git add -u ;;
      patch) git add -p ;;
    esac

    if ! git diff --cached --quiet; then
      DEFAULT_MSG="chore: sync v${CURRENT_VERSION} updates"
      if [[ -z "$COMMIT_MSG" ]]; then
        if $AUTO_YES; then
          COMMIT_MSG="$DEFAULT_MSG"
        else
          read -r -p "Enter commit message [default: '$DEFAULT_MSG']: " COMMIT_MSG
          COMMIT_MSG="${COMMIT_MSG:-$DEFAULT_MSG}"
        fi
      fi
      git commit -m "$COMMIT_MSG"
      echo -e "${GREEN}  [OK] Committed changes: ${COMMIT_MSG}${RESET}"
    else
      echo -e "${YELLOW}  [i] No staged changes to commit.${RESET}"
    fi
  fi
else
  echo -e "${GREEN}Working tree is clean.${RESET}"
fi

TAG_NAME="v${CURRENT_VERSION}"
if git rev-parse "$TAG_NAME" >/dev/null 2>&1; then
  echo -e "Tag ${YELLOW}${TAG_NAME}${RESET} already exists locally."
else
  if [[ -z "$CREATE_TAG" ]]; then
    if $AUTO_YES; then
      CREATE_TAG="no"
    else
      read -r -p "Create annotated git tag ${TAG_NAME}? [y/N]: " tag_ans
      tag_ans="${tag_ans:-N}"
      if [[ "$tag_ans" =~ ^[Yy] ]]; then
        CREATE_TAG="yes"
      else
        CREATE_TAG="no"
      fi
    fi
  fi

  if [[ "$CREATE_TAG" == "yes" ]]; then
    git tag -a "$TAG_NAME" -m "LibreLinkUp Rust Desktop ${TAG_NAME}"
    echo -e "${GREEN}  [OK] Created annotated tag ${TAG_NAME}${RESET}"
  fi
fi

if ! $AUTO_YES; then
  echo ""
  read -r -p "Push branch '${BRANCH}' (and any new tags) to origin? [Y/n]: " push_ans
  push_ans="${push_ans:-Y}"
  if [[ ! "$push_ans" =~ ^[Yy] ]]; then
    echo "Skipped git push."
    exit 0
  fi
fi

echo -e "Pushing ${BOLD}${BRANCH}${RESET} to origin..."
git push origin "$BRANCH"
if git rev-parse "$TAG_NAME" >/dev/null 2>&1 && [[ "$CREATE_TAG" == "yes" ]]; then
  echo -e "Pushing tag ${BOLD}${TAG_NAME}${RESET} to origin..."
  git push origin "$TAG_NAME"
fi

echo ""
echo -e "${GREEN}Successfully synced with GitHub!${RESET}"
