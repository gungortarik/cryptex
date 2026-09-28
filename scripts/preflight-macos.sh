#!/usr/bin/env bash

set -u
set -o pipefail

PASS_COUNT=0
WARN_COUNT=0
FAIL_COUNT=0

pass() {
  printf '[PASS] %s\n' "$1"
  PASS_COUNT=$((PASS_COUNT + 1))
}

warn() {
  printf '[WARN] %s\n' "$1"
  WARN_COUNT=$((WARN_COUNT + 1))
}

fail() {
  printf '[FAIL] %s\n' "$1"
  FAIL_COUNT=$((FAIL_COUNT + 1))
}

info() {
  printf '[INFO] %s\n' "$1"
}

check_command() {
  local command_name="$1"

  if command -v "$command_name" >/dev/null 2>&1; then
    pass "$command_name found: $(command -v "$command_name")"
  else
    fail "$command_name not found"
  fi
}

echo "Cryptex macOS Preflight"
echo "======================"
echo

# Platform
if [[ "$(uname -s)" == "Darwin" ]]; then
  pass "Running on macOS"
else
  fail "This script must run on macOS"
fi

info "macOS version: $(sw_vers -productVersion 2>/dev/null || echo unknown)"
info "macOS build: $(sw_vers -buildVersion 2>/dev/null || echo unknown)"
info "Architecture: $(uname -m)"

case "$(uname -m)" in
  arm64|x86_64)
    pass "Supported CPU architecture detected"
    ;;
  *)
    warn "Unexpected CPU architecture: $(uname -m)"
    ;;
esac

echo
echo "Developer toolchain"
echo "-------------------"

if xcode-select -p >/dev/null 2>&1; then
  pass "Xcode developer tools configured: $(xcode-select -p)"
else
  fail "Xcode developer tools are not configured"
fi

if command -v xcrun >/dev/null 2>&1; then
  info "$(xcrun clang --version 2>/dev/null | head -1)"
else
  fail "xcrun not found"
fi

for cmd in git gh node pnpm rustc cargo rustup; do
  check_command "$cmd"
done

command -v git >/dev/null 2>&1 && info "$(git --version)"
command -v gh >/dev/null 2>&1 && info "$(gh --version | head -1)"
command -v node >/dev/null 2>&1 && info "Node: $(node --version)"
command -v pnpm >/dev/null 2>&1 && info "pnpm: $(pnpm --version)"
command -v rustc >/dev/null 2>&1 && info "$(rustc --version)"
command -v cargo >/dev/null 2>&1 && info "$(cargo --version)"
command -v rustup >/dev/null 2>&1 && info "Rust toolchain: $(rustup show active-toolchain 2>/dev/null || echo unknown)"

if command -v gh >/dev/null 2>&1; then
  if gh auth status >/dev/null 2>&1; then
    pass "GitHub CLI is authenticated"
  else
    warn "GitHub CLI is installed but authentication was not verified"
  fi
fi

echo
echo "Desktop prerequisites"
echo "---------------------"

if [[ -d "/Applications/Cursor.app" || -d "$HOME/Applications/Cursor.app" ]]; then
  pass "Cursor.app found"
else
  fail "Cursor.app not found"
fi

if [[ -d "/Applications/Obsidian.app" || -d "$HOME/Applications/Obsidian.app" ]]; then
  pass "Obsidian.app found"
else
  fail "Obsidian.app not found"
fi

if command -v syncthing >/dev/null 2>&1; then
  pass "Syncthing found: $(command -v syncthing)"
  info "$(syncthing --version 2>/dev/null | head -1)"
elif [[ -d "/Applications/Syncthing.app" || -d "$HOME/Applications/Syncthing.app" ]]; then
  pass "Syncthing application found"
else
  fail "Syncthing baseline not found"
fi

echo
echo "Device protection"
echo "-----------------"

if command -v fdesetup >/dev/null 2>&1; then
  FILEVAULT_STATUS="$(fdesetup status 2>/dev/null || true)"
  info "$FILEVAULT_STATUS"

  if printf '%s' "$FILEVAULT_STATUS" | grep -qi "FileVault is On"; then
    pass "FileVault is enabled"
  else
    warn "FileVault is not reported as enabled"
  fi
else
  warn "Unable to query FileVault status"
fi

echo
echo "Filesystem"
echo "----------"

APP_SUPPORT="$HOME/Library/Application Support"

if [[ -d "$APP_SUPPORT" && -w "$APP_SUPPORT" ]]; then
  TEST_FILE="$(mktemp "$APP_SUPPORT/.cryptex-preflight.XXXXXX" 2>/dev/null || true)"

  if [[ -n "$TEST_FILE" && -f "$TEST_FILE" ]]; then
    rm -f "$TEST_FILE"
    pass "Application Support is writable"
  else
    fail "Application Support write test failed"
  fi
else
  fail "Application Support is not writable"
fi

echo
echo "Repository"
echo "----------"

if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  REPO_ROOT="$(git rev-parse --show-toplevel)"
  pass "Git repository detected: $REPO_ROOT"

  if [[ -f "$REPO_ROOT/AGENTS.md" && -f "$REPO_ROOT/docs/00_MASTER_SPEC.md" ]]; then
    pass "Cryptex authority files found"
  else
    fail "Expected Cryptex authority files not found"
  fi

  ORIGIN_URL="$(git remote get-url origin 2>/dev/null || true)"
  if [[ "$ORIGIN_URL" == *"gungortarik/cryptex"* ]]; then
    pass "Expected Git origin configured"
  else
    warn "Unexpected or missing Git origin: ${ORIGIN_URL:-none}"
  fi
else
  warn "Script was not run from inside a Git repository"
fi

if command -v brew >/dev/null 2>&1; then
  info "Homebrew: $(brew --version | head -1)"
else
  info "Homebrew not found (optional; not a Cryptex runtime dependency)"
fi

echo
echo "Summary"
echo "-------"
printf 'PASS: %d\n' "$PASS_COUNT"
printf 'WARN: %d\n' "$WARN_COUNT"
printf 'FAIL: %d\n' "$FAIL_COUNT"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
  exit 1
fi

exit 0
