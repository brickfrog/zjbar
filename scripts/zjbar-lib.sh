#!/usr/bin/env bash
# zjbar-lib.sh — Shared library for zjbar hook scripts.
# Source this file from hook scripts to avoid duplicating common logic.
#
# Provides:
#   zjbar_clean_and_truncate TEXT [MAXLEN]
#   zjbar_load_notify_settings
#   zjbar_is_notify_event EVENT
#   zjbar_check_should_notify
#   zjbar_send_notification TITLE MESSAGE ICON_DIR ICON_FILE

# -- Text utilities --

# Strip markdown formatting and truncate at word boundary.
# Usage: result=$(zjbar_clean_and_truncate "$text" [maxlen])
zjbar_clean_and_truncate() {
  local text="$1" maxlen="${2:-120}"
  [ -z "$text" ] && return

  # Strip markdown formatting
  text=$(echo "$text" | sed -E 's/\*\*//g; s/\*//g; s/`//g; s/^#+ //; s/\[([^]]*)\]\([^)]*\)/\1/g' | tr '\n' ' ' | sed 's/  */ /g')
  # Truncate at word boundary
  if [ ${#text} -gt "$maxlen" ]; then
    text="${text:0:$((maxlen - 3))}"
    text="${text% *}..."
  fi
  echo "$text"
}

# -- Notification settings --

# Load notification settings from zjbar.json.
# Sets: ZJBAR_NOTIFY_EVENTS, ZJBAR_NOTIFY_MODE
zjbar_load_notify_settings() {
  local settings_file="$HOME/.config/zellij/plugins/zjbar.json"
  ZJBAR_NOTIFY_EVENTS="PermissionRequest Notification Stop"
  ZJBAR_NOTIFY_MODE="always"

  if [ -f "$settings_file" ]; then
    local custom_events custom_mode
    custom_events=$(jq -r '.notify_events // empty | join(" ")' "$settings_file" 2>/dev/null)
    [ -n "$custom_events" ] && ZJBAR_NOTIFY_EVENTS="$custom_events"
    custom_mode=$(jq -r '.notifications // empty' "$settings_file" 2>/dev/null)
    [ -n "$custom_mode" ] && ZJBAR_NOTIFY_MODE=$(echo "$custom_mode" | tr '[:upper:]' '[:lower:]')
  fi
}

# Check if an event is in the notify list.
# Usage: zjbar_is_notify_event "Stop" && echo "yes"
zjbar_is_notify_event() {
  local event="$1" evt
  for evt in $ZJBAR_NOTIFY_EVENTS; do
    [ "$event" = "$evt" ] && return 0
  done
  return 1
}

# -- Focus detection --

# Check if the terminal app is currently focused.
# Returns 0 (true) if focused, 1 (false) if not.
zjbar_is_terminal_focused() {
  case "$(uname)" in
  Darwin)
    local expected="${TERM_PROGRAM:-}"
    case "$expected" in
    Apple_Terminal) expected="Terminal" ;;
    iTerm.app) expected="iTerm2" ;;
    esac
    local front_app
    front_app=$(osascript -e 'tell application "System Events" to get name of first application process whose frontmost is true' 2>/dev/null)
    [ "$front_app" = "$expected" ] && return 0
    ;;
  Linux)
    if command -v xdotool >/dev/null 2>&1; then
      local active_pid
      active_pid=$(xdotool getactivewindow getwindowpid 2>/dev/null)
      if [ -n "$active_pid" ]; then
        local pid=$$
        while [ "$pid" -gt 1 ] 2>/dev/null; do
          [ "$pid" = "$active_pid" ] && return 0
          pid=$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')
        done
      fi
    fi
    ;;
  esac
  return 1
}

# Determine whether a desktop notification should be sent,
# based on ZJBAR_NOTIFY_MODE and terminal focus state.
# Returns 0 (true) if should notify, 1 (false) if not.
zjbar_check_should_notify() {
  case "$ZJBAR_NOTIFY_MODE" in
  always) return 0 ;;
  unfocused)
    zjbar_is_terminal_focused && return 1
    return 0
    ;;
  *) return 1 ;;
  esac
}

# -- Desktop notification --

# Send a desktop notification with click-to-focus support.
# Usage: zjbar_send_notification TITLE MESSAGE ICON_DIR ICON_FILE
#
# Requires: ZELLIJ_SESSION_NAME, ZELLIJ_PANE_ID (from environment)
zjbar_send_notification() {
  local title="$1" message="$2" icon_dir="$3" icon_file="$4"

  local zellij_bin
  zellij_bin=$(command -v zellij)
  local focus_cmd="${zellij_bin} -s '${ZELLIJ_SESSION_NAME}' pipe --name zjbar:focus -- ${ZELLIJ_PANE_ID}"

  case "$(uname)" in
  Darwin)
    [ -n "${TERM_PROGRAM:-}" ] && focus_cmd="open -a '${TERM_PROGRAM}' && ${focus_cmd}"
    local icon_path="${icon_dir}/${icon_file}"
    local icon_flag=()
    [ -f "$icon_path" ] && icon_flag=(-contentImage "$icon_path")
    if command -v terminal-notifier >/dev/null 2>&1; then
      terminal-notifier \
        "${icon_flag[@]}" \
        -group "zjbar-${ZELLIJ_PANE_ID}" \
        -title "$title" \
        -message "$message" \
        -execute "$focus_cmd" &
    else
      osascript -e "display notification \"$message\" with title \"$title\"" &
    fi
    ;;
  Linux)
    if command -v notify-send >/dev/null 2>&1; then
      notify-send "$title" "$message" &
    fi
    ;;
  esac
}

# -- Icon directory resolution --

# Resolve the assets directory relative to the calling script.
# Usage: ICON_DIR=$(zjbar_resolve_icon_dir)
zjbar_resolve_icon_dir() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[1]:-$0}")" && pwd)"
  if [ -d "${script_dir}/assets" ]; then
    echo "${script_dir}/assets"
  else
    echo "$(dirname "$script_dir")/assets"
  fi
}
