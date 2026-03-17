#!/usr/bin/env bash
# zjbar-hook.sh — Claude Code hook → zellij pipe bridge
# Forwards hook events to the zjbar Zellij plugin via pipe.
#
# Usage in ~/.claude/settings.json hooks:
#   "command": "/path/to/zjbar-hook.sh"

# Debug logging: set ZJBAR_DEBUG=1 to log all hook events with timestamps
# Logs to /tmp/zjbar-debug-<pane_id>.log
ZJBAR_DEBUG="${ZJBAR_DEBUG:-0}"

# Exit silently if not running inside Zellij
[ -z "$ZELLIJ_SESSION_NAME" ] && exit 0
[ -z "$ZELLIJ_PANE_ID" ] && exit 0

# Require jq for JSON parsing
command -v jq >/dev/null 2>&1 || { echo "zjbar: jq is required but not found" >&2; exit 1; }

# Source shared library
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=zjbar-lib.sh
source "${SCRIPT_DIR}/zjbar-lib.sh"

# Read hook JSON from stdin
INPUT=$(cat)

# Resolve plugin root: CodeBuddy uses CODEBUDDY_PLUGIN_ROOT, Claude Code uses CLAUDE_PLUGIN_ROOT
PLUGIN_ROOT="${CODEBUDDY_PLUGIN_ROOT:-${CLAUDE_PLUGIN_ROOT:-}}"

# Extract fields from JSON.
# Structured ID/path fields are safe to join with tab; free-text fields
# (message, title) are extracted separately to avoid delimiter issues.
_FIELDS=$(echo "$INPUT" | jq -r '[
  .hook_event_name // "",
  .session_id // "",
  .tool_name // "",
  .cwd // "",
  .transcript_path // "",
  .notification_type // "",
  .agent_id // ""
] | join("\t")') || exit 0

IFS=$'\t' read -r HOOK_EVENT SESSION_ID TOOL_NAME CWD TRANSCRIPT_PATH \
  NOTIF_TYPE AGENT_ID <<< "$_FIELDS"
NOTIF_MESSAGE=$(echo "$INPUT" | jq -r '.message // ""')
NOTIF_TITLE=$(echo "$INPUT" | jq -r '.title // ""')

[ -z "$HOOK_EVENT" ] && exit 0

# Debug: log raw event with millisecond timestamp
if [ "$ZJBAR_DEBUG" = "1" ]; then
  _TS=$(python3 -c 'import time; print(f"{time.time():.3f}")' 2>/dev/null || date +%s)
  _EXTRA=""
  [ -n "$TOOL_NAME" ] && _EXTRA=" tool=$TOOL_NAME"
  [ -n "$NOTIF_TYPE" ] && _EXTRA="$_EXTRA notif_type=$NOTIF_TYPE"
  [ -n "$AGENT_ID" ] && _EXTRA="$_EXTRA agent_id=$AGENT_ID"
  echo "${_TS} pane=${ZELLIJ_PANE_ID} event=${HOOK_EVENT}${_EXTRA}" \
    >> "/tmp/zjbar-debug-${ZELLIJ_PANE_ID}.log"
  # Log complete raw JSON payload to separate file
  echo "${_TS} pane=${ZELLIJ_PANE_ID} RAW_PAYLOAD: ${INPUT}" \
    >> "/tmp/zjbar-debug-raw-${ZELLIJ_PANE_ID}.log"
fi

# Skip noise notification types that shouldn't affect the status bar.
# auth_success fires on every startup (login/auth) and would trigger
# flash animation on the tab. permission_prompt duplicates the
# PermissionRequest event.
if [ "$HOOK_EVENT" = "Notification" ]; then
  case "$NOTIF_TYPE" in
  auth_success | permission_prompt) exit 0 ;;
  esac
fi

# Ignore subagent events — only track the main agent.
# When a hook fires inside a subagent, Claude Code includes an `agent_id`
# field in the JSON payload.  We don't want subagent tool-use, thinking,
# or completion events to update the status bar or trigger notifications.
# Exception: SubagentStop is forwarded so Rust can transition Suspending → Done.
if [ -n "$AGENT_ID" ] && [ "$HOOK_EVENT" != "SubagentStop" ]; then
  exit 0
fi

# SubagentStop: forward to Rust as-is (it checks Suspending state),
# but treat it as "Stop" for desktop notification purposes.
NOTIFY_AS_EVENT="$HOOK_EVENT"
if [ "$HOOK_EVENT" = "SubagentStop" ]; then
  NOTIFY_AS_EVENT="Stop"
  if [ "$ZJBAR_DEBUG" = "1" ]; then
    _TS=$(python3 -c 'import time; print(f"{time.time():.3f}")' 2>/dev/null || date +%s)
    echo "${_TS} pane=${ZELLIJ_PANE_ID} SubagentStop received, notify as Stop" \
      >> "/tmp/zjbar-debug-${ZELLIJ_PANE_ID}.log"
  fi
