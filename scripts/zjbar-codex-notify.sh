#!/usr/bin/env bash
# zjbar-codex-notify.sh — Codex CLI notify → zellij pipe bridge
# Receives Codex "agent-turn-complete" notifications and forwards them
# to the zjbar Zellij plugin as Stop events.
#
# Usage in ~/.codex/config.toml:
#   notify = ["/path/to/zjbar-codex-notify.sh"]
#
# Codex passes a single JSON argument via $1 with fields:
#   type, thread-id, turn-id, cwd, input-messages, last-assistant-message

# Exit silently if not running inside Zellij
[ -z "$ZELLIJ_SESSION_NAME" ] && exit 0
[ -z "$ZELLIJ_PANE_ID" ] && exit 0

# Read JSON from first argument (Codex passes notification as $1)
INPUT="${1:-}"
[ -z "$INPUT" ] && exit 0

# Extract fields with jq
EVENT_TYPE=$(echo "$INPUT" | jq -r '.type // ""' 2>/dev/null)

# Only handle agent-turn-complete events
[ "$EVENT_TYPE" != "agent-turn-complete" ] && exit 0

# Extract notification details
eval "$(echo "$INPUT" | jq -r '
  @sh "THREAD_ID=\(."thread-id" // "")",
  @sh "TURN_ID=\(."turn-id" // "")",
  @sh "CWD=\(.cwd // "")",
  @sh "LAST_MESSAGE=\(."last-assistant-message" // "")"
')"

# Resolve plugin root (for notification icon)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PLUGIN_ROOT="$(dirname "$SCRIPT_DIR")"

# Build zjbar JSON payload (maps to Stop event)
PAYLOAD=$(jq -nc \
  --arg source "codex" \
  --arg pane_id "$ZELLIJ_PANE_ID" \
  --arg session_id "$THREAD_ID" \
  --arg hook_event "Stop" \
  --arg cwd "$CWD" \
  --arg zellij_session "$ZELLIJ_SESSION_NAME" \
  --arg term_program "${TERM_PROGRAM:-}" \
  '{
    source: $source,
    pane_id: ($pane_id | tonumber),
    session_id: $session_id,
    hook_event: $hook_event,
    tool_name: null,
    cwd: (if $cwd == "" then null else $cwd end),
    zellij_session: $zellij_session,
    term_program: (if $term_program == "" then null else $term_program end)
  }')

# -- Desktop notification --
SETTINGS_FILE="$HOME/.config/zellij/plugins/zjbar.json"
NOTIFY_MODE="always"

if [ -f "$SETTINGS_FILE" ]; then
  CUSTOM_MODE=$(jq -r '.notifications // empty' "$SETTINGS_FILE" 2>/dev/null)
  [ -n "$CUSTOM_MODE" ] && NOTIFY_MODE=$(echo "$CUSTOM_MODE" | tr '[:upper:]' '[:lower:]')
fi

