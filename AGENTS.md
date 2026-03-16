# zjbar Development Guide

## Overview

zjbar is a Zellij WASM plugin that replaces the default tab bar with a Tokyo Night powerline-themed status bar, with optional AI coding agent activity awareness (Claude Code, Codex, OpenCode, Gemini CLI, etc.).

## Architecture

```
src/
├── main.rs           # Plugin entry point (ZellijPlugin trait impl), event routing, state management
├── config.rs         # BarConfig struct, KDL config parser, color/mode/activity helpers
├── render.rs         # Status bar rendering with ANSI escape codes and powerline arrows
├── state.rs          # State types: Activity, SessionInfo, HookPayload, etc.
├── event_handler.rs  # Maps hook events to Activity states (tool-agnostic)
└── tab_pane_map.rs   # Maps pane IDs to (tab_index, tab_name) pairs
hooks/
└── hooks.json                # Claude Code hook event definitions (default path for Claude Code plugin)
claude-hooks/
└── hooks.json                # Claude Code hook event definitions (backup copy, same as hooks/hooks.json)
gemini-hooks/
└── hooks.json                # Gemini CLI hook event definitions (reference/backup, not used at runtime)
scripts/
├── zjbar-hook.sh             # Claude Code hook → zellij pipe bridge
├── install-hooks.sh          # Claude Code hook installer (legacy, use plugin instead)
├── zjbar-codex-notify.sh     # Codex CLI notify → zellij pipe bridge
├── install-codex-hooks.sh    # Codex notify installer (used by `make install-codex-hooks`)
├── zjbar-gemini-hook.sh      # Gemini CLI hook → zellij pipe bridge
├── install-gemini-hooks.sh   # Gemini hook installer (used by `make install-gemini-hooks`)
└── install-codex-hooks.sh    # Codex notify installer (used by `make install-codex-hooks`)
opencode-plugin/              # npm package: zjbar-opencode
├── src/index.ts              # OpenCode plugin → zellij pipe bridge (TypeScript)
├── package.json              # npm package config
└── tsconfig.json             # TypeScript config
assets/
├── claude-logo.png           # Claude Code logo (used by hook for desktop notifications)
├── codebuddy-logo.png        # CodeBuddy logo
├── codex-logo.png            # Codex CLI logo
├── gemini-logo.png           # Gemini CLI logo
└── opencode-logo.png         # OpenCode logo
.claude-plugin/
├── marketplace.json          # Claude Code plugin marketplace listing
└── plugin.json               # Claude Code plugin metadata
gemini-extension.json         # Gemini CLI extension metadata (for `gemini extensions install`)
install.sh                    # Bootstrap installer (checks prerequisites, delegates to make)
layout.kdl                    # Default Zellij layout with zjbar
layout.swap.kdl               # Swap layout for stacked/alternate pane arrangements
rust-toolchain.toml           # Pins Rust toolchain version
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

- **Fixed session name**: Always use `zjbar_test` for both tmux session and Zellij session.
- **Dedicated tmux socket**: Always use `tmux -L zjbar_test` (dedicated socket) for ALL tmux commands. This isolates the test tmux server from any existing tmux sessions and ensures a clean environment free of inherited Zellij variables.
- **Clear Zellij env vars**: When AI agents run inside Zellij, environment variables (`ZELLIJ`, `ZELLIJ_SESSION_NAME`, `ZELLIJ_PANE_ID`) are inherited by child processes. Zellij refuses to start when these are set (it detects nesting). Always launch the tmux session with `env -u ZELLIJ -u ZELLIJ_SESSION_NAME -u ZELLIJ_PANE_ID` to strip them, and use `/bin/bash -c '...'` as the shell command (the user's default shell may be fish, which has different syntax).
- **Use `-n` not `--layout`**: Always use `zellij -s zjbar_test -n <layout>` (`--new-session-with-layout`) instead of `--layout`. When other Zellij sessions exist on the same machine (e.g. a `work` session), `--layout` tries to attach to an existing session instead of creating a new one and fails with "Session not found". `-n` always creates a new session.
- **Pre-cleanup**: Kill any existing `zjbar_test` tmux server and Zellij session before starting a new one (`zellij delete-session zjbar_test --force`).
- **Post-cleanup**: Always `tmux -L zjbar_test kill-server` after testing is complete (this kills the dedicated tmux server and all sessions/Zellij processes inside it).
- **Auto-test before delivery**: For any change that can be verified via tmux (rendering, colors, tab behavior, click regions, status bar content), you MUST build, deploy, and run the tmux test automatically, then verify the output matches expectations before delivering to the user. Never ask the user to manually observe or confirm test results unless tmux verification is truly impossible (e.g. interactive mouse clicks, visual aesthetics). Default assumption: if it renders in the status bar, you can capture and verify it yourself.
- **No confirmation needed**: Never ask the user for permission to run tmux tests, build commands, or any standard development operations (build, test, install, git operations, file creation/modification). Just do it. Be aggressive and autonomous — only ask the user when there is a genuine ambiguity in requirements or design decisions, never for routine execution.

### Basic workflow

```bash
# 1. Build and install
cargo build --release --target wasm32-wasip1
cp target/wasm32-wasip1/release/zjbar.wasm ~/.config/zellij/plugins/

