---
description: Download zjbar WASM plugin and install Zellij layouts
allowed-tools: Bash
---

# Install zjbar Zellij Plugin

Download the zjbar WASM plugin from GitHub Releases and install Zellij layouts.

## Install

```bash
bash -c '
set -e

PLUGIN_DIR="$HOME/.config/zellij/plugins"
LAYOUT_DIR="$HOME/.config/zellij/layouts"
BASE_URL="https://raw.githubusercontent.com/imroc/zjbar/main"

# Install WASM plugin
mkdir -p "$PLUGIN_DIR"
echo "Downloading zjbar.wasm..."
if curl -fsSL "https://github.com/imroc/zjbar/releases/download/v1.1.1/zjbar.wasm" -o "$PLUGIN_DIR/zjbar.wasm"; then
  echo "Installed: $PLUGIN_DIR/zjbar.wasm"
else
  echo "Error: failed to download zjbar.wasm" >&2
  exit 1
fi

# Install notification icons
echo "Downloading notification icons..."
curl -fsSL "$BASE_URL/assets/claude-logo.png" -o "$PLUGIN_DIR/claude-logo.png" || true
curl -fsSL "$BASE_URL/assets/codebuddy-logo.png" -o "$PLUGIN_DIR/codebuddy-logo.png" || true
curl -fsSL "$BASE_URL/assets/opencode-logo.png" -o "$PLUGIN_DIR/opencode-logo.png" || true

# Install layout files
mkdir -p "$LAYOUT_DIR"
echo "Downloading layouts..."
curl -fsSL "$BASE_URL/layout.kdl" -o "$LAYOUT_DIR/zjbar.kdl"
curl -fsSL "$BASE_URL/layout.swap.kdl" -o "$LAYOUT_DIR/zjbar.swap.kdl"
echo "Installed layouts: $LAYOUT_DIR/zjbar.kdl"

echo ""
echo "Done! Start zellij with: zellij --layout zjbar"
'
```
