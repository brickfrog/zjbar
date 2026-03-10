# OpenCode Integration Design

## Goal

Add OpenCode support to zjbar as the first step toward multi-AI-tool compatibility. Use a unified event protocol so that zjbar plugin itself requires minimal changes, and each AI tool provides its own external translation layer.

## Architecture: Translation Layer Outside

```
┌─────────────────┐    ┌──────────────────┐    ┌────────────┐
│ Claude Code     │    │ zjbar-hook.sh    │    │            │
│ (hook events)   │───→│ translate→JSON   │───→│            │
└─────────────────┘    └──────────────────┘    │            │
                                                │  zjbar     │
┌─────────────────┐    ┌──────────────────┐    │  plugin    │
│ OpenCode        │    │ JS plugin        │    │            │
│ (plugin events) │───→│ translate→JSON   │───→│ (unchanged)│
└─────────────────┘    └──────────────────┘    └────────────┘
```

Each AI tool has its own hook script/plugin that translates events into a **unified HookPayload** JSON format. The zjbar WASM plugin processes only the unified format.

## Unified HookPayload Protocol

Add a `source` field to the existing `HookPayload`:

```rust
pub struct HookPayload {
    pub source: Option<String>,     // NEW: "claude", "opencode", etc.
    pub session_id: Option<String>,
    pub pane_id: u32,
    pub hook_event: String,         // Unified event names (see below)
    pub tool_name: Option<String>,
    pub cwd: Option<String>,
    pub zellij_session: Option<String>,
    pub term_program: Option<String>,
}
```

`source` is `Option<String>` for backward compatibility — existing Claude Code hooks that omit it default to `None`.

### Standard Event Names

All external hooks/plugins translate their tool-specific events into these names:

| Unified Event | Description |
|---|---|
| `SessionStart` | AI session created |
| `SessionEnd` | AI session ended |
| `PreToolUse` | Tool execution starting |
| `PostToolUse` | Tool execution finished |
| `UserPromptSubmit` | User submitted a prompt |
| `PermissionRequest` | Permission requested |
| `Notification` | General notification |
| `Stop` | AI finished responding |
| `SubagentStop` | Subagent finished |

### Standard Tool Names

| Unified Name | Description |
|---|---|
| `Bash` | Shell command execution |
| `Read` | File reading |
| `Edit` | File editing |
| `Write` | File writing |
| `Grep` | Content search |
| `Glob` | File pattern search |
| `WebSearch` / `WebFetch` | Web operations |
| `Task` | Subagent/task |

## OpenCode Event Mapping

### Events

| OpenCode Event | → Unified Event |
|---|---|
| `session.created` | `SessionStart` |
| `session.idle` | `Stop` |
| `session.deleted` | `SessionEnd` |
| `permission.asked` | `PermissionRequest` |
| `tool.execute.before` | `PreToolUse` |
| `tool.execute.after` | `PostToolUse` |

### Tool Names

| OpenCode Tool | → Unified Name |
|---|---|
| `bash` | `Bash` |
| `read` | `Read` |
| `edit` | `Edit` |
| `write` | `Write` |
| `grep` | `Grep` |
| `glob` | `Glob` |
| `webfetch` | `WebFetch` |
| Others | Capitalize first letter |

## Changes Required

### zjbar Plugin (Rust) — Minimal

1. **`state.rs`**: Add `source: Option<String>` to `HookPayload` (1 line)
2. **`event_handler.rs`**: No changes
3. **`render.rs`**: No changes

### zjbar-hook.sh — Small

1. Add `--arg source "claude"` to the payload JSON builder
2. Use `source` for notification `APP_NAME` derivation (alongside existing CodeBuddy detection)

### New Files

1. **`scripts/zjbar-opencode-plugin.js`** — OpenCode JS plugin that:
   - Listens to OpenCode events
   - Translates to unified HookPayload JSON
   - Pipes to zjbar via `zellij pipe --name zjbar`
2. **`scripts/install-opencode.sh`** — Installs the JS plugin to `~/.config/opencode/plugins/` and copies `opencode-logo.png` to `~/.config/zellij/plugins/`

### Documentation

- `README.md` / `README.zh-CN.md`: Add OpenCode integration section

## Notification Icons

Pre-existing assets in `assets/`:
- `claude-logo.png` — Claude Code notifications
- `codebuddy-logo.png` — CodeBuddy notifications
- `opencode-logo.png` — OpenCode notifications

The notification icon is selected based on the `source` field in the payload.

## Future Extensibility

Adding a new AI tool (e.g., Codex, Gemini CLI) requires:
1. Write a new hook script/plugin for that tool
2. Map its events to the unified event names
3. Send the unified HookPayload JSON via `zellij pipe --name zjbar`

No changes to the zjbar WASM plugin are needed.
