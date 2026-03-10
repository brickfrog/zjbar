# zjbar Development Guide

## Overview

zjbar is a Zellij WASM plugin that replaces the default tab bar with a Tokyo Night powerline-themed status bar, with optional Claude Code activity awareness.

## Architecture

```
src/
├── main.rs           # Plugin entry point (ZellijPlugin trait impl), event routing, state management
├── config.rs         # BarConfig struct, KDL config parser, color/mode/activity helpers
├── render.rs         # Status bar rendering with ANSI escape codes and powerline arrows
├── state.rs          # State types: Activity, SessionInfo, HookPayload, etc.
├── event_handler.rs  # Maps Claude Code hook events to Activity states
└── tab_pane_map.rs   # Maps pane IDs to (tab_index, tab_name) pairs
scripts/
├── zjbar-hook.sh     # Claude Code hook → zellij pipe bridge (embedded in WASM via include_str!)
└── install-hooks.sh  # Standalone hook installer (used by `make install-hooks`)
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

### Basic workflow

```bash
# 1. Build and install
cargo build --release --target wasm32-wasip1
cp target/wasm32-wasip1/release/zjbar.wasm ~/.config/zellij/plugins/

# 2. Start Zellij in a detached tmux session
tmux new-session -d -s zjbar_test -x 120 -y 30 \
  'zellij --layout layout.kdl'
sleep 2  # wait for Zellij to initialize

# 3. Check the status bar (last line of the pane)
tmux capture-pane -t zjbar_test -p | tail -1
```

### Creating tabs via `zellij action`

tmux intercepts `Ctrl+T` etc., so use `zellij action` from outside to manipulate tabs:

```bash
# Extract session name from status bar
SESSION=$(tmux capture-pane -t zjbar_test -p | tail -1 | awk '{print $1}')

# Create new tabs
ZELLIJ_SESSION_NAME=$SESSION zellij action new-tab
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

tmux new-session -d -s zjbar_custom -x 120 -y 30 \
  'zellij --layout /tmp/zjbar-test.kdl'
sleep 2
tmux capture-pane -t zjbar_custom -p -e | tail -1 | sed 's/\x1b/ESC/g'
# Confirm: 48;2;255;0;0 (session bg red), 48;2;0;255;0 (index bg green)
```

### Cleanup

```bash
tmux kill-session -t zjbar_test
```

## Key Concepts

- **Rendering**: `render.rs` outputs raw ANSI escape codes via `print!()` in the `render()` method. Zellij captures stdout as pane content.
- **IPC**: Claude Code hooks → `zjbar-hook.sh` → `zellij pipe --name zjbar` → plugin's `pipe()` method. Hook registration is manual via `make install-hooks`.
- **Multi-instance sync**: Each tab has its own plugin instance. They sync state via `pipe_message_to_plugin()` with names like `zjbar:sync`, `zjbar:request`.
- **Configuration**: All visual and behavioral settings are parsed from the KDL layout plugin block via `BarConfig::from_kdl()` in `config.rs`. No runtime settings file.

## Conventions

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

Use `grep -r 'releases/download/v' .` to verify all URLs are updated.