fi

# CodeBuddy compatibility:
# CodeBuddy doesn't fire Stop events. Instead it sends a Notification
# with notification_type="idle_prompt" when the session becomes idle.
# Map to Suspending (not Stop) so we wait for SubagentStop before
# triggering the desktop notification.  If no SubagentStop arrives
# within SUSPENDING_TIMEOUT (15s), the Rust timer auto-promotes to Done.
if [ "$HOOK_EVENT" = "Notification" ] && [ "$NOTIF_TYPE" = "idle_prompt" ]; then
  HOOK_EVENT="Suspending"
  if [ "$ZJBAR_DEBUG" = "1" ]; then
    _TS=$(python3 -c 'import time; print(f"{time.time():.3f}")' 2>/dev/null || date +%s)
    echo "${_TS} pane=${ZELLIJ_PANE_ID} MAPPED idle_prompt → Suspending" \
      >> "/tmp/zjbar-debug-${ZELLIJ_PANE_ID}.log"
  fi
fi

# Build compact JSON payload
PAYLOAD=$(jq -nc \
  --arg source "claude" \
  --arg pane_id "$ZELLIJ_PANE_ID" \
  --arg session_id "$SESSION_ID" \
  --arg hook_event "$HOOK_EVENT" \
  --arg tool_name "$TOOL_NAME" \
  --arg cwd "$CWD" \
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

# -- Transcript path fallback --
# When transcript_path is not provided (e.g. CodeBuddy idle_prompt),
# attempt to locate it from session_id and cwd.
# Layout: ~/.codebuddy/projects/<slug>/<session_id>.jsonl  (CodeBuddy)
#         ~/.claude/projects/-<slug>/<session_id>.jsonl     (Claude Code)
resolve_transcript_path() {
  local session="$1" cwd="$2"
  [ -z "$session" ] || [ -z "$cwd" ] && return

  # Convert /Users/roc/dev/zjbar → Users-roc-dev-zjbar
  local slug
  slug=$(echo "$cwd" | sed 's|^/||; s|/|-|g')

  # Try CodeBuddy first, then Claude Code
  local candidate
  for candidate in \
    "$HOME/.codebuddy/projects/${slug}/${session}.jsonl" \
    "$HOME/.claude/projects/-${slug}/${session}.jsonl"; do
    if [ -f "$candidate" ]; then
      echo "$candidate"
      return
    fi
  done
}

if [ -z "$TRANSCRIPT_PATH" ] || [ ! -f "$TRANSCRIPT_PATH" ]; then
  TRANSCRIPT_PATH=$(resolve_transcript_path "$SESSION_ID" "$CWD")
fi

# -- Transcript summary extraction --
# Extract a concise summary from the JSONL transcript.
# Supports both Claude Code and CodeBuddy transcript formats:
#   Claude Code: {"type":"assistant","message":{"content":[{"type":"text","text":"..."}]}}
#   CodeBuddy:   {"type":"message","role":"assistant","content":[{"type":"output_text","text":"..."}]}
# Usage: summary=$(extract_summary "$TRANSCRIPT_PATH" "$HOOK_EVENT")
extract_summary() {
  local transcript="$1" event="$2"
  [ -z "$transcript" ] || [ ! -f "$transcript" ] && return

  case "$event" in
  Stop)
    # Get last assistant message text, clean markdown, truncate
    # Handles both Claude Code (.type=="assistant" → .message.content[].text)
    # and CodeBuddy (.type=="message" + .role=="assistant" → .content[].text)
    local text
    text=$(tail -100 "$transcript" |
      jq -r '
        if .type == "assistant" then
          .message.content[]? | select(.type == "text") | .text
        elif (.type == "message" and .role == "assistant") then
          .content[]? | select(.type == "text" or .type == "output_text") | .text
        else empty end
      ' 2>/dev/null |
      tail -1)
    [ -z "$text" ] && return
    text=$(zjbar_clean_and_truncate "$text")
    # Count tool usage in recent messages
    # Claude Code: tool_use embedded in assistant message content
    # CodeBuddy: function_call as separate top-level entries
    local tools
    tools=$(tail -200 "$transcript" |
      jq -r '
        if .type == "assistant" then
          .message.content[]? | select(.type == "tool_use") | .name
        elif .type == "function_call" then
          .name
        else empty end
      ' 2>/dev/null)
    local write_n edit_n bash_n
    write_n=$(echo "$tools" | grep -c '^Write$' 2>/dev/null || echo 0)
    edit_n=$(echo "$tools" | grep -c '^Edit$' 2>/dev/null || echo 0)
    bash_n=$(echo "$tools" | grep -c '^Bash$' 2>/dev/null || echo 0)
    local stats=""
    [ "$write_n" -gt 0 ] 2>/dev/null && stats="${stats}📝${write_n} "
    [ "$edit_n" -gt 0 ] 2>/dev/null && stats="${stats}✏️${edit_n} "
    [ "$bash_n" -gt 0 ] 2>/dev/null && stats="${stats}▶${bash_n} "
    stats=$(echo "$stats" | sed 's/ $//')
    if [ -n "$stats" ] && [ -n "$text" ]; then
      echo "${text} [${stats}]"
    else
      echo "$text"
    fi
    ;;
  PermissionRequest)
    # Extract the last tool_use that matches the tool_name
    # Claude Code: tool_use in assistant message content
    # CodeBuddy: function_call as separate top-level entry with .arguments
    if [ -n "$TOOL_NAME" ] && [ -n "$transcript" ] && [ -f "$transcript" ]; then
      local detail
      detail=$(tail -50 "$transcript" |
        jq -r --arg tn "$TOOL_NAME" '
          if .type == "assistant" then
            .message.content[]? | select(.type == "tool_use" and .name == $tn) | .input | tostring
          elif (.type == "function_call" and .name == $tn) then
            .arguments | tostring
          else empty end
        ' 2>/dev/null |
        tail -1)
      if [ -n "$detail" ]; then
        # Extract meaningful short description from tool input
        local short
        case "$TOOL_NAME" in
        Bash)
          short=$(echo "$detail" | jq -r '.command // empty' 2>/dev/null)
          ;;
        Write | Read | Edit)
          short=$(echo "$detail" | jq -r '.file_path // .path // empty' 2>/dev/null)
          ;;
        *)
          short=$(echo "$detail" | jq -r 'to_entries[0].value // empty' 2>/dev/null | head -c 80)
          ;;
        esac
        [ -n "$short" ] && echo "$short"
      fi
    fi
    ;;
  Notification)
    # Notification events carry their own message
    [ -n "$NOTIF_MESSAGE" ] && echo "$NOTIF_MESSAGE"
    ;;
  esac
}

