# zjbar Optimization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix CJK click-region bug, enable Notification activity, optimize hook script, refactor render_tabs, and eliminate ANSI string allocations.

**Architecture:** Five independent improvements applied sequentially to the existing codebase. Each task is a self-contained commit. No new dependencies.

**Tech Stack:** Rust (wasm32-wasip1), Bash/jq, Zellij plugin API

---

### Task 1: Fix `display_width` for CJK/wide characters (P0 — Bug)

**Files:**
- Modify: `src/render.rs:56-58`

**Step 1: Replace `display_width` with CJK-aware version**

Replace the function at `src/render.rs:56-58`:

```rust
fn char_width(c: char) -> usize {
    let cp = c as u32;
    // CJK Radicals, Kangxi, Ideographic, CJK Unified, Compatibility, Extensions
    if (0x2E80..=0x9FFF).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0xFE30..=0xFE4F).contains(&cp)
        || (0xFF01..=0xFF60).contains(&cp)
        || (0xFFE0..=0xFFE6).contains(&cp)
        || (0x20000..=0x2FA1F).contains(&cp)
        || (0x30000..=0x323AF).contains(&cp)
    {
        2
    } else {
        1
    }
}

fn display_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}
```

**Step 2: Build and verify**

Run: `cargo build --release --target wasm32-wasip1`
Expected: Compiles without warnings.

**Step 3: Manual test with CJK tab names**

1. Install the new WASM: `cp target/wasm32-wasip1/release/zjbar.wasm ~/.config/zellij/plugins/`
2. Open Zellij, rename a tab to include Chinese characters (e.g. "错的 buddy 哈哈")
3. Click tabs to the right of the CJK-named tab — verify click targets are correct

**Step 4: Commit**

```bash
git add src/render.rs
git commit -m "fix: correct display_width for CJK/wide characters

display_width used chars().count() which counts CJK characters as
width 1 instead of 2, causing click regions to be offset after any
tab with wide characters in its name."
```

---

### Task 2: Fix Notification activity state (P1 — Bug)

**Files:**
- Modify: `src/event_handler.rs:28-33`

**Step 1: Change Notification handler to set activity normally**

In `src/event_handler.rs`, replace lines 28-33:

```rust
        "Notification" => {
            if let Some(session) = state.sessions.get_mut(&payload.pane_id) {
                session.last_event_ts = crate::state::unix_now();
            }
            return;
        }
```

With simply:

```rust
        "Notification" => Activity::Notification,
```

This lets Notification flow through the normal session update path below (lines 45-86), which sets `last_event_ts`, `activity`, flash deadlines, etc.

**Step 2: Add flash trigger for Notification events**

In `src/event_handler.rs`, change the flash condition at line 58 from:

```rust
    if matches!(activity, Activity::Waiting) {
```

To:

```rust
    if matches!(activity, Activity::Waiting | Activity::Notification) {
```

**Step 3: Build and verify**

Run: `cargo build --release --target wasm32-wasip1`
Expected: Compiles without warnings.

**Step 4: Commit**

```bash
git add src/event_handler.rs
git commit -m "fix: enable Notification activity state on status bar

Notification events were handled with early return, never setting
Activity::Notification on the session. The ◇ symbol was dead code.
Now flows through normal update path with flash support."
```

---

### Task 3: Consolidate hook script jq calls (P2 — Performance)

**Files:**
- Modify: `scripts/zjbar-hook.sh:15-23`

**Step 1: Replace 6 individual jq calls with single eval**

In `scripts/zjbar-hook.sh`, replace lines 15-23:

```bash
# Extract fields with jq (required dependency)
HOOK_EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty')
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // empty')
CWD=$(echo "$INPUT" | jq -r '.cwd // empty')
TRANSCRIPT_PATH=$(echo "$INPUT" | jq -r '.transcript_path // empty')
# Notification event has message/title directly in input
NOTIF_MESSAGE=$(echo "$INPUT" | jq -r '.message // empty')
NOTIF_TITLE=$(echo "$INPUT" | jq -r '.title // empty')
```

With:

