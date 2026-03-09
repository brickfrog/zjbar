#!/usr/bin/env bash
# zjbar-hook.sh — Claude Code hook → zellij pipe bridge
# Forwards hook events to the zjbar Zellij plugin via pipe.
#
# Usage in ~/.claude/settings.json hooks:
#   "command": "/path/to/zjbar-hook.sh"

# Exit silently if not running inside Zellij
[ -z "$ZELLIJ_SESSION_NAME" ] && exit 0
[ -z "$ZELLIJ_PANE_ID" ] && exit 0

# Read hook JSON from stdin
INPUT=$(cat)

# Extract fields with jq (required dependency)
HOOK_EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty')
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // empty')
CWD=$(echo "$INPUT" | jq -r '.cwd // empty')

[ -z "$HOOK_EVENT" ] && exit 0

# Build compact JSON payload
PAYLOAD=$(jq -nc \
  --arg pane_id "$ZELLIJ_PANE_ID" \
  --arg session_id "$SESSION_ID" \
  --arg hook_event "$HOOK_EVENT" \
  --arg tool_name "$TOOL_NAME" \
  --arg cwd "$CWD" \
  --arg zellij_session "$ZELLIJ_SESSION_NAME" \
  --arg term_program "${TERM_PROGRAM:-}" \
  '{
    pane_id: ($pane_id | tonumber),
    session_id: $session_id,
    hook_event: $hook_event,
    tool_name: (if $tool_name == "" then null else $tool_name end),
    cwd: (if $cwd == "" then null else $cwd end),
    zellij_session: $zellij_session,
    term_program: (if $term_program == "" then null else $term_program end)
  }')

# Desktop notification + bell
# Default notify events: PermissionRequest, Notification, Stop
# Override via ~/.config/zellij/plugins/zjbar.json:
#   { "notify_events": ["PermissionRequest", "Stop"], "notifications": "always" }
SETTINGS_FILE="$HOME/.config/zellij/plugins/zjbar.json"
DEFAULT_NOTIFY_EVENTS="PermissionRequest Notification Stop"
NOTIFY_EVENTS="$DEFAULT_NOTIFY_EVENTS"
NOTIFY_MODE="always"

if [ -f "$SETTINGS_FILE" ]; then
  CUSTOM_EVENTS=$(jq -r '.notify_events // empty | join(" ")' "$SETTINGS_FILE" 2>/dev/null)
  [ -n "$CUSTOM_EVENTS" ] && NOTIFY_EVENTS="$CUSTOM_EVENTS"
  CUSTOM_MODE=$(jq -r '.notifications // empty' "$SETTINGS_FILE" 2>/dev/null)
  [ -n "$CUSTOM_MODE" ] && NOTIFY_MODE=$(echo "$CUSTOM_MODE" | tr '[:upper:]' '[:lower:]')
fi

# Check if current event is in the notify list
IS_NOTIFY_EVENT=false
for EVT in $NOTIFY_EVENTS; do
  [ "$HOOK_EVENT" = "$EVT" ] && { IS_NOTIFY_EVENT=true; break; }
done

if [ "$IS_NOTIFY_EVENT" = true ]; then
  # Bell for PermissionRequest
  [ "$HOOK_EVENT" = "PermissionRequest" ] && printf '\a' > /dev/tty 2>/dev/null || true

  # Build notification title and message per event type
  case "$HOOK_EVENT" in
    PermissionRequest)
      TOOL_SUFFIX=""
      [ -n "$TOOL_NAME" ] && TOOL_SUFFIX=" — $TOOL_NAME"
      TITLE="⚠ Claude Code"
      MESSAGE="Permission requested${TOOL_SUFFIX}"
      ;;
    Notification)
      TITLE="Claude Code"
      MESSAGE="Notification received"
      ;;
    Stop)
      TITLE="✅ Claude Code"
      MESSAGE="Task completed"
      ;;
    *)
      TITLE="Claude Code"
      MESSAGE="Event: $HOOK_EVENT"
      ;;
  esac

  # For "unfocused" mode, check if the terminal app is frontmost
  SHOULD_NOTIFY=false
  case "$NOTIFY_MODE" in
    always) SHOULD_NOTIFY=true ;;
    unfocused)
      TERM_FOCUSED=false
      case "$(uname)" in
        Darwin)
          # Map TERM_PROGRAM to macOS process name
          EXPECTED="${TERM_PROGRAM:-}"
          case "$EXPECTED" in
            Apple_Terminal) EXPECTED="Terminal" ;;
            iTerm.app)     EXPECTED="iTerm2" ;;
          esac
          FRONT_APP=$(osascript -e 'tell application "System Events" to get name of first application process whose frontmost is true' 2>/dev/null)
          [ "$FRONT_APP" = "$EXPECTED" ] && TERM_FOCUSED=true
          ;;
        Linux)
          # X11: check if focused window belongs to our terminal
          if command -v xdotool >/dev/null 2>&1; then
            ACTIVE_PID=$(xdotool getactivewindow getwindowpid 2>/dev/null)
            if [ -n "$ACTIVE_PID" ]; then
              # Walk up the process tree from our shell to see if the
              # focused window's process is an ancestor (i.e. our terminal)
              PID=$$
              while [ "$PID" -gt 1 ] 2>/dev/null; do
                [ "$PID" = "$ACTIVE_PID" ] && { TERM_FOCUSED=true; break; }
                PID=$(ps -o ppid= -p "$PID" 2>/dev/null | tr -d ' ')
              done
            fi
          fi
          # Wayland: no standard way to check; fall through to not-focused
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
      echo "$NOW" > "$LOCK"

      # Click callback: activate terminal + focus the pane
      ZELLIJ_BIN=$(command -v zellij)
      FOCUS_CMD="${ZELLIJ_BIN} -s '${ZELLIJ_SESSION_NAME}' pipe --name zjbar:focus -- ${ZELLIJ_PANE_ID}"

      case "$(uname)" in
        Darwin)
          [ -n "${TERM_PROGRAM:-}" ] && FOCUS_CMD="open -a '${TERM_PROGRAM}' && ${FOCUS_CMD}"
          if command -v terminal-notifier >/dev/null 2>&1; then
            terminal-notifier \
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
fi

# Send to plugin
# Use -s flag to specify session explicitly because Claude Code sets ZELLIJ=0
# which breaks the default IPC path detection.
zellij -s "$ZELLIJ_SESSION_NAME" pipe --name "zjbar" -- "$PAYLOAD"
