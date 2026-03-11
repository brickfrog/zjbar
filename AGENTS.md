# zjbar Development Guide

## Overview

zjbar is a Zellij WASM plugin that replaces the default tab bar with a Tokyo Night powerline-themed status bar, with optional AI coding agent activity awareness (Claude Code, OpenCode, etc.).

## Architecture

```
src/
├── main.rs           # Plugin entry point (ZellijPlugin trait impl), event routing, state management
├── config.rs         # BarConfig struct, KDL config parser, color/mode/activity helpers
├── render.rs         # Status bar rendering with ANSI escape codes and powerline arrows
├── state.rs          # State types: Activity, SessionInfo, HookPayload, etc.
├── event_handler.rs  # Maps hook events to Activity states (tool-agnostic)
└── tab_pane_map.rs   # Maps pane IDs to (tab_index, tab_name) pairs
scripts/
├── zjbar-hook.sh             # Claude Code hook → zellij pipe bridge
├── install-opencode.sh       # OpenCode plugin installer/uninstaller (legacy)
└── install-hooks.sh          # Claude Code hook installer (used by `make install-hooks`)
opencode-plugin/              # npm package: zjbar-opencode
├── src/index.ts              # OpenCode plugin → zellij pipe bridge (TypeScript)
├── package.json              # npm package config
└── tsconfig.json             # TypeScript config
```

## Build & Test

```bash
# Build WASM plugin
cargo build --release --target wasm32-wasip1

# Install to zellij plugins directory
cp target/wasm32-wasip1/release/zjbar.wasm ~/.config/zellij/plugins/

# Test with a layout
zellij --layout layout.kdl
```

## Testing with tmux

Zellij is an interactive terminal app, so use tmux to test the plugin programmatically.

### Rules

- **Fixed session name**: Always use `zjbar_test` for both tmux session and Zellij session (`zellij -s zjbar_test --layout layout.kdl`).
- **Pre-cleanup**: Kill any existing `zjbar_test` tmux session and Zellij session before starting a new one (`zellij delete-session zjbar_test` — otherwise `zellij -s zjbar_test --layout ...` will attach to the old session and ignore the layout).
- **Post-cleanup**: Always `tmux kill-session -t zjbar_test` after testing is complete (this also terminates the Zellij process inside it).
- **Auto-test before delivery**: For any change that can be verified via tmux (rendering, colors, tab behavior, click regions, status bar content), you MUST run the tmux test automatically and confirm the result passes before delivering to the user. Do NOT ask the user to manually observe or confirm — verify it yourself.

### Basic workflow

```bash
# 1. Build and install
cargo build --release --target wasm32-wasip1
cp target/wasm32-wasip1/release/zjbar.wasm ~/.config/zellij/plugins/

# 2. Clean up any leftover session, then start Zellij
tmux kill-session -t zjbar_test 2>/dev/null
zellij delete-session zjbar_test 2>/dev/null
tmux new-session -d -s zjbar_test -x 120 -y 30 \
  'zellij -s zjbar_test --layout layout.kdl'
sleep 2  # wait for Zellij to initialize

# 3. Check the status bar (last line of the pane)
tmux capture-pane -t zjbar_test -p | tail -1

# 4. Always clean up when done
tmux kill-session -t zjbar_test
```

### Creating tabs via `zellij action`

tmux intercepts `Ctrl+T` etc., so use `zellij action` from outside to manipulate tabs:

```bash
# Create new tabs (use fixed session name)
ZELLIJ_SESSION_NAME=zjbar_test zellij action new-tab
sleep 1
tmux capture-pane -t zjbar_test -p | tail -1
```

### Verifying ANSI colors

`capture-pane -p` strips colors. Use `-e` flag to preserve escape codes:

```bash
# Dump with ANSI codes in readable form
tmux capture-pane -t zjbar_test -p -e | tail -1 | sed 's/\x1b/ESC/g'

# Verify specific RGB values (e.g. #7aa2f7 = 122,162,247)
# Look for patterns like: ESC[48;2;122;162;247m (background)
#                          ESC[38;2;122;162;247m (foreground)
```

