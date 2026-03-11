#!/usr/bin/env bash
# install-opencode.sh — Install zjbar OpenCode plugin
#
# Usage: ./scripts/install-opencode.sh [--uninstall]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PLUGIN_SRC="$SCRIPT_DIR/zjbar-opencode-plugin.js"
ICON_SRC="$SCRIPT_DIR/../assets/opencode-logo.png"

PLUGIN_DST="$HOME/.config/opencode/plugins/zjbar-opencode-plugin.js"
ICON_DST="$HOME/.config/zellij/plugins/opencode-logo.png"

uninstall() {
  local removed=false
  if [ -f "$PLUGIN_DST" ]; then
    rm "$PLUGIN_DST"
    echo "Removed $PLUGIN_DST"
    removed=true
  fi
  if [ -f "$ICON_DST" ]; then
    rm "$ICON_DST"
    echo "Removed $ICON_DST"
    removed=true
  fi
  if [ "$removed" = false ]; then
    echo "Nothing to uninstall."
  else
    echo "Uninstalled zjbar OpenCode plugin."
  fi
}

install() {
  if [ ! -f "$PLUGIN_SRC" ]; then
    echo "Error: Plugin not found at $PLUGIN_SRC" >&2
    exit 1
  fi

  mkdir -p "$(dirname "$PLUGIN_DST")"
  mkdir -p "$(dirname "$ICON_DST")"

  cp "$PLUGIN_SRC" "$PLUGIN_DST"
  echo "Installed plugin: $PLUGIN_DST"

  if [ -f "$ICON_SRC" ]; then
    cp "$ICON_SRC" "$ICON_DST"
    echo "Installed icon:   $ICON_DST"
  fi

  echo ""
  echo "Done! zjbar will now display OpenCode activity in your Zellij status bar."
}

case "${1:-}" in
  --uninstall)
    uninstall
    ;;
  *)
    install
    ;;
esac