# 2. Clean up any leftover session, then start Zellij
tmux -L zjbar_test kill-server 2>/dev/null
zellij delete-session zjbar_test --force 2>/dev/null
env -u ZELLIJ -u ZELLIJ_SESSION_NAME -u ZELLIJ_PANE_ID \
  tmux -L zjbar_test new-session -d -s zjbar_test -x 120 -y 30 \
  '/bin/bash -c "zellij -s zjbar_test -n layout.kdl"'
sleep 3  # wait for Zellij to initialize

# 3. Check the status bar (last line of the pane)
tmux -L zjbar_test capture-pane -t zjbar_test -p | tail -1

# 4. Always clean up when done
tmux -L zjbar_test kill-server
```

### Creating tabs via `zellij action`

tmux intercepts `Ctrl+T` etc., so use `zellij action` from outside to manipulate tabs:

```bash
# Create new tabs (use -s to target the test session)
zellij -s zjbar_test action new-tab
sleep 1
tmux -L zjbar_test capture-pane -t zjbar_test -p | tail -1
```

### Verifying ANSI colors

`capture-pane -p` strips colors. Use `-e` flag to preserve escape codes:

```bash
# Dump with ANSI codes in readable form
tmux -L zjbar_test capture-pane -t zjbar_test -p -e | tail -1 | sed 's/\x1b/ESC/g'

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

tmux -L zjbar_test kill-server 2>/dev/null
zellij delete-session zjbar_test --force 2>/dev/null
env -u ZELLIJ -u ZELLIJ_SESSION_NAME -u ZELLIJ_PANE_ID \
  tmux -L zjbar_test new-session -d -s zjbar_test -x 120 -y 30 \
  '/bin/bash -c "zellij -s zjbar_test -n /tmp/zjbar-test.kdl"'
sleep 3
tmux -L zjbar_test capture-pane -t zjbar_test -p -e | tail -1 | sed 's/\x1b/ESC/g'
# Confirm: 48;2;255;0;0 (session bg red), 48;2;0;255;0 (index bg green)

# Clean up
tmux -L zjbar_test kill-server
```

### Testing AI integration events

Send a mock hook event and verify the status bar updates:

```bash
# Start Zellij
tmux -L zjbar_test kill-server 2>/dev/null
zellij delete-session zjbar_test --force 2>/dev/null
env -u ZELLIJ -u ZELLIJ_SESSION_NAME -u ZELLIJ_PANE_ID \
  tmux -L zjbar_test new-session -d -s zjbar_test -x 120 -y 30 \
  '/bin/bash -c "zellij -s zjbar_test -n layout.kdl"'
sleep 3