### Testing custom KDL config

Write a temp layout with overridden colors, then launch:

```bash
cat > /tmp/zjbar-test.kdl <<'EOF'
layout {
    default_tab_template {
        children
        pane size=1 borderless=true {
            plugin location="file:~/.config/zellij/plugins/zjbar.wasm" {
                session_bg "#ff0000"
                tab_active_index_bg "#00ff00"
            }
        }
    }
}
EOF

tmux kill-session -t zjbar_test 2>/dev/null
zellij delete-session zjbar_test 2>/dev/null
tmux new-session -d -s zjbar_test -x 120 -y 30 \
  'zellij -s zjbar_test --layout /tmp/zjbar-test.kdl'
sleep 2
tmux capture-pane -t zjbar_test -p -e | tail -1 | sed 's/\x1b/ESC/g'
# Confirm: 48;2;255;0;0 (session bg red), 48;2;0;255;0 (index bg green)

# Clean up
tmux kill-session -t zjbar_test
```

### Testing AI integration events

Send a mock hook event and verify the status bar updates:

```bash
# Start Zellij
tmux kill-session -t zjbar_test 2>/dev/null
zellij delete-session zjbar_test 2>/dev/null
tmux new-session -d -s zjbar_test -x 120 -y 30 \
  'zellij -s zjbar_test --layout layout.kdl'
sleep 2

# Get the pane ID of the first terminal pane
PANE_ID=$(zellij -s zjbar_test action list-clients 2>/dev/null | head -1 | awk '{print $1}')

# Send a mock event (e.g. PreToolUse with Bash)
zellij -s zjbar_test pipe --name zjbar -- \
  "{\"source\":\"claude\",\"pane_id\":${PANE_ID:-1},\"session_id\":\"test-session\",\"hook_event\":\"PreToolUse\",\"tool_name\":\"Bash\"}"

sleep 1
tmux capture-pane -t zjbar_test -p | tail -1
# Should show ⚡ icon on the tab

# Clean up
tmux kill-session -t zjbar_test
```

## Debugging AI Integration

### WASM plugin (Rust side)

- Use `eprintln!()` for debug output — it goes to Zellij's log file at `/tmp/zellij-<UID>/zellij-log/zellij.log`.
- To watch logs in real time: `tail -f /tmp/zellij-$(id -u)/zellij-log/zellij.log | grep -i zjbar`
- The `pipe()` method in `main.rs` receives all IPC messages. Add `eprintln!` there to inspect incoming payloads.
- Remember to remove all debug logging before committing.

### Claude Code integration

- **Hook registration**: Hooks are defined in `~/.claude/settings.json` (or `~/.codebuddy/settings.json` for CodeBuddy). The zjbar Claude Code plugin registers hooks automatically via `.claude-plugin/hooks.json`.
- **Hook script**: `scripts/zjbar-hook.sh` — receives hook event name and context JSON from stdin, formats and sends to `zellij pipe --name zjbar`.
- **Manual test**: Send a mock event directly:
  ```bash
  zellij -s <session> pipe --name zjbar -- \
    '{"source":"claude","pane_id":1,"session_id":"test","hook_event":"PreToolUse","tool_name":"Bash"}'
  ```
- **Hook events flow**: Claude Code → hook script (runs in separate process) → `zellij pipe` → WASM `pipe()` → `event_handler::handle_hook_event()` → Activity state update → re-render.

### OpenCode integration

- **Plugin source**: `opencode-plugin/src/index.ts`
- **Build**: `cd opencode-plugin && bun run build` → outputs to `opencode-plugin/dist/index.js`
- **Cache locations** (all three must be updated when debugging locally):
  1. `.opencode/plugins/zjbar.js` (project-local, copied from dist)
  2. `~/.config/opencode/node_modules/zjbar-opencode/dist/index.js` (global config cache)
  3. `~/.cache/opencode/node_modules/zjbar-opencode/dist/index.js` (global cache)
