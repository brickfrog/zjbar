#!/usr/bin/env bash
# zjbar-gemini-hook.sh — Gemini CLI hook → zellij pipe bridge
# Forwards hook events to the zjbar Zellij plugin via pipe.
#
# Gemini CLI hooks MUST output valid JSON to stdout.
# All debug output goes to stderr only.
#
# Usage in ~/.gemini/settings.json hooks:
#   "command": "/path/to/zjbar-gemini-hook.sh"

# Exit silently if not running inside Zellij
if [ -z "$ZELLIJ_SESSION_NAME" ] || [ -z "$ZELLIJ_PANE_ID" ]; then
  echo '{}' # Gemini requires JSON stdout
  exit 0
fi

# Read hook JSON from stdin
INPUT=$(cat)
[ -z "$INPUT" ] && { echo '{}'; exit 0; }

# Detect which Gemini hook event fired.
# First try the hook_event_name field from Gemini's stdin JSON,
# then fall back to the ZJBAR_GEMINI_EVENT env var set by the wrapper.
HOOK_EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // ""' 2>/dev/null)
[ -z "$HOOK_EVENT" ] && HOOK_EVENT="${ZJBAR_GEMINI_EVENT:-}"
[ -z "$HOOK_EVENT" ] && { echo '{}'; exit 0; }

# Extract common fields from Gemini hook input
eval "$(echo "$INPUT" | jq -r '
  @sh "SESSION_ID=\(.session_id // "")",
  @sh "GEMINI_CWD_INPUT=\(.cwd // "")",
  @sh "TOOL_NAME=\(.tool_name // "")",
  @sh "PROMPT_RESPONSE=\(.prompt_response // "")"
' 2>/dev/null)" || true

# Use cwd from input, fall back to env vars
EFFECTIVE_CWD="${GEMINI_CWD_INPUT:-${GEMINI_CWD:-${GEMINI_PROJECT_DIR:-}}}"

# Map Gemini tool names to zjbar-standard tool names
map_tool_name() {
  local t="$1"
  case "$t" in
    run_shell_command)   echo "Bash" ;;
    read_file|read_many_files) echo "Read" ;;
    write_file)          echo "Write" ;;
    edit_file|replace)   echo "Edit" ;;
    web_fetch)           echo "WebFetch" ;;
    google_web_search)   echo "WebSearch" ;;
    save_memory)         echo "Task" ;;
    glob)                echo "Glob" ;;
    grep)                echo "Grep" ;;
    list_directory)      echo "Read" ;;
    *)                   echo "$t" ;;
  esac
}

# Map Gemini hook events to zjbar hook events
ZJBAR_EVENT=""
ZJBAR_TOOL=""
case "$HOOK_EVENT" in
  SessionStart)   ZJBAR_EVENT="SessionStart" ;;
  SessionEnd)     ZJBAR_EVENT="SessionEnd" ;;
  BeforeAgent)    ZJBAR_EVENT="UserPromptSubmit" ;;
  AfterAgent)     ZJBAR_EVENT="Stop" ;;
  BeforeTool)
    ZJBAR_EVENT="PreToolUse"
    ZJBAR_TOOL=$(map_tool_name "$TOOL_NAME")
    ;;
  AfterTool)
    ZJBAR_EVENT="PostToolUse"
    ZJBAR_TOOL=$(map_tool_name "$TOOL_NAME")
    ;;
  *)
    # Unknown event — pass through as-is
    ZJBAR_EVENT="$HOOK_EVENT"
    ;;
esac

[ -z "$ZJBAR_EVENT" ] && { echo '{}'; exit 0; }

# Build compact JSON payload for zjbar
PAYLOAD=$(jq -nc \
  --arg source "gemini" \
  --arg pane_id "$ZELLIJ_PANE_ID" \
  --arg session_id "$SESSION_ID" \
  --arg hook_event "$ZJBAR_EVENT" \
  --arg tool_name "$ZJBAR_TOOL" \
  --arg cwd "$EFFECTIVE_CWD" \
  --arg zellij_session "$ZELLIJ_SESSION_NAME" \
  --arg term_program "${TERM_PROGRAM:-}" \
  '{
    source: $source,
    pane_id: ($pane_id | tonumber),
    session_id: $session_id,
    hook_event: $hook_event,
    tool_name: (if $tool_name == "" then null else $tool_name end),
    cwd: (if $cwd == "" then null else $cwd end),
    zellij_session: $zellij_session,
    term_program: (if $term_program == "" then null else $term_program end)
  }')

# -- Summary extraction --
# Gemini AfterAgent provides prompt_response directly in the hook input.
# Clean markdown and truncate for desktop notification.
extract_summary() {
  local text="$1"
  [ -z "$text" ] && return

  # Strip markdown formatting
  text=$(echo "$text" | sed -E 's/\*\*//g; s/\*//g; s/`//g; s/^#+ //; s/\[([^]]*)\]\([^)]*\)/\1/g' | tr '\n' ' ' | sed 's/  */ /g')
  # Truncate to 120 chars at word boundary
  if [ ${#text} -gt 120 ]; then
    text="${text:0:117}"
    text="${text% *}..."
  fi
  echo "$text"
}

# -- Desktop notification --
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

IS_NOTIFY_EVENT=false
for EVT in $NOTIFY_EVENTS; do
  [ "$ZJBAR_EVENT" = "$EVT" ] && { IS_NOTIFY_EVENT=true; break; }
done

# Resolve script directory for notification icon
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
if [ -d "${SCRIPT_DIR}/assets" ]; then
  ICON_DIR="${SCRIPT_DIR}/assets"
else
  ICON_DIR="$(dirname "$SCRIPT_DIR")/assets"
fi

if [ "$IS_NOTIFY_EVENT" = true ] && [ "$ZJBAR_EVENT" = "Stop" ]; then
  # Extract summary from AfterAgent's prompt_response
  SUMMARY=$(extract_summary "$PROMPT_RESPONSE")

  TITLE="✅ Gemini"
  if [ -n "$SUMMARY" ]; then
    MESSAGE="$SUMMARY"
  else
    MESSAGE="Task completed"
  fi

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
    ZELLIJ_BIN=$(command -v zellij)
    FOCUS_CMD="${ZELLIJ_BIN} -s '${ZELLIJ_SESSION_NAME}' pipe --name zjbar:focus -- ${ZELLIJ_PANE_ID}"

    case "$(uname)" in
    Darwin)
      [ -n "${TERM_PROGRAM:-}" ] && FOCUS_CMD="open -a '${TERM_PROGRAM}' && ${FOCUS_CMD}"
      ICON_PATH="${ICON_DIR}/gemini-logo.png"
      ICON_FLAG=()
      [ -f "$ICON_PATH" ] && ICON_FLAG=(-contentImage "$ICON_PATH")
      if command -v terminal-notifier >/dev/null 2>&1; then
        terminal-notifier \
          "${ICON_FLAG[@]}" \
          -group "zjbar-${ZELLIJ_PANE_ID}" \
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

# Send to zjbar plugin (fire-and-forget)
zellij -s "$ZELLIJ_SESSION_NAME" pipe --name "zjbar" -- "$PAYLOAD" &

# Gemini CLI requires valid JSON on stdout
echo '{}'
exit 0
