# zjbar

English | [简体中文](README.zh-CN.md)

A Zellij status bar plugin with a Tokyo Night powerline theme and authoritative [choir](https://github.com/brickfrog/choir) agent status awareness.

This is an owned fork of [imroc/zjbar](https://github.com/imroc/zjbar) for choir workflows. The fork name remains `zjbar`; upstream MIT attribution is preserved in [LICENSE](LICENSE).

## Features

- **Unified powerline navigation** — Tokyo Night themed, clickable row with native Zellij tabs first and choir leaf entries after them
- **Session & mode display** — shows session name and input mode (NORMAL, LOCKED, PANE, etc.) with color-coded pills
- **Choir status source** — polls `.choir/server.sock` for the typed `status_bar_state` snapshot; no process-name inference is used
- **Choir status folding** — renders lifecycle symbols beside the matching tab or leaf pane and collapses duplicate agents that resolve to the same Zellij pane
- **Native shortcut strip** — keeps Zellij's original bottom status-bar plugin for keybinding hints
- **Click-to-focus panes** — click any synthetic choir leaf entry to focus its Zellij pane
- **Graceful degradation** — keeps native tab navigation usable, shows compact choir source errors, and refuses unsupported schema versions fail-closed

## Install

### Prerequisites

- [Zellij](https://zellij.dev)
- Python 3 on `PATH` (used by the WASM plugin's host-side UDS bridge)
- choir with the `status_bar_state` MCP command exposed on `.choir/server.sock`

### Option 1: Use release binary

Add the zjbar plugin to your Zellij layout file (e.g. `~/.config/zellij/layouts/zjbar.kdl`):

```kdl
layout {
    default_tab_template {
        pane size=1 borderless=true {
            plugin location="https://github.com/brickfrog/zjbar/releases/download/v1.3.0/zjbar.wasm" {
                choir_socket ".choir/server.sock"
            }
        }
        children
        pane size=1 borderless=true {
            plugin location="zellij:status-bar"
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
git clone https://github.com/brickfrog/zjbar.git
cd zjbar
make install
```

This builds the WASM binary, copies it to `~/.config/zellij/plugins/`, and installs the layout files. Then start Zellij:

```bash
zellij --layout zjbar
```

## Choir Integration

zjbar polls the choir Unix domain socket once per second and sends a newline-framed JSON-RPC MCP request:

```json
{"jsonrpc":"2.0","id":"zjbar-status-bar-state","method":"tools/call","params":{"name":"status_bar_state","arguments":{}}}
```

The response payload must match [protocol/status_bar_state.json](protocol/status_bar_state.json). The renderer consumes only this typed snapshot plus Zellij's pane manifest for tab/pane titles; it does not inspect process names and does not infer state from Claude/Codex/OpenCode/Gemini hook events.

Choir lifecycle state is folded into the main navigation row. Real Zellij tabs keep their native tab numbers and click behavior, while non-top-level choir panes are appended as focusable entries using the pane title from Zellij when available. If multiple choir agents resolve to the same Zellij pane, zjbar chooses the highest-priority/current status for that pane instead of rendering duplicate `agent-*` entries.

Until choir exposes `status_bar_state`, or whenever `.choir/server.sock` is unreachable, zjbar keeps the native tab row usable and renders a compact `no choir` indicator without rendering inferred per-pane fields. If the server returns a snapshot with `schema_version` greater than the client-supported version, zjbar renders `schema ahead vN` and refuses to render per-pane fields.

The old hook scripts are still present in the repository for upstream compatibility work, but this fork's status rendering is driven by choir state.

Runtime settings (`flash`, `elapsed_time`, `notifications`) are still stored in `~/.config/zellij/plugins/zjbar.json` and can be changed via the settings menu (click the session name):

```json
{
  "flash": "brief",
  "elapsed_time": true,
  "notifications": "always",
  "notify_events": ["PermissionRequest", "Notification", "Stop"]
}
```

- **`flash`** — `brief` | `persist` | `off` (default: `brief`). Choir attention pulses use the flash colors.
- **`elapsed_time`** — `true` | `false` (default: `true`). Retained for legacy tab rendering helpers.
- **`notifications`** — `always` | `unfocused` | `off` (default: `always`). Retained for legacy hook scripts; notify-send from choir attention is out of scope for this fork.
- **`notify_events`** — array of legacy hook events to notify on (default: `["PermissionRequest", "Notification", "Stop"]`)

## Lifecycle Symbols

Choir lifecycle values render as:

| Symbol | Lifecycle              |
| ------ | ---------------------- |
| ⏳     | `working`              |
| 👁     | `review_owned`         |
| ✎      | `changes_requested`    |
| ✓      | `done`                 |
| ✗      | `failed`               |
| ⊖      | `exitable`             |
| 🔴     | `waiting_for_red_gate` |

Synthetic choir leaf entries also show PR number, unresolved thread count, and CI rollup when those fields are present. When `attention_needed` is true, the matching navigation entry pulses with the configured flash colors for about two seconds.

## Configuration

All visual settings are configured via the KDL layout file. Every option is optional — defaults use the Tokyo Night theme.

Behavioral settings (`flash`, `elapsed_time`, `notifications`) are stored in `~/.config/zellij/plugins/zjbar.json` and can be changed at runtime via the settings menu (click the session name).

```kdl
plugin location="zjbar.wasm" {
    // Choir status source
    choir_socket ".choir/server.sock"

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

    // Lifecycle icon colors
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

1. **WASM plugin** — runs inside Zellij as the top chrome, renders the unified tab/choir navigation row, manages click regions and flash timing.
2. **Host-side UDS bridge** — Zellij `run_command` invokes Python 3 to connect to `.choir/server.sock`, send the `status_bar_state` JSON-RPC request, and return one newline-framed response.
3. **Typed snapshot renderer** — Rust parses the response against the v1 schema and renders only recognized fields. The default layout keeps zjbar at the top, restores Zellij's native shortcut/status strip at the bottom, and folds choir state into the same selectable row as the Zellij tabs. All failures are fail-closed.

## Uninstall

```bash
make uninstall
```

## License

MIT
