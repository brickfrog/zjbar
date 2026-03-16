#!/usr/bin/env bash
# install-gemini-hooks.sh — Add zjbar hooks to Gemini CLI settings.json
#
# Copies the hook script and icon to ~/.gemini/zjbar/ so the repo can
# be deleted after installation without breaking the hook.
#
# Usage: ./scripts/install-gemini-hooks.sh [--uninstall]
#        GEMINI_HOME=/path/to/gemini ./scripts/install-gemini-hooks.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
SOURCE_SCRIPT="${SCRIPT_DIR}/zjbar-gemini-hook.sh"
SOURCE_ICON="${REPO_ROOT}/assets/gemini-logo.png"

GEMINI_HOME="${GEMINI_HOME:-$HOME/.gemini}"
SETTINGS="${GEMINI_HOME}/settings.json"
INSTALL_DIR="${GEMINI_HOME}/zjbar"
INSTALLED_SCRIPT="${INSTALL_DIR}/zjbar-gemini-hook.sh"
INSTALLED_ICON="${INSTALL_DIR}/assets/gemini-logo.png"

if [ ! -f "$SOURCE_SCRIPT" ]; then
  echo "Error: Hook script not found at $SOURCE_SCRIPT" >&2
  exit 1
fi

has_our_entry() {
  grep -qF "zjbar-gemini-hook.sh" "$SETTINGS" 2>/dev/null
}

uninstall() {
  if [ ! -f "$SETTINGS" ]; then
    echo "No settings file found at $SETTINGS"
    exit 0
  fi

  if ! has_our_entry; then
    echo "No zjbar hooks found in $SETTINGS"
    return 0
  fi

  # Use python3 to safely remove zjbar hooks from settings.json
  python3 - "$SETTINGS" <<'PYEOF'
import sys, json, os

settings_path = sys.argv[1]

with open(settings_path, 'r') as f:
    settings = json.load(f)

hooks = settings.get('hooks', {})
if not hooks:
    print("No hooks section found")
    sys.exit(0)

# Remove hook entries that contain our script marker
marker = "zjbar-gemini-hook.sh"
modified = False

for event_name in list(hooks.keys()):
    event_hooks = hooks[event_name]
    if not isinstance(event_hooks, list):
        continue

    # Filter out hook groups that reference our script
    filtered = []
    for group in event_hooks:
        if not isinstance(group, dict):
            filtered.append(group)
            continue
        group_hooks = group.get('hooks', [])
        has_our_hook = any(
            marker in h.get('command', '')
            for h in group_hooks
            if isinstance(h, dict)
        )
        if not has_our_hook:
            filtered.append(group)
        else:
            modified = True

    if filtered:
        hooks[event_name] = filtered
    else:
        del hooks[event_name]
        modified = True

if not hooks:
    del settings['hooks']

if modified:
    with open(settings_path, 'w') as f:
        json.dump(settings, f, indent=2, ensure_ascii=False)
        f.write('\n')
    print(f"Removed zjbar hooks from {settings_path}")
else:
    print("No zjbar hooks found to remove")

PYEOF

  # Remove installed files
  if [ -d "$INSTALL_DIR" ]; then
    rm -rf "$INSTALL_DIR"
    echo "Removed $INSTALL_DIR"
  fi

  echo "Uninstalled zjbar Gemini hooks from $SETTINGS"
}

install() {
  # Create settings file if it doesn't exist
  if [ ! -f "$SETTINGS" ]; then
    mkdir -p "$(dirname "$SETTINGS")"
    echo '{}' > "$SETTINGS"
  fi

  # Remove any existing zjbar hooks first (idempotent)
  if has_our_entry; then
    uninstall 2>/dev/null || true
  fi

  # Copy script and assets to persistent directory
  mkdir -p "${INSTALL_DIR}/assets"
  cp "$SOURCE_SCRIPT" "$INSTALLED_SCRIPT"
  chmod +x "$INSTALLED_SCRIPT"
  [ -f "$SOURCE_ICON" ] && cp "$SOURCE_ICON" "$INSTALLED_ICON"
  echo "Copied files to $INSTALL_DIR"

  # Use python3 to add zjbar hooks to settings.json
  python3 - "$SETTINGS" "$INSTALLED_SCRIPT" <<'PYEOF'
import sys, json, os

settings_path = sys.argv[1]
script_path = sys.argv[2]

with open(settings_path, 'r') as f:
    settings = json.load(f)

# Define the Gemini hook events we need
gemini_events = [
    "SessionStart",
    "BeforeAgent",
    "BeforeTool",
    "AfterTool",
    "AfterAgent",
    "SessionEnd",
]

# Build the hook entry for each event
def make_hook_entry(event_name, script):
    return {
        "hooks": [
            {
                "type": "command",
                "command": script,
                "timeout": 5000,
                "env": {
                    "ZJBAR_GEMINI_EVENT": event_name
                }
            }
        ]
    }

hooks = settings.setdefault('hooks', {})

for event in gemini_events:
    entry = make_hook_entry(event, script_path)
    if event in hooks:
        # Append to existing event hooks
        if isinstance(hooks[event], list):
            hooks[event].append(entry)
        else:
            hooks[event] = [entry]
    else:
        hooks[event] = [entry]

with open(settings_path, 'w') as f:
    json.dump(settings, f, indent=2, ensure_ascii=False)
    f.write('\n')

PYEOF

  echo "Installed zjbar Gemini hooks into $SETTINGS"
  echo "Hook script: $INSTALLED_SCRIPT"
  echo "The repo can now be safely deleted."
}

case "${1:-}" in
  --uninstall)
    uninstall
    ;;
  *)
    install
    ;;
esac
