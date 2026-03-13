#!/usr/bin/env bash
# install-gemini-hooks.sh — Add zjbar hooks to Gemini CLI settings.json
#
# Copies the hook script and icon to $GEMINI_HOME/zjbar/ so the repo can
# be deleted after installation without breaking the hooks.
#
# Usage: ./scripts/install-gemini-hooks.sh [--uninstall]
#        GEMINI_HOME=/path/to/gemini ./scripts/install-gemini-hooks.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
SOURCE_SCRIPT="${SCRIPT_DIR}/zjbar-gemini-hook.sh"
SOURCE_ICON="${REPO_ROOT}/assets/gemini-logo.png"

GEMINI_HOME="${GEMINI_HOME:-$HOME/.gemini}"
CONFIG="${GEMINI_HOME}/settings.json"
INSTALL_DIR="${GEMINI_HOME}/zjbar"
INSTALLED_SCRIPT="${INSTALL_DIR}/zjbar-gemini-hook.sh"
INSTALLED_ICON="${INSTALL_DIR}/assets/gemini-logo.png"

if [ ! -f "$SOURCE_SCRIPT" ]; then
  echo "Error: Hook script not found at $SOURCE_SCRIPT" >&2
  exit 1
fi

# Gemini CLI hook events we want to register
HOOK_EVENTS="SessionStart SessionEnd BeforeAgent AfterAgent BeforeTool AfterTool"

has_our_entry() {
  grep -qF "zjbar-gemini-hook.sh" "$CONFIG" 2>/dev/null
}

uninstall() {
  if [ ! -f "$CONFIG" ]; then
    echo "No config file found at $CONFIG"
    exit 0
  fi

  if ! has_our_entry; then
    echo "No zjbar hooks found in $CONFIG"
    # Still remove installed files if they exist
    if [ -d "$INSTALL_DIR" ]; then
      rm -rf "$INSTALL_DIR"
      echo "Removed $INSTALL_DIR"
    fi
    return 0
  fi

  # Use python3 for safe JSON manipulation
  python3 - "$CONFIG" <<'PYEOF'
import sys, json, os

config_path = sys.argv[1]

with open(config_path, 'r') as f:
    try:
        config = json.load(f)
    except json.JSONDecodeError:
        print(f"Warning: Could not parse {config_path}", file=sys.stderr)
        sys.exit(1)

hooks = config.get("hooks", {})
modified = False

# Remove all hook entries that reference our script
for event in list(hooks.keys()):
    if event == "disabled":
        continue
    entries = hooks[event]
    if not isinstance(entries, list):
        continue
    new_entries = []
    for entry in entries:
        entry_hooks = entry.get("hooks", [])
        filtered = [h for h in entry_hooks if "zjbar-gemini-hook.sh" not in h.get("command", "")]
        if filtered:
            entry["hooks"] = filtered
            new_entries.append(entry)
        else:
            modified = True
    if new_entries:
        hooks[event] = new_entries
    elif entries:  # Was non-empty, now empty
        del hooks[event]
        modified = True

if not hooks or hooks == {}:
    config.pop("hooks", None)
else:
    config["hooks"] = hooks

if modified:
    with open(config_path, 'w') as f:
        json.dump(config, f, indent=2)
        f.write('\n')
    print(f"Removed zjbar hooks from {config_path}")
else:
    print(f"No zjbar hooks found in {config_path}")
PYEOF

  # Remove installed files
  if [ -d "$INSTALL_DIR" ]; then
    rm -rf "$INSTALL_DIR"
    echo "Removed $INSTALL_DIR"
  fi

  echo "Uninstalled zjbar hooks from Gemini CLI"
}

install() {
  # Create config file if it doesn't exist
  if [ ! -f "$CONFIG" ]; then
    mkdir -p "$(dirname "$CONFIG")"
    echo '{}' > "$CONFIG"
  fi

  # Remove any existing zjbar entries first (idempotent)
  if has_our_entry; then
    uninstall 2>/dev/null || true
  fi

  # Copy script and assets to persistent directory
  mkdir -p "${INSTALL_DIR}/assets"
  cp "$SOURCE_SCRIPT" "$INSTALLED_SCRIPT"
  chmod +x "$INSTALLED_SCRIPT"
  [ -f "$SOURCE_ICON" ] && cp "$SOURCE_ICON" "$INSTALLED_ICON"
  echo "Copied files to $INSTALL_DIR"

  # Use python3 to inject hooks into settings.json
  python3 - "$CONFIG" "$INSTALLED_SCRIPT" "$HOOK_EVENTS" <<'PYEOF'
import sys, json, os

config_path = sys.argv[1]
script_path = sys.argv[2]
events = sys.argv[3].split()

with open(config_path, 'r') as f:
    content = f.read().strip()
    if not content:
        content = '{}'
    try:
        config = json.loads(content)
    except json.JSONDecodeError:
        # Try stripping comments (Gemini settings may have // comments)
        import re
        cleaned = re.sub(r'//.*?$', '', content, flags=re.MULTILINE)
        # Remove trailing commas before closing braces/brackets
        cleaned = re.sub(r',\s*([}\]])', r'\1', cleaned)
        config = json.loads(cleaned)

hooks = config.setdefault("hooks", {})

for event in events:
    # Build the wrapper command that sets ZJBAR_GEMINI_EVENT env var
    command = f"ZJBAR_GEMINI_EVENT={event} {script_path}"

    # Determine matcher: tool events use "*", lifecycle events use "startup"/"exit"
    if event in ("BeforeTool", "AfterTool"):
        matcher = "*"
    elif event == "SessionStart":
        matcher = "startup"
    elif event == "SessionEnd":
        matcher = "exit"
    else:
        matcher = "*"

    hook_entry = {
        "matcher": matcher,
        "hooks": [
            {
                "name": f"zjbar-{event.lower()}",
                "type": "command",
                "command": command,
                "timeout": 5000
            }
        ]
    }

    if event not in hooks:
        hooks[event] = []
    hooks[event].append(hook_entry)

with open(config_path, 'w') as f:
    json.dump(config, f, indent=2)
    f.write('\n')

print(f"Added zjbar hooks for: {', '.join(events)}")
PYEOF

  echo "Installed zjbar hooks into $CONFIG"
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
