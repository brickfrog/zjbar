#!/usr/bin/env bash
# setup-gemini-extension.sh — Setup zjbar as a Gemini CLI extension
#
# Since hooks/hooks.json is now Gemini format (shared with Gemini extensions),
# this script simply validates and links the extension for local development.
#
# Remote installation works directly: gemini extensions install https://github.com/imroc/zjbar
#
# Usage: ./scripts/setup-gemini-extension.sh {validate|link|unlink}
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
SOURCE_HOOKS="${REPO_ROOT}/hooks/hooks.json"

# Check gemini-internal availability
if ! command -v gemini-internal &>/dev/null; then
  echo "Error: gemini-internal not found. Please install Gemini CLI first." >&2
  exit 1
fi

# Ensure hooks.json exists
if [ ! -f "$SOURCE_HOOKS" ]; then
  echo "Error: $SOURCE_HOOKS not found" >&2
  exit 1
fi

validate() {
  echo "Validating extension structure at $REPO_ROOT..."
  gemini-internal extensions validate "$REPO_ROOT"
  echo "✓ Extension validation passed"
}

link() {
  echo "Linking zjbar as Gemini extension for local development..."
  
  # Validate first
  validate
  
  # Link extension
  echo "Linking extension from $REPO_ROOT..."
  echo "Y" | gemini-internal extensions link "$REPO_ROOT" 2>&1
  echo "✓ Extension linked successfully"
  echo ""
  echo "Development setup complete. The repo changes will be reflected immediately."
  echo "To unlink, run: $0 unlink"
}

unlink() {
  echo "Unlinking Gemini extension..."
  
  # Unlink extension
  gemini-internal extensions uninstall zjbar 2>/dev/null || true
  echo "✓ Extension unlinked"
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
    echo "  link      - Link extension for local development"
    echo "  unlink    - Unlink extension"
    exit 1
    ;;
esac
