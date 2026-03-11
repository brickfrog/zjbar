# zjbar

English | [简体中文](README.zh-CN.md)

A Zellij status bar plugin with a Tokyo Night powerline theme and optional Claude Code activity awareness.

## Features

- **Powerline tab bar** — Tokyo Night themed tab bar with sharp powerline arrows between segments
- **Session & mode display** — shows session name and input mode (NORMAL, LOCKED, PANE, etc.) with color-coded pills
- **Clickable tabs** — click any tab to switch
- **Optional AI coding agent integration** — live activity indicators, permission flash, desktop notifications, and click-to-focus (supports Claude Code, OpenCode, and other compatible tools)
- **Multi-instance sync** — all Zellij tabs show a unified view of all Claude sessions

## Install

### Prerequisites

- [Zellij](https://zellij.dev)

### Option 1: Claude Code plugin (recommended)

Install as a Claude Code plugin to get automatic hook registration and a one-command setup:

```
/plugin marketplace add imroc/zjbar
/plugin install zjbar@zjbar
```

Then download the WASM plugin and layouts:

```
/zjbar:install
```

Restart Claude Code for hooks to take effect, then start Zellij:

```bash
zellij --layout zjbar
```

### Option 2: Zellij layout only

Add the plugin to your Zellij layout directly (no Claude Code integration):

```kdl
default_tab_template {
    children
    pane size=1 borderless=true {
        plugin location="https://github.com/imroc/zjbar/releases/download/v1.0.11/zjbar.wasm"
    }
}
```

### Option 3: Build from source

Prerequisites: [Rust](https://rustup.rs), [jq](https://jqlang.github.io/jq/) (for hooks)

```bash
git clone https://github.com/imroc/zjbar.git
cd zjbar
./install.sh
```

Or use make targets directly:

```bash
make               # build wasm + update plugin
make install       # build + install layouts
make install-hooks # register Claude Code hooks
make uninstall     # remove plugin and layouts
make release       # create GitHub release (requires tag on HEAD)
```

The hook installer auto-detects the settings path (`~/.claude/settings.json`). To specify a custom path:

```bash
CLAUDE_SETTINGS=~/.codebuddy/settings.json make install-hooks
```

### Optional: click-to-focus notifications

```bash
brew install terminal-notifier
```

When installed, desktop notifications are sent for `PermissionRequest`, `Notification`, and `Stop` events by default. Notifications include **context-aware message summaries** extracted from Claude Code's transcript:

- **Stop** — last assistant message + tool usage stats (e.g. `📝2 ✏️3 ▶5`)
- **PermissionRequest** — the specific command or file path being requested
- **Notification** — the notification message from Claude Code

You can customize which events trigger notifications and the notification mode via `~/.config/zellij/plugins/zjbar.json` or the **settings menu** (click the session name in the status bar):

```json
{
  "flash": "brief",
  "elapsed_time": true,
  "notifications": "always",
  "notify_events": ["PermissionRequest", "Notification", "Stop"]
}
```

- **`flash`** — `brief` | `persist` | `off` (default: `brief`). Tab background flash on permission request.
- **`elapsed_time`** — `true` | `false` (default: `true`). Show elapsed time since last Claude Code activity per tab.
- **`notifications`** — `always` | `unfocused` | `off` (default: `always`). When set to `unfocused`, notifications only fire when the terminal is not the frontmost app.
- **`notify_events`** — array of Claude Code hook events to notify on (default: `["PermissionRequest", "Notification", "Stop"]`)

## Activity Symbols

When integrated with an AI coding agent (Claude Code, OpenCode, etc.), zjbar shows live activity indicators on each tab:

| Symbol | Meaning                   |
| ------ | ------------------------- |
| ◆      | Session starting          |
| ●      | Thinking                  |
| ⚡     | Running Bash              |
| ◉      | Reading / searching files |
| ✎      | Editing / writing files   |
| ⊜      | Spawning subagent         |
| ◈      | Web search / fetch        |
| ⚙      | Other tool                |
| ▶      | Waiting for user prompt   |
| ⚠      | Waiting for permission    |
| ✓      | Done                      |

## Configuration

All visual settings are configured via the KDL layout file. Every option is optional — defaults use the Tokyo Night theme.

Behavioral settings (`flash`, `elapsed_time`, `notifications`) are stored in `~/.config/zellij/plugins/zjbar.json` and can be changed at runtime via the settings menu (click the session name).

```kdl
plugin location="zjbar.wasm" {
    // Colors: any "#rrggbb" hex value
    bar_bg          "#1a1b26"
    session_bg      "#7aa2f7"
    session_fg      "#16161e"
    tab_active_bg   "#292e42"
    tab_active_fg   "#c0caf5"
    tab_inactive_bg "#16161e"
    tab_inactive_fg "#a9b1d6"

    // Mode colors: mode_<name>_bg / mode_<name>_fg
    // Modes: normal, locked, pane, tab, resize, move, scroll,
    //        search, entersearch, session, prompt, renametab,
    //        renamepane, tmux

    // Activity icon colors
    activity_thinking_color "#bb9af7"
    activity_tool_color     "#ff9e64"

    // Separators (powerline characters)
    separator_left ""     // \ue0b0
    separator_tab  ""     // \ue0b1

    // Tab indicators (shown when pane is fullscreen / floating)
    tab_fullscreen_indicator " 󰊓"
    tab_floating_indicator " 󰹙"
}
```

See [layout.kdl](layout.kdl) for the full list of available options with defaults.

## How It Works

1. **WASM plugin** — runs inside Zellij, renders the status bar, manages state
2. **Hook / plugin bridge** (optional) — forwards AI coding agent events via `zellij pipe`

```
Claude Code hook → zjbar-hook.sh        → zellij pipe → plugin → render
OpenCode plugin  → zjbar-opencode-plugin → zellij pipe → plugin → render
```

All integrations use a unified JSON payload with a `source` field to identify the AI tool.

## Uninstall

```bash
make uninstall
```

Or if installed as a Claude Code plugin:

```
/plugin uninstall zjbar@zjbar
```

## License

MIT