# Build notification summary from last-assistant-message
SUMMARY=""
if [ -n "$LAST_MESSAGE" ]; then
  # Strip markdown formatting
  SUMMARY=$(echo "$LAST_MESSAGE" | sed -E 's/\*\*//g; s/\*//g; s/`//g; s/^#+ //; s/\[([^]]*)\]\([^)]*\)/\1/g' | tr '\n' ' ' | sed 's/  */ /g')
  # Truncate to 120 chars at word boundary
  if [ ${#SUMMARY} -gt 120 ]; then
    SUMMARY="${SUMMARY:0:117}"
    SUMMARY="${SUMMARY% *}..."
  fi
fi

TITLE="✅ Codex"
MESSAGE="${SUMMARY:-Task completed}"

# -- Stop debounce --
# Codex may fire multiple turn-complete events in quick succession.
STOP_DEBOUNCE_SECS=3
DEBOUNCE_PID_FILE="/tmp/zjbar-stop-debounce-${ZELLIJ_PANE_ID}.pid"

cancel_stop_debounce() {
  if [ -f "$DEBOUNCE_PID_FILE" ]; then
    local old_pid
    old_pid=$(cat "$DEBOUNCE_PID_FILE" 2>/dev/null)
    if [ -n "$old_pid" ]; then
      kill "$old_pid" 2>/dev/null
      wait "$old_pid" 2>/dev/null
    fi
    rm -f "$DEBOUNCE_PID_FILE"
  fi
}

cancel_stop_debounce

# Schedule debounced notification in background
(
  sleep "$STOP_DEBOUNCE_SECS"
  rm -f "$DEBOUNCE_PID_FILE"

  SHOULD_NOTIFY=false
  case "$NOTIFY_MODE" in
  always) SHOULD_NOTIFY=true ;;
  unfocused)
    TERM_FOCUSED=false
    case "$(uname)" in
    Darwin)
      EXPECTED="${TERM_PROGRAM:-}"
      case "$EXPECTED" in
      Apple_Terminal) EXPECTED="Terminal" ;;
      iTerm.app) EXPECTED="iTerm2" ;;
      esac
      FRONT_APP=$(osascript -e 'tell application "System Events" to get name of first application process whose frontmost is true' 2>/dev/null)
      [ "$FRONT_APP" = "$EXPECTED" ] && TERM_FOCUSED=true
      ;;
    Linux)
      if command -v xdotool >/dev/null 2>&1; then
        ACTIVE_PID=$(xdotool getactivewindow getwindowpid 2>/dev/null)
        if [ -n "$ACTIVE_PID" ]; then
          PID=$$
          while [ "$PID" -gt 1 ] 2>/dev/null; do
            [ "$PID" = "$ACTIVE_PID" ] && { TERM_FOCUSED=true; break; }
            PID=$(ps -o ppid= -p "$PID" 2>/dev/null | tr -d ' ')
          done
        fi
      fi
      ;;
    esac
    [ "$TERM_FOCUSED" = false ] && SHOULD_NOTIFY=true
    ;;
  esac

  if [ "$SHOULD_NOTIFY" = true ]; then
    # Rate-limit: one notification per pane per 10 seconds
    LOCK="/tmp/zjbar-notify-${ZELLIJ_PANE_ID}"
    NOW=$(date +%s)
    LAST=0
    [ -f "$LOCK" ] && LAST=$(cat "$LOCK" 2>/dev/null)
    if [ $((NOW - LAST)) -ge 10 ]; then
      echo "$NOW" >"$LOCK"

      ZELLIJ_BIN=$(command -v zellij)
      FOCUS_CMD="${ZELLIJ_BIN} -s '${ZELLIJ_SESSION_NAME}' pipe --name zjbar:focus -- ${ZELLIJ_PANE_ID}"

      case "$(uname)" in
      Darwin)
        [ -n "${TERM_PROGRAM:-}" ] && FOCUS_CMD="open -a '${TERM_PROGRAM}' && ${FOCUS_CMD}"
        ICON_PATH="${PLUGIN_ROOT}/assets/codex-logo.png"
        ICON_FLAG=()
        [ -f "$ICON_PATH" ] && ICON_FLAG=(-contentImage "$ICON_PATH")
        if command -v terminal-notifier >/dev/null 2>&1; then
          terminal-notifier \
            "${ICON_FLAG[@]}" \
            -title "$TITLE" \
            -message "$MESSAGE" \
            -execute "$FOCUS_CMD" &
        else
          osascript -e "display notification \"$MESSAGE\" with title \"$TITLE\"" &
        fi
        ;;
      Linux)
        if command -v notify-send >/dev/null 2>&1; then
          notify-send "$TITLE" "$MESSAGE" &
        fi
        ;;
      esac
    fi
  fi
) &
echo $! >"$DEBOUNCE_PID_FILE"

# Send to plugin (fire-and-forget)
zellij -s "$ZELLIJ_SESSION_NAME" pipe --name "zjbar" -- "$PAYLOAD" &
