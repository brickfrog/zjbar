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

# Require jq for JSON parsing
if ! command -v jq >/dev/null 2>&1; then
  echo '{}' # Gemini requires JSON stdout
  echo "zjbar: jq is required but not found" >&2
  exit 1
fi

# Source shared library
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=zjbar-lib.sh
source "${SCRIPT_DIR}/zjbar-lib.sh"

# Read hook JSON from stdin
INPUT=$(cat)
[ -z "$INPUT" ] && { echo '{}'; exit 0; }

# Extract all fields from Gemini hook JSON in a single jq call.
# Uses @sh format for shell-safe quoting — avoids the tab-join delimiter
# collision issue (bash `read` collapses consecutive tab delimiters).
# First try hook_event_name, fall back to env var after extraction.
_JQ_OUT=$(echo "$INPUT" | jq -r '
  "HOOK_EVENT=" + (.hook_event_name // "" | @sh) + " " +
  "SESSION_ID=" + (.session_id // "" | @sh) + " " +
  "GEMINI_CWD_INPUT=" + (.cwd // "" | @sh) + " " +
  "TOOL_NAME=" + (.tool_name // "" | @sh) + " " +
  "PROMPT_RESPONSE=" + (.prompt_response // "" | @sh)
') || {
  echo "zjbar-gemini-hook.sh: failed to parse hook JSON — input may be malformed" >&2
  echo '{}'
  exit 1
}
eval "$_JQ_OUT"

# Fall back to env var if hook_event_name not in JSON
[ -z "$HOOK_EVENT" ] && HOOK_EVENT="${ZJBAR_GEMINI_EVENT:-}"
if [ -z "$HOOK_EVENT" ]; then
  echo '{}'
  exit 0
fi

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
  }') || {
  echo "zjbar-gemini-hook.sh: failed to build payload JSON" >&2
  echo '{}'
  exit 1
}

# -- Desktop notification --
zjbar_load_notify_settings

if zjbar_is_notify_event "$ZJBAR_EVENT" && [ "$ZJBAR_EVENT" = "Stop" ]; then
  SUMMARY=$(zjbar_clean_and_truncate "$PROMPT_RESPONSE")

  TITLE="✅ Gemini"
  MESSAGE="${SUMMARY:-Task completed}"

  ICON_DIR=$(zjbar_resolve_icon_dir)

  if zjbar_check_should_notify; then
    zjbar_send_notification "$TITLE" "$MESSAGE" "$ICON_DIR" "gemini-logo.png"
  fi
fi

# Send to zjbar plugin (fire-and-forget)
zellij -s "$ZELLIJ_SESSION_NAME" pipe --name "zjbar" -- "$PAYLOAD" &

# Gemini CLI requires valid JSON on stdout
echo '{}'
exit 0
