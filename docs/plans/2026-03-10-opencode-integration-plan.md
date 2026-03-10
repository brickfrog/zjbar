# OpenCode Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add OpenCode support to zjbar by creating an OpenCode JS plugin and making minimal changes to the existing codebase for multi-AI-tool compatibility.

**Architecture:** Translation layer outside — OpenCode JS plugin translates OpenCode events into the unified HookPayload JSON format. zjbar WASM plugin receives the same unified format from all AI tools. A `source` field is added to HookPayload for notification display purposes only.

**Tech Stack:** Rust (WASM plugin), JavaScript (OpenCode plugin), Bash (install script)

**Design doc:** `docs/plans/2026-03-10-opencode-integration-design.md`

---

### Task 1: Add `source` field to HookPayload

**Files:**
- Modify: `src/state.rs:47-56`

**Step 1: Add source field**

In `src/state.rs`, add `source: Option<String>` to the `HookPayload` struct:

```rust
#[derive(Debug, Deserialize)]
pub struct HookPayload {
    pub source: Option<String>,
    pub session_id: Option<String>,
    pub pane_id: u32,
    pub hook_event: String,
    pub tool_name: Option<String>,
    pub cwd: Option<String>,
    pub zellij_session: Option<String>,
    pub term_program: Option<String>,
}
```

Since `source` is `Option<String>` and `HookPayload` uses `#[derive(Deserialize)]`, existing JSON payloads that omit `source` will deserialize to `None` — fully backward compatible.

**Step 2: Build and verify compilation**

Run: `cargo build --target wasm32-wasip1`
Expected: Build succeeds with no errors.

**Step 3: Commit**

```bash
git add src/state.rs
git commit -m "feat: add source field to HookPayload for multi-AI-tool support"
```

---

### Task 2: Add `source` to zjbar-hook.sh payload

**Files:**
- Modify: `scripts/zjbar-hook.sh:29-45` (payload builder)
- Modify: `scripts/zjbar-hook.sh:153-159` (APP_NAME detection)

**Step 1: Add `source` arg to payload builder**

In `scripts/zjbar-hook.sh`, add `--arg source "claude"` to the `jq -nc` call (around line 29) and include `source: $source` in the JSON object:

```bash
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
```

**Step 2: Commit**

```bash
git add scripts/zjbar-hook.sh
git commit -m "feat: add source field to zjbar-hook.sh payload"
```

---

### Task 3: Create OpenCode JS plugin

**Files:**
- Create: `scripts/zjbar-opencode-plugin.js`

**Step 1: Write the plugin**

Create `scripts/zjbar-opencode-plugin.js`. The plugin must:

1. Listen to OpenCode events: `session.created`, `session.idle`, `session.deleted`, `permission.asked`, `tool.execute.before`, `tool.execute.after`
2. Map each event to the unified zjbar event name
3. Map OpenCode tool names (lowercase) to unified tool names (PascalCase)
4. Build the unified HookPayload JSON
5. Send via `zellij pipe --name zjbar`
6. Exit silently if not inside Zellij (`ZELLIJ_SESSION_NAME` env var not set)

Reference the OpenCode plugin API:
- Plugin signature: `export const ZjbarPlugin = async ({ $, directory }) => { return { ... } }`
- `$` is Bun's shell API for executing commands
- Event hooks are keyed by event name strings
- `tool.execute.before` receives `(input, output)` where `input.tool` is the tool name

Tool name mapping table:

| OpenCode | Unified |
|----------|---------|
| `bash`   | `Bash`  |
| `read`   | `Read`  |
| `edit`   | `Edit`  |
| `write`  | `Write` |
| `grep`   | `Grep`  |
| `glob`   | `Glob`  |
| `webfetch` | `WebFetch` |
| others   | Capitalize first letter |

Event mapping table:

| OpenCode Event | Unified Event |
|----------------|---------------|
| `session.created` | `SessionStart` |
| `session.idle` | `Stop` |
| `session.deleted` | `SessionEnd` |
| `permission.asked` | `PermissionRequest` |
| `tool.execute.before` | `PreToolUse` |
| `tool.execute.after` | `PostToolUse` |

Also handle desktop notifications within the plugin (bell for PermissionRequest, terminal-notifier/notify-send for Stop/PermissionRequest) following the same pattern as `zjbar-hook.sh`, but simpler since OpenCode doesn't provide transcript access.