```bash
# Extract all fields in a single jq call (required dependency)
eval "$(echo "$INPUT" | jq -r '
  @sh "HOOK_EVENT=\(.hook_event_name // "")",
  @sh "SESSION_ID=\(.session_id // "")",
  @sh "TOOL_NAME=\(.tool_name // "")",
  @sh "CWD=\(.cwd // "")",
  @sh "TRANSCRIPT_PATH=\(.transcript_path // "")",
  @sh "NOTIF_MESSAGE=\(.message // "")",
  @sh "NOTIF_TITLE=\(.title // "")"
')"
```

**Step 2: Verify jq output format locally**

Run this to confirm correct output:

```bash
echo '{"hook_event_name":"Stop","session_id":"abc","tool_name":null,"cwd":"/tmp","transcript_path":"/tmp/t.jsonl","message":null,"title":null}' | jq -r '
  @sh "HOOK_EVENT=\(.hook_event_name // "")",
  @sh "SESSION_ID=\(.session_id // "")",
  @sh "TOOL_NAME=\(.tool_name // "")",
  @sh "CWD=\(.cwd // "")",
  @sh "TRANSCRIPT_PATH=\(.transcript_path // "")",
  @sh "NOTIF_MESSAGE=\(.message // "")",
  @sh "NOTIF_TITLE=\(.title // "")"
'
```

Expected output (shell-safe quoted strings):

```
HOOK_EVENT='Stop' SESSION_ID='abc' TOOL_NAME='' CWD='/tmp' TRANSCRIPT_PATH='/tmp/t.jsonl' NOTIF_MESSAGE='' NOTIF_TITLE=''
```

**Step 3: Commit**

```bash
git add scripts/zjbar-hook.sh
git commit -m "perf: consolidate 6 jq calls into 1 in hook script

Each hook invocation was parsing the same JSON 6 times. Now uses a
single jq call with @sh output to extract all fields at once."
```

---

### Task 4: Refactor `render_tabs` — pre-compute tab render info (P3 — Maintainability)

**Files:**
- Modify: `src/render.rs`

**Step 1: Add `TabRenderInfo` struct**

Add after the `format_elapsed` function (around line 72), before `render_status_bar`:

```rust
struct TabRenderInfo<'a> {
    best_session: Option<&'a SessionInfo>,
    is_flash_bright: bool,
    waiting_pane_id: Option<u32>,
    elapsed_str: Option<String>,
}
```

**Step 2: Extract `compute_tab_info` function**

Add a new function that pre-computes all per-tab data in a single pass over sessions:

```rust
fn compute_tab_info<'a>(
    state: &'a State,
    tabs: &[&TabInfo],
    now_s: u64,
    now_ms: u64,
) -> Vec<TabRenderInfo<'a>> {
    tabs.iter()
        .map(|tab| {
            let tab_sessions: Vec<&SessionInfo> = state
                .sessions
                .values()
                .filter(|s| s.tab_index == Some(tab.position))
                .collect();

            let best_session = tab_sessions
                .iter()
                .copied()
                .max_by_key(|s| activity_priority(&s.activity));

            let is_flash_bright = tab_sessions.iter().any(|s| {
                state
                    .flash_deadlines
                    .get(&s.pane_id)
                    .map(|&deadline| now_ms < deadline && (now_ms / 250) % 2 == 0)
                    .unwrap_or(false)
            });

            let waiting_pane_id = tab_sessions
                .iter()
                .find(|s| matches!(s.activity, Activity::Waiting))
                .map(|s| s.pane_id);

            let elapsed_str = if !state.settings.elapsed_time {
                None
            } else {
                best_session.and_then(|s| {
                    let elapsed = now_s.saturating_sub(s.last_event_ts);
                    if elapsed >= ELAPSED_THRESHOLD {
                        Some(format_elapsed(elapsed))
                    } else {
                        None
                    }
                })
            };

            TabRenderInfo {
                best_session,
                is_flash_bright,
                waiting_pane_id,
                elapsed_str,
            }
        })
        .collect()
}
```

**Step 3: Update `render_tabs` to use `TabRenderInfo`**

Replace the `best_sessions` and `elapsed_strs` computation (lines 290-317) with:

