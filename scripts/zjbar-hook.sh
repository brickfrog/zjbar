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

# Resolve plugin root: CodeBuddy uses CODEBUDDY_PLUGIN_ROOT, Claude Code uses CLAUDE_PLUGIN_ROOT
PLUGIN_ROOT="${CODEBUDDY_PLUGIN_ROOT:-${CLAUDE_PLUGIN_ROOT:-}}"

# Extract all fields in a single jq call (required dependency)
eval "$(echo "$INPUT" | jq -r '
  @sh "HOOK_EVENT=\(.hook_event_name // "")",
  @sh "SESSION_ID=\(.session_id // "")",
  @sh "TOOL_NAME=\(.tool_name // "")",
  @sh "CWD=\(.cwd // "")",
  @sh "TRANSCRIPT_PATH=\(.transcript_path // "")",
  @sh "NOTIF_MESSAGE=\(.message // "")",
  @sh "NOTIF_TITLE=\(.title // "")",
  @sh "NOTIF_TYPE=\(.notification_type // "")",
  @sh "AGENT_ID=\(.agent_id // "")"
')"

[ -z "$HOOK_EVENT" ] && exit 0

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
[ -n "$AGENT_ID" ] && exit 0

# CodeBuddy compatibility:
# CodeBuddy doesn't fire Stop events. Instead it sends a Notification
# with notification_type="idle_prompt" when the session becomes idle.
# Map this to a Stop event so zjbar shows the correct ✅ Done state
# and can extract a transcript summary for the desktop notification.
if [ "$HOOK_EVENT" = "Notification" ] && [ "$NOTIF_TYPE" = "idle_prompt" ]; then
  HOOK_EVENT="Stop"
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
    # Strip markdown formatting
    text=$(echo "$text" | sed -E 's/\*\*//g; s/\*//g; s/`//g; s/^#+ //; s/\[([^]]*)\]\([^)]*\)/\1/g' | tr '\n' ' ' | sed 's/  */ /g')
    # Truncate to 120 chars at word boundary
    if [ ${#text} -gt 120 ]; then
      text="${text:0:117}"
      text="${text% *}..."
    fi
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
  [ "$HOOK_EVENT" = "$EVT" ] && {
    IS_NOTIFY_EVENT=true
    break
  }
done

# -- Stop debounce --
# Claude Code and CodeBuddy may fire multiple Stop-like events in quick
# succession (e.g. Stop + Notification/idle_prompt). To avoid duplicate
# desktop notifications:
#
# 1. On Stop: cancel any pending debounce, then schedule a new
#    notification to fire after STOP_DEBOUNCE_SECS seconds.
# 2. On activity events (UserPromptSubmit, PreToolUse): cancel the pending
#    debounce — the session is still active, not truly done.
#
# Implementation: each pane gets a debounce PID file at
# /tmp/zjbar-stop-debounce-<PANE_ID>.pid. The debounce is a background
# subshell that sleeps, then sends the notification.
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

# Activity events cancel any pending Stop notification
case "$HOOK_EVENT" in
UserPromptSubmit | PreToolUse | PostToolUse | PostToolUseFailure | PermissionRequest)
  cancel_stop_debounce
  ;;
esac

if [ "$IS_NOTIFY_EVENT" = true ]; then
  # Bell for PermissionRequest
  [ "$HOOK_EVENT" = "PermissionRequest" ] && printf '\a' >/dev/tty 2>/dev/null || true

  # Detect app variant: check CLAUDE_SETTINGS_DIR or CodeBuddy-specific env vars
  APP_NAME="Claude Code"
  ICON_FILE="claude-logo.png"
  if [ -n "${CODEBUDDY_PROJECT_DIR:-}" ]; then
    APP_NAME="CodeBuddy"
    ICON_FILE="codebuddy-logo.png"
  fi

  # For Stop events, use debounced notification
  if [ "$HOOK_EVENT" = "Stop" ]; then
    # Cancel any previous pending Stop notification
    cancel_stop_debounce

    # Schedule debounced notification in background
    (
      sleep "$STOP_DEBOUNCE_SECS"
      rm -f "$DEBOUNCE_PID_FILE"

      # Extract summary from transcript (if available)
      SUMMARY=$(extract_summary "$TRANSCRIPT_PATH" "$HOOK_EVENT")

      # Build notification title and message
      TITLE="✅ $APP_NAME"
      if [ -n "$SUMMARY" ]; then
        MESSAGE="$SUMMARY"
      else
        MESSAGE="Task completed"
      fi

      # Check notification mode
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
            ICON_PATH="${PLUGIN_ROOT}/assets/$ICON_FILE"
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
    ) &
    echo $! >"$DEBOUNCE_PID_FILE"

  else
    # Non-Stop events: send notification immediately (PermissionRequest, Notification, etc.)
    SUMMARY=$(extract_summary "$TRANSCRIPT_PATH" "$HOOK_EVENT")

    # Build notification title and message per event type
    case "$HOOK_EVENT" in
    PermissionRequest)
      TOOL_SUFFIX=""
      [ -n "$TOOL_NAME" ] && TOOL_SUFFIX=" — $TOOL_NAME"
      TITLE="⚠ $APP_NAME"
      if [ -n "$SUMMARY" ]; then
        MESSAGE="$SUMMARY"
      else
        MESSAGE="Permission requested${TOOL_SUFFIX}"
      fi
      ;;
    Notification)
      if [ -n "$NOTIF_TITLE" ]; then
        TITLE="$NOTIF_TITLE"
      else
        TITLE="$APP_NAME"
      fi
      if [ -n "$SUMMARY" ]; then
        MESSAGE="$SUMMARY"
      elif [ -n "$NOTIF_MESSAGE" ]; then
        MESSAGE="$NOTIF_MESSAGE"
      else
        MESSAGE="Notification received"
      fi
      ;;
    *)
      TITLE="$APP_NAME"
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
        iTerm.app) EXPECTED="iTerm2" ;;
        esac
        FRONT_APP=$(osascript -e 'tell application "System Events" to get name of first application process whose frontmost is true' 2>/dev/null)
        [ "$FRONT_APP" = "$EXPECTED" ] && TERM_FOCUSED=true
        ;;
      Linux)
        # X11: check if focused window belongs to our terminal
        if command -v xdotool >/dev/null 2>&1; then
          ACTIVE_PID=$(xdotool getactivewindow getwindowpid 2>/dev/null)
          if [ -n "$ACTIVE_PID" ]; then
            PID=$$
            while [ "$PID" -gt 1 ] 2>/dev/null; do
              [ "$PID" = "$ACTIVE_PID" ] && {
                TERM_FOCUSED=true
                break
              }
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
        echo "$NOW" >"$LOCK"

        # Click callback: activate terminal + focus the pane
        ZELLIJ_BIN=$(command -v zellij)
        FOCUS_CMD="${ZELLIJ_BIN} -s '${ZELLIJ_SESSION_NAME}' pipe --name zjbar:focus -- ${ZELLIJ_PANE_ID}"

        case "$(uname)" in
        Darwin)
          [ -n "${TERM_PROGRAM:-}" ] && FOCUS_CMD="open -a '${TERM_PROGRAM}' && ${FOCUS_CMD}"
          ICON_PATH="${PLUGIN_ROOT}/assets/$ICON_FILE"
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
  fi
fi

# Send to plugin (fire-and-forget: zellij pipe blocks indefinitely)
# Use -s flag to specify session explicitly because Claude Code sets ZELLIJ=0
# which breaks the default IPC path detection.
zellij -s "$ZELLIJ_SESSION_NAME" pipe --name "zjbar" -- "$PAYLOAD" &
