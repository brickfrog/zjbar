#!/usr/bin/env bash
# install-codex-hooks.sh — Add zjbar notify entry to Codex config.toml
#
# Usage: ./scripts/install-codex-hooks.sh [--uninstall]
#        CODEX_CONFIG=/path/to/config.toml ./scripts/install-codex-hooks.sh
set -euo pipefail

NOTIFY_SCRIPT="$(cd "$(dirname "$0")" && pwd)/zjbar-codex-notify.sh"
CONFIG="${CODEX_CONFIG:-$HOME/.codex/config.toml}"

if [ ! -f "$NOTIFY_SCRIPT" ]; then
  echo "Error: Notify script not found at $NOTIFY_SCRIPT" >&2
  exit 1
fi

# Use fixed-string grep (-F) to avoid regex metacharacter issues (e.g. dots in paths)
has_our_entry() {
  grep -qF "$NOTIFY_SCRIPT" "$CONFIG" 2>/dev/null
}

uninstall() {
  if [ ! -f "$CONFIG" ]; then
    echo "No config file found at $CONFIG"
    exit 0
  fi

  if ! has_our_entry; then
    echo "No zjbar notify entry found in $CONFIG"
    return 0
  fi

  # Use python3 for safe TOML manipulation — avoids sed/grep edge cases
  python3 - "$CONFIG" "$NOTIFY_SCRIPT" "remove" <<'PYEOF'
import sys, os

config_path = sys.argv[1]
script_path = sys.argv[2]

with open(config_path, 'r') as f:
    lines = f.readlines()

# Find the notify line containing our script
new_lines = []
removed = False
for line in lines:
    stripped = line.strip()
    # Match lines that start with "notify" and contain our script path
    if stripped.startswith('notify') and '=' in stripped and script_path in stripped:
        # Parse the array value to remove only our entry
        eq_pos = stripped.index('=')
        prefix = stripped[:eq_pos + 1].strip()
        value_part = stripped[eq_pos + 1:].strip()

        if value_part.startswith('[') and value_part.endswith(']'):
            # Single-line array: parse entries
            inner = value_part[1:-1].strip()
            if inner:
                # Split by comma, preserving quoted strings
                entries = []
                current = ''
                in_quote = False
                for ch in inner:
                    if ch == '"' and (not current or current[-1] != '\\'):
                        in_quote = not in_quote
                    if ch == ',' and not in_quote:
                        entries.append(current.strip())
                        current = ''
                    else:
                        current += ch
                if current.strip():
                    entries.append(current.strip())

                # Remove entries containing our script path
                filtered = [e for e in entries if script_path not in e]

                if filtered:
                    new_lines.append(f'notify = [{", ".join(filtered)}]\n')
                else:
                    # All entries removed — drop the entire line
                    pass
                removed = True
            else:
                # Empty array, just drop it
                removed = True
        else:
            # Not a standard single-line array, skip this line entirely
            removed = True
    else:
        new_lines.append(line)

if removed:
    with open(config_path, 'w') as f:
        f.writelines(new_lines)

sys.exit(0 if removed else 1)
PYEOF

  echo "Uninstalled zjbar notify from $CONFIG"
}

install() {
  # Create config file if it doesn't exist
  if [ ! -f "$CONFIG" ]; then
    mkdir -p "$(dirname "$CONFIG")"
    touch "$CONFIG"
  fi

  # Remove any existing zjbar entry first (idempotent)
  if has_our_entry; then
    uninstall 2>/dev/null || true
  fi

  # Use python3 for safe TOML manipulation
  python3 - "$CONFIG" "$NOTIFY_SCRIPT" "add" <<'PYEOF'
import sys, os

config_path = sys.argv[1]
script_path = sys.argv[2]

with open(config_path, 'r') as f:
    content = f.read()

lines = content.splitlines(keepends=True)
if lines and not lines[-1].endswith('\n'):
    lines[-1] += '\n'

# Check if a notify line already exists (user has other notify scripts)
notify_idx = None
for i, line in enumerate(lines):
    stripped = line.strip()
    if stripped.startswith('notify') and '=' in stripped:
        eq_pos = stripped.index('=')
        key = stripped[:eq_pos].strip()
        if key == 'notify':
            notify_idx = i
            break

quoted_path = f'"{script_path}"'

if notify_idx is not None:
    # Append our script to existing notify array
    line = lines[notify_idx]
    stripped = line.strip()
    eq_pos = stripped.index('=')
    value_part = stripped[eq_pos + 1:].strip()

    if value_part.startswith('[') and value_part.endswith(']'):
        inner = value_part[1:-1].strip()
        if inner:
            # Already has entries — append ours
            new_value = f'notify = [{inner}, {quoted_path}]'
        else:
            # Empty array
            new_value = f'notify = [{quoted_path}]'
        lines[notify_idx] = new_value + '\n'
    else:
        # Non-array value (shouldn't happen, but handle gracefully)
        lines[notify_idx] = f'notify = [{quoted_path}]\n'
else:
    # No existing notify — insert as top-level key (before first [section])
    new_line = f'notify = [{quoted_path}]\n'

    first_section = None
    for i, line in enumerate(lines):
        if line.strip().startswith('['):
            first_section = i
            break

    if first_section is not None:
        lines.insert(first_section, new_line)
    else:
        lines.append(new_line)

with open(config_path, 'w') as f:
    f.writelines(lines)
PYEOF

  echo "Installed zjbar notify into $CONFIG"
  echo "Notify script: $NOTIFY_SCRIPT"
}

case "${1:-}" in
  --uninstall)
    uninstall
    ;;
  *)
    install
    ;;
esac