# Send a mock event (e.g. PreToolUse with Bash)
zellij -s zjbar_test pipe --name zjbar -- \
  '{"source":"claude","pane_id":1,"session_id":"test-session","hook_event":"PreToolUse","tool_name":"Bash"}'

sleep 1
tmux -L zjbar_test capture-pane -t zjbar_test -p | tail -1
# Should show ⚡ icon on the tab

# Clean up
tmux -L zjbar_test kill-server
```

## Debugging AI Integration

### WASM plugin (Rust side)

- Use `eprintln!()` for debug output — it goes to Zellij's log file at `/tmp/zellij-<UID>/zellij-log/zellij.log`.
- To watch logs in real time: `tail -f /tmp/zellij-$(id -u)/zellij-log/zellij.log | grep -i zjbar`
- The `pipe()` method in `main.rs` receives all IPC messages. Add `eprintln!` there to inspect incoming payloads.
- Remember to remove all debug logging before committing.

### Claude Code integration

- **Hook registration**: Hooks are defined in `~/.claude/settings.json` (or `~/.codebuddy/settings.json` for CodeBuddy). The zjbar Claude Code plugin registers hooks automatically via `hooks/hooks.json`.
- **Hook script**: `scripts/zjbar-hook.sh` — receives hook event name and context JSON from stdin, formats and sends to `zellij pipe --name zjbar`.
- **Manual test**: Send a mock event directly:
  ```bash
  zellij -s <session> pipe --name zjbar -- \
    '{"source":"claude","pane_id":1,"session_id":"test","hook_event":"PreToolUse","tool_name":"Bash"}'
  ```
- **Hook events flow**: Claude Code → hook script (runs in separate process) → `zellij pipe` → WASM `pipe()` → `event_handler::handle_hook_event()` → Activity state update → re-render.

### Codex CLI integration

- **Integration method**: Codex uses `notify` config in `~/.codex/config.toml`. Unlike Claude Code hooks, Codex only has one event: `agent-turn-complete`. Override the Codex config directory with `CODEX_HOME` env var (default `~/.codex`).
- **Notify script**: `scripts/zjbar-codex-notify.sh` — receives JSON via `$1` (command-line argument, not stdin), maps `agent-turn-complete` to a `Stop` hook event, and sends to `zellij pipe --name zjbar`.
- **Install**: `make install-codex-hooks` or `scripts/install-codex-hooks.sh`. This copies the notify script and icon to `$CODEX_HOME/zjbar/` and adds `notify = ["$CODEX_HOME/zjbar/zjbar-codex-notify.sh"]` to `config.toml`. The repo can be safely deleted after installation.
- **Uninstall**: `make uninstall-codex-hooks` or `scripts/install-codex-hooks.sh --uninstall`. This removes the `$CODEX_HOME/zjbar/` directory and the `notify` entry from `config.toml`.
- **Limitations**: Only the Done (checkmark) state is supported — no PreToolUse, Thinking, or Waiting states since Codex doesn't expose granular hook events.
- **Summary extraction**: The `last-assistant-message` field from the Codex notification JSON is used directly for desktop notification summaries.
- **Manual test**: Send a mock event directly:
  ```bash
  zellij -s <session> pipe --name zjbar -- \
    '{"source":"codex","pane_id":0,"session_id":"test","hook_event":"Stop","tool_name":null}'
  ```
- **Events flow**: Codex agent-turn-complete → notify script (`$1` JSON) → `zellij pipe` → WASM `pipe()` → `event_handler::handle_hook_event()` → Activity::Done → re-render.

### Gemini CLI integration

- **Integration method**: Gemini CLI hooks are installed into `~/.gemini/settings.json` via `make install-gemini-hooks` (similar to Codex). This avoids the extension system's hooks conflict with Claude Code (both hardcode reading `hooks/hooks.json` but have incompatible event schemas).
- **Hook script**: `scripts/zjbar-gemini-hook.sh` — reads Gemini hook JSON from stdin, maps Gemini events to zjbar events, and sends to `zellij pipe --name zjbar`. Outputs `{}` to stdout (Gemini requires valid JSON on stdout).
- **Install**: `make install-gemini-hooks` or `scripts/install-gemini-hooks.sh`. This copies the hook script and icon to `~/.gemini/zjbar/` and registers hooks in `~/.gemini/settings.json`. The repo can be safely deleted after installation.
- **Uninstall**: `make uninstall-gemini-hooks` or `scripts/install-gemini-hooks.sh --uninstall`. This removes the `~/.gemini/zjbar/` directory and the hooks from `settings.json`.
- **Architecture note**: `hooks/hooks.json` is purely Claude Code format (used by Claude Code/CodeBuddy plugins). Gemini hooks are installed directly into `~/.gemini/settings.json` via the install script, not via the extension system. This clean separation avoids the event name conflict between Claude Code and Gemini CLI.
- **Event mapping**:
  | Gemini Hook | zjbar Event | Activity |
  |------------|-------------|----------|
  | SessionStart | SessionStart | Init (◆) |
  | BeforeAgent | UserPromptSubmit | Thinking (●) |
  | BeforeTool | PreToolUse | Tool (⚡/◉/✎/etc.) |
  | AfterTool | PostToolUse | Thinking (●) |
  | AfterAgent | Stop | Done (✓) |
  | SessionEnd | SessionEnd | (removed) |
- **Tool name mapping**: Gemini uses different tool names than Claude Code. The hook script maps them: `run_shell_command` → Bash, `read_file`/`read_many_files` → Read, `write_file` → Write, `edit_file`/`replace` → Edit, `web_fetch` → WebFetch, `google_web_search` → WebSearch.
- **Key difference from Claude Code**: Gemini hooks MUST output valid JSON to stdout (`{}`). Any non-JSON stdout causes parsing failure. All debug output must go to stderr.
- **Manual test**: Send a mock event directly:
  ```bash
  zellij -s <session> pipe --name zjbar -- \
    '{"source":"gemini","pane_id":0,"session_id":"test","hook_event":"PreToolUse","tool_name":"Bash"}'
  ```
- **Events flow**: Gemini CLI hook event → wrapper sets `ZJBAR_GEMINI_EVENT` → `zjbar-gemini-hook.sh` (stdin JSON) → map event + tool names → `zellij pipe` → WASM `pipe()` → `event_handler::handle_hook_event()` → Activity state update → re-render.

### OpenCode integration

- **Plugin source**: `opencode-plugin/src/index.ts`
- **Build**: `cd opencode-plugin && bun run build` → outputs to `opencode-plugin/dist/index.js`
- **Cache locations** (both must be updated when debugging locally):
  1. `~/.config/opencode/node_modules/zjbar-opencode/dist/index.js` (global config cache)
  2. `~/.cache/opencode/node_modules/zjbar-opencode/dist/index.js` (global cache)
- **Quick update all caches after rebuild**:
  ```bash
  cd opencode-plugin && bun run build
  cp dist/index.js ~/.config/opencode/node_modules/zjbar-opencode/dist/index.js 2>/dev/null
  cp dist/index.js ~/.cache/opencode/node_modules/zjbar-opencode/dist/index.js 2>/dev/null
  ```
- **Environment quirks**: OpenCode sets `ZELLIJ=0` inside Zellij, so `zellij pipe` without `-s <session>` fails silently. The plugin resolves the session name from `ZELLIJ_SESSION_NAME` env var and always passes `-s`.
- **Execution model difference**: OpenCode runs tool hooks in-process (not as separate shell commands like Claude Code). This means `tool.execute.before` and `tool.execute.after` fire within milliseconds of each other. Sending both PreToolUse and PostToolUse would cause the tool icon to be immediately overwritten by Thinking (●). The fix: only send PreToolUse, let the state naturally transition on `session.idle` (Stop).
- **Debug logging**: Temporarily add `appendFileSync("/tmp/zjbar-opencode.log", ...)` in `sendToZjbar()` to trace events. Remove before committing.

## Key Concepts

- **Rendering**: `render.rs` outputs raw ANSI escape codes via `print!()` in the `render()` method. Zellij captures stdout as pane content.
- **IPC**: AI tool hooks/plugins → `zellij pipe --name zjbar` → plugin's `pipe()` method. All integrations use a unified JSON payload with a `source` field. Claude Code uses `zjbar-hook.sh` (registered via plugin marketplace), Codex uses `zjbar-codex-notify.sh` (registered via `make install-codex-hooks`), OpenCode uses the `zjbar-opencode` npm package (`opencode-plugin/src/index.ts`), Gemini CLI uses `zjbar-gemini-hook.sh` (registered via `make install-gemini-hooks`).
- **Multi-instance sync**: Each tab has its own plugin instance. They sync state via `pipe_message_to_plugin()` with names like `zjbar:sync`, `zjbar:request`.
- **Configuration**: All visual and behavioral settings are parsed from the KDL layout plugin block via `BarConfig::from_kdl()` in `config.rs`. No runtime settings file.

## Conventions

- This file (`AGENTS.md`) is the single source of project instructions. `CLAUDE.md`, `CODEBUDDY.md`, and `GEMINI.md` are all **symlinks** to `AGENTS.md`. When committing, always `git add AGENTS.md` — never add the symlink names directly.
- All commit messages and code comments must be in **English**.
- The WASM target is `wasm32-wasip1` (configured in `.cargo/config.toml`).
- Release profile uses `opt-level = "s"` and LTO for minimal binary size.
- Color palette follows Tokyo Night. All color defaults are defined in `config.rs`.
- After any feature change, check if `README.md` needs updating (e.g. new config options, changed behavior, new install steps). If so, update both `README.md` (English) and `README.zh-CN.md` (Chinese) directly without asking for confirmation.

## Development Workflow: Fix → Release → Verify

Users install zjbar through plugin systems (Claude Code plugin marketplace, OpenCode npm plugin, CodeBuddy plugin). This means they can update to the latest version with a simple plugin update command and restart. **Always release immediately after fixing a bug or completing a feature** — do not batch multiple changes before releasing.

The workflow for every code change is:

1. **Fix/implement** the change
2. **Build & test** locally (tmux test for WASM rendering, deploy to OpenCode caches for OpenCode plugin)
3. **Bump version** in all 6 files (see checklist below)
4. **Build WASM** (`cargo build --release --target wasm32-wasip1`) to update `Cargo.lock`
5. **Commit, push, tag, `make release`** — this creates the GitHub Release and publishes the npm package
6. **Update local caches** for OpenCode plugin so the user can verify immediately:
   ```bash
   cp opencode-plugin/dist/index.js ~/.config/opencode/node_modules/zjbar-opencode/dist/index.js
   cp opencode-plugin/dist/index.js ~/.cache/opencode/node_modules/zjbar-opencode/dist/index.js
   ```
7. **Tell the user** the version number and what changed, so they can update and verify

This fast iteration loop allows the user to test fixes within seconds of a release by running plugin update commands (`/plugin update` for Claude Code/CodeBuddy, restart for OpenCode).

## Releasing a New Version

When creating a new release (e.g. bumping from `v1.0.4` to `v1.0.5`), update the version in **all** of these places:

1. **`Cargo.toml`** — `version = "x.y.z"`
2. **`README.md`** — WASM download URL in the layout example (`releases/download/vX.Y.Z/zjbar.wasm`)
3. **`README.zh-CN.md`** — same WASM download URL
4. **`.claude-plugin/marketplace.json`** — both `version` fields
5. **`.claude-plugin/plugin.json`** — `version` field
6. **`opencode-plugin/package.json`** — `version` field (must match the release version)
7. **`gemini-extension.json`** — `version` field
8. **`Cargo.lock`** — auto-updated by `cargo build`, commit when `Cargo.toml` version changes

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