- **Quick update all caches after rebuild**:
  ```bash
  cd opencode-plugin && bun run build
  cp dist/index.js ../.opencode/plugins/zjbar.js
  cp dist/index.js ~/.config/opencode/node_modules/zjbar-opencode/dist/index.js 2>/dev/null
  cp dist/index.js ~/.cache/opencode/node_modules/zjbar-opencode/dist/index.js 2>/dev/null
  ```
- **Environment quirks**: OpenCode sets `ZELLIJ=0` inside Zellij, so `zellij pipe` without `-s <session>` fails silently. The plugin resolves the session name from `ZELLIJ_SESSION_NAME` env var and always passes `-s`.
- **Execution model difference**: OpenCode runs tool hooks in-process (not as separate shell commands like Claude Code). This means `tool.execute.before` and `tool.execute.after` fire within milliseconds of each other. Sending both PreToolUse and PostToolUse would cause the tool icon to be immediately overwritten by Thinking (●). The fix: only send PreToolUse, let the state naturally transition on `session.idle` (Stop).
- **Debug logging**: Temporarily add `appendFileSync("/tmp/zjbar-opencode.log", ...)` in `sendToZjbar()` to trace events. Remove before committing.

## Key Concepts

- **Rendering**: `render.rs` outputs raw ANSI escape codes via `print!()` in the `render()` method. Zellij captures stdout as pane content.
- **IPC**: AI tool hooks/plugins → `zellij pipe --name zjbar` → plugin's `pipe()` method. All integrations use a unified JSON payload with a `source` field. Claude Code uses `zjbar-hook.sh` (registered via `make install-hooks`), OpenCode uses `zjbar-opencode-plugin.js` (installed via `scripts/install-opencode.sh`).
- **Multi-instance sync**: Each tab has its own plugin instance. They sync state via `pipe_message_to_plugin()` with names like `zjbar:sync`, `zjbar:request`.
- **Configuration**: All visual and behavioral settings are parsed from the KDL layout plugin block via `BarConfig::from_kdl()` in `config.rs`. No runtime settings file.

## Conventions

- This file (`AGENTS.md`) is the single source of project instructions. `CLAUDE.md`, `CODEBUDDY.md`, and `GEMINI.md` are all **symlinks** to `AGENTS.md`. When committing, always `git add AGENTS.md` — never add the symlink names directly.
- All commit messages and code comments must be in **English**.
- The WASM target is `wasm32-wasip1` (configured in `.cargo/config.toml`).
- Release profile uses `opt-level = "s"` and LTO for minimal binary size.
- Color palette follows Tokyo Night. All color defaults are defined in `config.rs`.
- After any feature change, check if `README.md` needs updating (e.g. new config options, changed behavior, new install steps). If so, update both `README.md` (English) and `README.zh-CN.md` (Chinese) directly without asking for confirmation.

## Releasing a New Version

When creating a new release (e.g. bumping from `v1.0.4` to `v1.0.5`), update the version in **all** of these places:

1. **`Cargo.toml`** — `version = "x.y.z"`
2. **`README.md`** — WASM download URL in the layout example (`releases/download/vX.Y.Z/zjbar.wasm`)
3. **`README.zh-CN.md`** — same WASM download URL
4. **`commands/install.md`** — WASM download URL in the curl command
5. **`.claude-plugin/marketplace.json`** — both `version` fields
6. **`.claude-plugin/plugin.json`** — `version` field
7. **`opencode-plugin/package.json`** — `version` field (must match the release version)

Use `grep -r 'releases/download/v' .` to verify all WASM URLs are updated.

**Note:** `Cargo.lock` is auto-updated by `cargo build` when `Cargo.toml` version changes. Remember to `git add Cargo.lock` when committing the version bump.

After updating all versions, commit, push, and complete the release:

1. **Commit & push** the version bump.
2. **Tag & release** (builds WASM, creates GitHub Release with changelog, publishes npm package):
   ```bash
   git tag vX.Y.Z
   make release
   ```
   npm authentication is configured via `~/.npmrc` (`_authToken`).
