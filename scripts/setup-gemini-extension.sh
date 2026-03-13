#!/usr/bin/env bash
# setup-gemini-extension.sh — Setup zjbar as a Gemini CLI extension
#
# This script handles Gemini extension installation/uninstallation for zjbar.
# It supports two methods:
#   1. Local linking: gemini extensions link /path/to/zjbar (for development)
#   2. Remote install: gemini extensions install https://github.com/imroc/zjbar
#
# For local development, this script temporarily replaces hooks.json with
# the Gemini-compatible version while linking is active.
#
# Usage: ./scripts/setup-gemini-extension.sh {validate|link|unlink}
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
SOURCE_HOOKS_CLAUDE="${REPO_ROOT}/hooks/hooks.json"
SOURCE_HOOKS_GEMINI="${REPO_ROOT}/hooks/hooks-gemini.json"
HOOKS_BACKUP="${REPO_ROOT}/.hooks.json.claude-backup"

# Check gemini-internal availability
if ! command -v gemini-internal &>/dev/null; then
  echo "Error: gemini-internal not found. Please install Gemini CLI first." >&2
  exit 1
fi

# Ensure both hooks files exist
if [ ! -f "$SOURCE_HOOKS_CLAUDE" ]; then
  echo "Error: $SOURCE_HOOKS_CLAUDE not found" >&2
  exit 1
fi

if [ ! -f "$SOURCE_HOOKS_GEMINI" ]; then
  echo "Error: $SOURCE_HOOKS_GEMINI not found" >&2
  exit 1
fi

validate() {
  echo "Validating extension structure at $REPO_ROOT..."
  gemini-internal extensions validate "$REPO_ROOT"
  echo "✓ Extension validation passed"
}

link() {
  echo "Setting up Gemini extension for local development..."
  
  # Backup Claude Code hooks.json if needed
  if [ ! -f "$HOOKS_BACKUP" ]; then
    cp "$SOURCE_HOOKS_CLAUDE" "$HOOKS_BACKUP"
    echo "✓ Backed up Claude Code hooks to $HOOKS_BACKUP"
  fi
  
  # Switch to Gemini hooks
  cp "$SOURCE_HOOKS_GEMINI" "$SOURCE_HOOKS_CLAUDE"
  echo "✓ Switched to Gemini hooks"
  
  # Validate extension structure
  validate
  
  # Link extension
  echo "Linking extension from $REPO_ROOT..."
  echo "Y" | gemini-internal extensions link "$REPO_ROOT" 2>&1
  echo "✓ Extension linked successfully"
  echo ""
  echo "Development setup complete. The repo changes will be reflected immediately."
  echo "To restore Claude Code hooks, run: $0 unlink"
}

unlink() {
  echo "Unlinking Gemini extension and restoring Claude Code hooks..."
  
  # Unlink extension
  gemini-internal extensions uninstall zjbar 2>/dev/null || true
  echo "✓ Extension unlinked"
  
  # Restore Claude Code hooks if backup exists
  if [ -f "$HOOKS_BACKUP" ]; then
    mv "$HOOKS_BACKUP" "$SOURCE_HOOKS_CLAUDE"
    echo "✓ Restored Claude Code hooks"
  fi
}

case "${1:-}" in
  validate)
    validate
    ;;
  link)
    link
    ;;
  unlink)
    unlink
    ;;
  *)
    echo "Usage: $0 {validate|link|unlink}"
    echo ""
    echo "Commands:"
    echo "  validate  - Validate extension structure"
    echo "  link      - Link extension for local development (switches to Gemini hooks)"
    echo "  unlink    - Unlink extension and restore Claude Code hooks"
    exit 1
    ;;
esac