# Desktop notification + bell
zjbar_load_notify_settings

if zjbar_is_notify_event "$NOTIFY_AS_EVENT"; then
  # Bell for PermissionRequest
  [ "$NOTIFY_AS_EVENT" = "PermissionRequest" ] && printf '\a' >/dev/tty 2>/dev/null || true

  # Detect app variant: check CLAUDE_SETTINGS_DIR or CodeBuddy-specific env vars
  APP_NAME="Claude Code"
  ICON_FILE="claude-logo.png"
  if [ -n "${CODEBUDDY_PROJECT_DIR:-}" ]; then
    APP_NAME="CodeBuddy"
    ICON_FILE="codebuddy-logo.png"
  fi

  SUMMARY=$(extract_summary "$TRANSCRIPT_PATH" "$NOTIFY_AS_EVENT")

  # Build notification title and message per event type
  case "$NOTIFY_AS_EVENT" in
  Stop)
    TITLE="✅ $APP_NAME"
    MESSAGE="${SUMMARY:-Task completed}"
    ;;
  PermissionRequest)
    TOOL_SUFFIX=""
    [ -n "$TOOL_NAME" ] && TOOL_SUFFIX=" — $TOOL_NAME"
    TITLE="⚠ $APP_NAME"
    MESSAGE="${SUMMARY:-Permission requested${TOOL_SUFFIX}}"
    ;;
  Notification)
    TITLE="${NOTIF_TITLE:-$APP_NAME}"
    MESSAGE="${SUMMARY:-${NOTIF_MESSAGE:-Notification received}}"
    ;;
  *)
    TITLE="$APP_NAME"
    MESSAGE="Event: $NOTIFY_AS_EVENT"
    ;;
  esac

  if zjbar_check_should_notify; then
    zjbar_send_notification "$TITLE" "$MESSAGE" "${PLUGIN_ROOT}/assets" "$ICON_FILE"
  fi
fi

# Send to plugin (fire-and-forget: zellij pipe blocks indefinitely)
# Use -s flag to specify session explicitly because Claude Code sets ZELLIJ=0
# which breaks the default IPC path detection.
zellij -s "$ZELLIJ_SESSION_NAME" pipe --name "zjbar" -- "$PAYLOAD" &
