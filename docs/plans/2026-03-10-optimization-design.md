# zjbar Optimization Design

Date: 2026-03-10

## Overview

Comprehensive optimization of the zjbar Zellij plugin addressing bugs, performance, and maintainability.

## P0: Fix `display_width` for CJK/wide characters

**Problem:** `render.rs:display_width()` uses `chars().count()`, which counts CJK characters as width 1 instead of 2. This causes click regions to be offset after any tab with wide characters in its name.

**Solution:** Add a `char_width()` function using Unicode East Asian Width properties. Characters in the CJK Unified Ideographs range (U+2E80–U+9FFF, U+F900–U+FAFF, U+FE30–U+FE4F, U+20000–U+2FA1F) and fullwidth forms (U+FF01–U+FF60, U+FFE0–U+FFE6) count as width 2. All others count as 1.

No external crate needed — a simple `match` on char ranges is sufficient and keeps the WASM size unchanged.

**Files:** `src/render.rs`

## P1: Fix Notification activity state never being set

**Problem:** `event_handler.rs:28-33` handles the `Notification` event by updating `last_event_ts` and returning early, never setting `session.activity = Activity::Notification`. The `◇` symbol and priority 4 defined in `render.rs` are dead code.

**Solution:** Set `activity = Activity::Notification` like other events, and let it flow through the normal session update path. Also trigger flash for Notification events (same as Waiting).

**Files:** `src/event_handler.rs`

## P2: Consolidate 6 jq calls into 1 in hook script

**Problem:** `zjbar-hook.sh:16-23` calls `jq -r` six times on the same `$INPUT` JSON, each time re-parsing the full document.

**Solution:** Replace with a single `jq` call using `@sh` output format to extract all fields at once into shell variables via `eval`.

```bash
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

**Files:** `scripts/zjbar-hook.sh`

## P3: Refactor `render_tabs` and eliminate duplicate session traversal

**Problem:** `render_tabs` is 250 lines with mixed concerns. Sessions are traversed twice for flash detection and waiting-pane click regions.

**Solution:**
1. Extract `compute_tab_layout()` for width calculation
2. Extract `render_single_tab()` for per-tab rendering
3. Pre-compute a struct per tab containing: best_session, is_flash_bright, waiting_pane_id — eliminating the second traversal

**Files:** `src/render.rs`

## P4: Zero-allocation ANSI color helpers

**Problem:** `fg()`, `fg_c()`, `bg_c()` each allocate a String via `format!()`. Called ~70 times per render cycle.

**Solution:** Replace with inline `write!` macros that write directly to the buffer:

```rust
macro_rules! write_fg {
    ($buf:expr, $c:expr) => {
        write!($buf, "\x1b[38;2;{};{};{}m", $c.0, $c.1, $c.2)
    };
}
```

Remove `fg()`, `fg_c()`, `bg_c()` functions entirely.

**Files:** `src/render.rs`

## Implementation Order

1. P0 — CJK display_width fix
2. P1 — Notification activity fix
3. P2 — Hook script jq consolidation
4. P3 — render_tabs refactor
5. P4 — ANSI macro conversion