```rust
    let tab_infos = compute_tab_info(state, &tabs, now_s, now_ms);
```

Then update the loop body to use `tab_infos[i]` instead of `best_sessions[i]` / `elapsed_strs[i]`, and use `info.waiting_pane_id` / `info.is_flash_bright` instead of the inline session traversals at lines 360-370 and 493-498.

Key replacements in the loop:
- `let session = best_sessions[i];` → `let session = info.best_session;`
- `let is_claude = session.is_some();` stays the same
- Remove the `is_flash_bright` computation block (lines 360-370), use `info.is_flash_bright`
- Replace click region `waiting_session` lookup (lines 493-498) with `info.waiting_pane_id`
- `elapsed_strs[i]` → `info.elapsed_str`

Also update the layout overhead computation (lines 319-347) to use `tab_infos` for `claude_overhead` and `elapsed_overhead`.

**Step 4: Build and verify**

Run: `cargo build --release --target wasm32-wasip1`
Expected: Compiles without warnings.

**Step 5: Manual test**

Install and visually verify status bar renders identically to before. Test: multiple tabs, CJK names, active Claude sessions, flash animation.

**Step 6: Commit**

```bash
git add src/render.rs
git commit -m "refactor: pre-compute tab render info, eliminate duplicate session traversal

Introduces TabRenderInfo struct and compute_tab_info() to consolidate
per-tab session lookups into a single pass. Removes duplicate traversals
for flash detection and waiting-pane click regions."
```

---

### Task 5: Zero-allocation ANSI color macros (P4 — Performance)

**Files:**
- Modify: `src/render.rs`

**Step 1: Add write macros at the top of the file**

Replace the `fg()`, `fg_c()`, `bg_c()` functions (lines 44-54) with macros:

```rust
macro_rules! write_fg {
    ($buf:expr, $r:expr, $g:expr, $b:expr) => {
        let _ = write!($buf, "\x1b[38;2;{};{};{}m", $r, $g, $b)
    };
    ($buf:expr, $c:expr) => {
        let _ = write!($buf, "\x1b[38;2;{};{};{}m", $c.0, $c.1, $c.2)
    };
}

macro_rules! write_bg {
    ($buf:expr, $c:expr) => {
        let _ = write!($buf, "\x1b[48;2;{};{};{}m", $c.0, $c.1, $c.2)
    };
}
```

**Step 2: Update all call sites**

This is a mechanical replacement throughout `render.rs`. The pattern is:

Before: `let _ = write!(buf, "{}{}...", fg_c(color), bg_c(color2), ...);`
After: Replace the format args with inline ANSI writes.

For example, `render_status_bar` line 109-112:

Before:
```rust
let _ = write!(
    buf,
    "{}{}{BOLD}{session_pill_text}{RESET}",
    bg_c(cfg.session_bg), fg_c(cfg.session_fg),
);
```

After:
```rust
write_bg!(buf, cfg.session_bg);
write_fg!(buf, cfg.session_fg);
let _ = write!(buf, "{BOLD}{session_pill_text}{RESET}");
```

Apply the same pattern to all ~20 call sites in `render_status_bar`, `render_tabs`, and `render_settings_menu`.

Also remove the standalone `fg()` function (used in `render_settings_menu` lines 175-177) — replace those with `write_fg!(buf, r, g, b)` using the 3-arg variant.

**Step 3: Build and verify**

Run: `cargo build --release --target wasm32-wasip1`
Expected: Compiles without warnings. No unused function warnings.

**Step 4: Commit**

```bash
git add src/render.rs
git commit -m "perf: replace ANSI color functions with zero-allocation write macros

fg(), fg_c(), bg_c() each allocated a String per call (~70 per render).
Now writes ANSI codes directly to the buffer via macros."
```

---

## Post-Implementation

After all 5 tasks, run a final build and manual verification:

```bash
cargo build --release --target wasm32-wasip1
cp target/wasm32-wasip1/release/zjbar.wasm ~/.config/zellij/plugins/
```

Verify:
1. CJK tab names — click regions align correctly
2. Notification events — `◇` symbol appears, flash works
3. Normal operation — tabs render, mode switching, elapsed time
4. Settings menu — all toggles work
