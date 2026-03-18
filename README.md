# zjbar

English | [简体中文](README.zh-CN.md)

A Zellij status bar plugin with a Tokyo Night powerline theme and optional AI coding agent activity awareness.

## Features

- **Powerline tab bar** — Tokyo Night themed tab bar with sharp powerline arrows between segments
- **Session & mode display** — shows session name and input mode (NORMAL, LOCKED, PANE, etc.) with color-coded pills
- **Clickable tabs** — click any tab to switch
- **Optional AI coding agent integration** — live activity indicators, permission flash, desktop notifications, and click-to-focus (supports Claude Code, CodeBuddy, Codex, OpenCode, Gemini CLI, and other compatible tools)
- **Multi-instance sync** — all Zellij tabs show a unified view of all AI sessions

## Install

### Prerequisites

- [Zellij](https://zellij.dev)

### Option 1: Use release binary

Add the zjbar plugin to your Zellij layout file (e.g. `~/.config/zellij/layouts/zjbar.kdl`):

```kdl
layout {
    default_tab_template {
        children
        pane size=1 borderless=true {
            plugin location="https://github.com/imroc/zjbar/releases/download/v1.1.36/zjbar.wasm"
        }
    }
}
```

Then start Zellij with this layout:

```bash
zellij --layout ~/.config/zellij/layouts/zjbar.kdl
```

> See [layout.kdl](layout.kdl) for a full example with all available color and style options.

### Option 2: Build from source

Prerequisites: [Rust](https://rustup.rs)

```bash
git clone https://github.com/imroc/zjbar.git
cd zjbar
make install
```

This builds the WASM binary, copies it to `~/.config/zellij/plugins/`, and installs the layout files. Then start Zellij:

```bash
zellij --layout zjbar
```

## AI Tool Integration (optional)

zjbar supports multiple AI coding agents. Each tool uses its own bridge to forward events to the zjbar plugin via `zellij pipe`.

### Claude Code / CodeBuddy

Install the zjbar plugin to automatically register hooks:

```
/plugin marketplace add imroc/zjbar
/plugin install zjbar@zjbar
```

Restart Claude Code / CodeBuddy for hooks to take effect.

To update to the latest version:

```
/plugin update
```

### OpenCode

Add the zjbar plugin to your `opencode.json`:

```json
{
  "plugin": ["zjbar-opencode@latest"]
}
```

Then start OpenCode inside a Zellij session with the zjbar layout — activity indicators will appear automatically.

To update, restart OpenCode — it will automatically fetch the latest version since `@latest` is specified.

### Codex CLI

Configure Codex to use the zjbar notify script:

```bash
make install-codex-hooks
```

This copies the notify script and icon to `~/.codex/zjbar/` and adds a `notify` entry to your Codex config (`~/.codex/config.toml`). When Codex finishes a turn, zjbar shows the Done indicator and sends a desktop notification with the task summary. The repo can be safely deleted after installation. Override the Codex config directory with `CODEX_HOME` env var.

To update, re-run `make install-codex-hooks` from the latest repo.

To uninstall:

```bash
make uninstall-codex-hooks
```

### Gemini CLI

First, clone the repo (or download it):

```bash
git clone https://github.com/imroc/zjbar.git
cd zjbar
```

Then install the hooks:

```bash
make install-gemini-hooks
```

This copies the hook script to `~/.gemini/zjbar/` and registers the hooks in `~/.gemini/settings.json`. The repo can be safely deleted after installation.

To uninstall:

```bash
make uninstall-gemini-hooks
```

zjbar tracks Gemini's full agent lifecycle: Thinking, Tool use, and Done states.

### Other tools

zjbar uses a unified JSON event protocol. Any AI coding tool can integrate by sending events via `zellij pipe --name zjbar`. See the [How It Works](#how-it-works) section for details.

### Desktop notifications (optional)

```bash
brew install terminal-notifier
```

When installed, desktop notifications are sent for `PermissionRequest`, `Notification`, and `Stop` events by default. Notifications include **context-aware message summaries** extracted from the AI tool's transcript:

- **Stop** — last assistant message + tool usage stats (e.g. `📝2 ✏️3 ▶5`)
- **PermissionRequest** — the specific command or file path being requested
- **Notification** — the notification message from the AI tool

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
- **`elapsed_time`** — `true` | `false` (default: `true`). Show elapsed time since last AI tool activity per tab.
- **`notifications`** — `always` | `unfocused` | `off` (default: `always`). When set to `unfocused`, notifications only fire when the terminal is not the frontmost app.
- **`notify_events`** — array of hook events to notify on (default: `["PermissionRequest", "Notification", "Stop"]`)

## Activity Symbols

When integrated with an AI coding agent (Claude Code, Codex, OpenCode, Gemini CLI, etc.), zjbar shows live activity indicators on each tab:

| Symbol | Meaning                   | Claude Code | CodeBuddy | Gemini CLI | OpenCode | Codex |
| ------ | ------------------------- | :---------: | :-------: | :--------: | :------: | :---: |
| ◆      | Session starting          |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ●      | Thinking                  |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ⚡     | Running Bash              |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ◉      | Reading / searching files |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ✎      | Editing / writing files   |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ⊜      | Spawning subagent         |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ◈      | Web search / fetch        |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ⚙      | Other tool                |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ▶      | Waiting for user prompt   |      ✔      |     ✔     |            |          |       |
| ⚠      | Waiting for permission    |      ✔      |     ✔     |            |    ✔     |       |
| ✓      | Done                      |      ✔      |     ✔     |     ✔      |    ✔     |   ✔   |

> **Note:** Codex CLI only provides the `agent-turn-complete` event (mapped to ✓ Done) and does not support other granular states. Gemini CLI lacks hook events for permission requests and user input prompts.

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
Codex notify     → zjbar-codex-notify.sh → zellij pipe → plugin → render
OpenCode plugin  → zjbar-opencode-plugin → zellij pipe → plugin → render
Gemini CLI hook  → zjbar-gemini-hook.sh  → zellij pipe → plugin → render
```

All integrations use a unified JSON payload with a `source` field to identify the AI tool.

## Uninstall

```bash
make uninstall
```

## License

MIT
