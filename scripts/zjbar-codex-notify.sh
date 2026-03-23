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

# Require jq for JSON parsing
command -v jq >/dev/null 2>&1 || { echo "zjbar: jq is required but not found" >&2; exit 1; }

# Source shared library
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=zjbar-lib.sh
source "${SCRIPT_DIR}/zjbar-lib.sh"

# Read JSON from first argument (Codex passes notification as $1)
INPUT="${1:-}"
[ -z "$INPUT" ] && exit 0

# Extract all fields from Codex notification JSON in a single jq call.
# Uses @sh format for shell-safe quoting — avoids the tab-join delimiter
# collision issue (bash `read` collapses consecutive tab delimiters).
_JQ_OUT=$(echo "$INPUT" | jq -r '
  "EVENT_TYPE=" + (.type // "" | @sh) + " " +
  "THREAD_ID=" + (."thread-id" // "" | @sh) + " " +
  "TURN_ID=" + (."turn-id" // "" | @sh) + " " +
  "CWD=" + (.cwd // "" | @sh) + " " +
  "LAST_MESSAGE=" + (."last-assistant-message" // "" | @sh)
') || {
  echo "zjbar-codex-notify.sh: failed to parse notification JSON — input may be malformed" >&2
  exit 1
}
eval "$_JQ_OUT"

# Validate required fields
if [ -z "$EVENT_TYPE" ]; then
  echo "zjbar-codex-notify.sh: missing required field: type" >&2
  exit 0
fi

# Only handle agent-turn-complete events
[ "$EVENT_TYPE" != "agent-turn-complete" ] && exit 0

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
  }') || {
  echo "zjbar-codex-notify.sh: failed to build payload JSON" >&2
  exit 1
}

# -- Desktop notification --
zjbar_load_notify_settings

SUMMARY=""
[ -n "$LAST_MESSAGE" ] && SUMMARY=$(zjbar_clean_and_truncate "$LAST_MESSAGE")

TITLE="✅ Codex"
MESSAGE="${SUMMARY:-Task completed}"

ICON_DIR=$(zjbar_resolve_icon_dir)

if zjbar_is_notify_event "Stop" && zjbar_check_should_notify; then
  zjbar_send_notification "$TITLE" "$MESSAGE" "$ICON_DIR" "codex-logo.png"
fi

# Send to plugin (fire-and-forget)
zellij -s "$ZELLIJ_SESSION_NAME" pipe --name "zjbar" -- "$PAYLOAD" &