**Step 2: Verify the JS syntax**

Run: `node -c scripts/zjbar-opencode-plugin.js`
Expected: No syntax errors.

**Step 3: Commit**

```bash
git add scripts/zjbar-opencode-plugin.js
git commit -m "feat: add OpenCode JS plugin for zjbar integration"
```

---

### Task 4: Create OpenCode install script

**Files:**
- Create: `scripts/install-opencode.sh`

**Step 1: Write the install script**

Create `scripts/install-opencode.sh` that:

1. Copies `scripts/zjbar-opencode-plugin.js` to `~/.config/opencode/plugins/zjbar-opencode-plugin.js`
2. Copies `assets/opencode-logo.png` to `~/.config/zellij/plugins/opencode-logo.png`
3. Creates directories if they don't exist
4. Supports `--uninstall` flag to remove both files
5. Prints clear success/error messages

Follow the same style as `scripts/install-hooks.sh` (set -euo pipefail, clear error messages, idempotent).

**Step 2: Make it executable**

Run: `chmod +x scripts/install-opencode.sh`

**Step 3: Test install and uninstall**

Run: `./scripts/install-opencode.sh`
Expected: Files copied, success message printed.

Run: `ls -la ~/.config/opencode/plugins/zjbar-opencode-plugin.js ~/.config/zellij/plugins/opencode-logo.png`
Expected: Both files exist.

Run: `./scripts/install-opencode.sh --uninstall`
Expected: Files removed, success message printed.

**Step 4: Commit**

```bash
git add scripts/install-opencode.sh
git commit -m "feat: add OpenCode plugin install script"
```

---

### Task 5: Update documentation

**Files:**
- Modify: `README.md` (around line 109, after "Claude Code Activity Symbols" section)
- Modify: `README.zh-CN.md` (corresponding location)

**Step 1: Add OpenCode section to README.md**

After the existing Claude Code integration content (the "Claude Code Activity Symbols" section and before "Configuration"), add an "OpenCode Integration" section that covers:

1. What it does (activity awareness for OpenCode)
2. Install steps: `./scripts/install-opencode.sh`
3. Brief mention that it uses the same status symbols as Claude Code
4. Link to OpenCode project

**Step 2: Add corresponding section to README.zh-CN.md**

Same content in Chinese.

**Step 3: Update CLAUDE.md Architecture section**

Update the `scripts/` section in CLAUDE.md to include the new files:

```
scripts/
├── zjbar-hook.sh           # Claude Code hook → zellij pipe bridge
├── zjbar-opencode-plugin.js # OpenCode plugin → zellij pipe bridge
├── install-hooks.sh        # Claude Code hook installer
└── install-opencode.sh     # OpenCode plugin installer
```

Also update the Overview line to mention multi-AI-tool support, not just Claude Code.

**Step 4: Commit**

```bash
git add README.md README.zh-CN.md CLAUDE.md
git commit -m "docs: add OpenCode integration documentation"
```

---

### Task 6: Build and integration test

**Step 1: Full build**

Run: `cargo build --release --target wasm32-wasip1`
Expected: Build succeeds.

**Step 2: Install the WASM plugin**

Run: `cp target/wasm32-wasip1/release/zjbar.wasm ~/.config/zellij/plugins/`

**Step 3: Test backward compatibility with Claude Code hooks**

Verify that the existing Claude Code hook still works by sending a test payload without `source`:

```bash
echo '{"pane_id":1,"session_id":"test","hook_event":"SessionStart","zellij_session":"'$ZELLIJ_SESSION_NAME'"}' | \
  zellij pipe --name zjbar
```

Expected: zjbar displays the Init activity indicator — no errors from missing `source` field.

**Step 4: Test OpenCode plugin payload format**

Send a test payload with `source`:

```bash
echo '{"source":"opencode","pane_id":1,"session_id":"test","hook_event":"PreToolUse","tool_name":"Bash","zellij_session":"'$ZELLIJ_SESSION_NAME'"}' | \
  zellij pipe --name zjbar
```

Expected: zjbar displays the Tool(Bash) activity indicator (⚡).

**Step 5: Reset test state**

```bash
echo '{"pane_id":1,"session_id":"test","hook_event":"SessionEnd","zellij_session":"'$ZELLIJ_SESSION_NAME'"}' | \
  zellij pipe --name zjbar
```

**Step 6: Commit any fixes if needed**
