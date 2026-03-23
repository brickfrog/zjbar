# Phase 1: Rust 核心质量与健壮性 - Research

**Researched:** 2025-03-20
**Domain:** Rust WASM plugin refactoring, error handling, state machine, graceful degradation
**Confidence:** HIGH

## Summary

Phase 1 focuses on improving the internal quality of the zjbar Rust WASM plugin without changing external behavior. The codebase is compact (~1100 lines of Rust across 6 files) with a clear architecture, making targeted refactoring feasible. The main challenge is the testing constraint: `zellij_tile` depends on WASM host functions (`_host_run_plugin_command`), so unit tests cannot link on native targets. The existing 28 tests only cover pure utility functions in `config.rs` (11 tests) and `render.rs` (17 tests) that don't call Zellij APIs.

The key refactoring areas are: (1) `render.rs` (718 lines) where `render_status_bar()`, `render_tabs()`, and `render_single_tab()` are already reasonably structured but width computation is entangled with ANSI output; (2) `main.rs` `pipe()` method silently discards JSON parse errors; (3) `event_handler.rs` has no state transition validation — any event can set any Activity state; (4) narrow terminal rendering (< 50 cols) produces empty bars.

**Primary recommendation:** Split work into two plans — (Plan 1) render refactoring + degradation rendering, (Plan 2) error handling + state machine — since render changes are the prerequisite for Phase 3 testing.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| QUAL-01 | 将 render_status_bar() 拆分为更小的独立函数 | render.rs 已有 `render_status_bar()` → `render_tabs()` → `render_single_tab()` 结构，需要进一步提取前缀渲染和填充逻辑为独立函数 |
| QUAL-02 | 将 render_tabs() 的宽度计算逻辑提取为独立的计算层 | 宽度计算（行 561-588）与渲染混合，需提取为纯函数 `compute_tab_widths()` 返回 `TabWidthBudget` |
| QUAL-03 | 替换 unwrap_or_default() 为带 eprintln! 日志的错误处理 | 识别出 main.rs 3 处、event_handler.rs 2 处、state.rs 2 处需要处理 |
| QUAL-04 | 替换 serde 反序列化静默失败为带警告日志的处理 | pipe() 方法 4 处 serde 操作静默失败，load_config 1 处 |
| QUAL-05 | 为 Activity 状态转换添加显式验证函数 | Activity 枚举有 9 个变体，当前无转换约束，需定义合法转换矩阵 |
| QUAL-06 | 将分散的状态转换逻辑集中到 event_handler.rs | 状态转换在 event_handler.rs（hook 事件）和 main.rs（超时清理）两处，需统一 |
| RBST-01 | 在 pipe() 中为 JSON 解析失败添加 eprintln! 警告日志 | main.rs:174-176 的 `Err(_) => return false` 需改为记录错误详情 |
| RBST-02 | 验证 HookPayload 必需字段（hook_event），记录不完整负载 | HookPayload 的 hook_event 是 String，空字符串也能通过反序列化 |
| RBST-07 | 实现最小化降级渲染 | render_status_bar() 行 184-189 在 cols < 5 时只显示空白，需改为最小信息渲染 |
| RBST-08 | 为极窄终端（< 50 列）提供有意义的最小渲染 | 行 234-246 有部分降级（省略 mode pill），但 < 5 列时完全空白 |
</phase_requirements>

## Standard Stack

### Core (Already in Use — No New Dependencies)

| Library | Version | Purpose | Constraint |
|---------|---------|---------|------------|
| zellij-tile | 0.43.1 | WASM plugin API | Fixed by project |
| serde | 1.x | Serialization/deserialization | Already a dependency |
| serde_json | 1.x | JSON parsing for IPC | Already a dependency |

### Tooling

| Tool | Version | Purpose |
|------|---------|---------|
| rustc | 1.94.0 stable | Compiler (pinned in rust-toolchain.toml) |
| cargo test | built-in | Unit tests (WASM target only) |
| cargo clippy | built-in | Lint checks |
| cargo fmt | built-in | Code formatting |

**No new dependencies.** This is a hard constraint from the project requirements. All refactoring must use `std` and existing dependencies only.

## Architecture Patterns

### Current Render Architecture (render.rs — 718 lines)

```
render_status_bar()          — 90 lines, orchestrator
├── [inline] prefix render   — session pill + mode pill + arrows (lines 192-246)
├── render_tabs()            — 70 lines, tab loop
│   ├── compute_tab_info()   — 54 lines, per-tab activity calculation
│   └── render_single_tab()  — 148 lines, single tab segment
├── render_settings_menu()   — 68 lines, settings view
│   └── render_menu_item()   — 32 lines, reusable menu item
└── [inline] fill remaining  — pad with bar_bg (lines 256-261)
```

### Recommended Refactored Architecture

```
render_status_bar()              — 60 lines, thin orchestrator
├── render_prefix()              — NEW: session pill + mode pill + arrows
│   ├── render_full_prefix()     — full prefix (session + mode)
│   └── render_minimal_prefix()  — session only (narrow terminal)
├── render_tabs()                — existing, with width calc extracted
│   ├── compute_tab_widths()     — NEW: pure function returning TabWidthBudget
│   ├── compute_tab_info()       — existing, unchanged
│   └── render_single_tab()      — existing, unchanged
├── render_settings_menu()       — existing, unchanged
├── render_degraded()            — NEW: minimal render for errors/narrow terminals
└── fill_remaining()             — NEW: trivial but extracted for clarity
```

### Pattern 1: Width Computation Separation

**What:** Extract width calculations into a pure struct that can be tested independently.

**When to use:** Whenever render logic mixes "how wide is this?" with "output ANSI codes."

**Example:**
```rust
/// Width budget computed before any ANSI output.
struct TabWidthBudget {
    max_name_len: usize,
    fixed_per_tab: Vec<usize>,  // fixed overhead per tab
    total_overhead: usize,
}

fn compute_tab_widths(
    tabs: &[&TabInfo],
    tab_infos: &[TabRenderInfo],
    cfg: &BarConfig,
    available_cols: usize,
) -> TabWidthBudget {
    // Pure computation, no side effects, fully testable
    // Move lines 561-588 from render_tabs() here
}
```

### Pattern 2: Activity State Machine with Transition Validation

**What:** Explicit state transition matrix that validates whether a transition is legal.

**When to use:** Before applying any Activity state change.

**Example:**
```rust
impl Activity {
    /// Returns true if transitioning from `self` to `target` is a valid
    /// state transition. Invalid transitions are logged and rejected.
    pub fn can_transition_to(&self, target: &Activity) -> bool {
        use Activity::*;
        match (self, target) {
            // Any state can receive SessionStart (re-init)
            (_, Init) => true,
            // Idle/Init/Done/AgentDone can start thinking
            (Idle | Init | Done | AgentDone | Thinking | Tool(_), Thinking) => true,
            // Thinking can start using a tool
            (Thinking | Tool(_) | Init, Tool(_)) => true,
            // Various states can transition to Waiting/Notification
            (Thinking | Tool(_), Waiting | Notification) => true,
            // Stop event → Done from any active state
            (Init | Thinking | Tool(_) | Prompting | Waiting | Notification, Done) => true,
            // Done → Idle (timeout), Done → AgentDone
            (Done, Idle | AgentDone) => true,
            (AgentDone, Idle) => true,
            // Prompting from user input
            (Idle | Done | AgentDone | Thinking, Prompting) => true,
            // SessionEnd removes session (handled separately)
            _ => false,
        }
    }
}
```

### Pattern 3: Graceful Degradation Rendering

**What:** Progressive rendering that always shows something meaningful, regardless of terminal width.

**When to use:** In `render_status_bar()` as the first decision point.

**Example:**
```rust
fn render_status_bar(state: &mut State, _rows: usize, cols: usize) {
    // ... setup ...
    
    if cols < 3 {
        // Absolute minimum: just fill with bg color
        render_minimal_fill(buf, cols, &cfg.bar_bg);
    } else if cols < 10 {
        // Ultra-narrow: mode indicator only (e.g., "N" for Normal)
        render_mode_indicator_only(buf, cols, cfg, state.input_mode);
    } else if cols < 30 {
        // Narrow: session name (truncated) + mode indicator
        render_narrow_prefix(buf, cols, cfg, state);
    } else {
        // Normal: full render
        render_full(buf, cols, cfg, state);
    }
}
```

### Anti-Patterns to Avoid

- **Changing render output format:** The ANSI escape code structure is correct and working. Do NOT change the output format, only refactor the internal code organization.
- **Adding `Result<>` returns to render functions:** Render functions write to a `String` buffer. `write!()` to a `String` never fails. Don't add unnecessary error handling where it's impossible to fail.
- **Moving functions between files unnecessarily:** Keep `render.rs` functions in `render.rs`. The refactoring is about splitting large functions into smaller ones within the same module.
- **Breaking the IPC protocol:** HookPayload JSON format must remain backward compatible. Validation should reject invalid payloads gracefully, not change the expected format.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| ANSI escape codes | Custom escape builder | Existing `write_fg!`/`write_bg!` macros | They work correctly and are compact |
| Unicode width | Full Unicode width library | Existing `char_width()` function | Adding `unicode-width` crate violates no-new-dependency constraint |
| State machine framework | Full FSM library | Simple `can_transition_to()` method | 9 states is small enough for a match expression |
| Logging framework | Custom log system | `eprintln!()` macro | Goes to Zellij log file, standard WASM debugging pattern |

**Key insight:** This project has only 3 dependencies and targets minimal binary size. The refactoring must improve code organization without adding dependencies.

## Common Pitfalls

### Pitfall 1: Breaking WASM Build by Referencing Native-Only APIs
**What goes wrong:** Using `std` APIs not available in `wasm32-wasip1` (e.g., `std::fs`, `std::net`, `std::thread`).
**Why it happens:** Easy to forget target is WASM, not native.
**How to avoid:** Always build with `cargo build --target wasm32-wasip1` after changes. The project's `.cargo/config.toml` sets `wasm32-wasip1` as default target.
**Warning signs:** Compile errors about missing imports.

### Pitfall 2: Tests Can't Run Because of zellij_tile Host Functions
**What goes wrong:** Adding tests that directly or indirectly call `focus_terminal_pane()`, `switch_tab_to()`, `pipe_message_to_plugin()`, `run_command()`, `set_timeout()`, `subscribe()`, `request_permission()`, `set_selectable()` — these require the Zellij WASM host.
**Why it happens:** The binary links against `zellij_tile` which uses `extern "C"` host functions. On native target, linker fails. On WASM target without Zellij runtime, execution panics.
**How to avoid:** Only write tests for pure functions that don't call Zellij APIs. For code that does call Zellij APIs, verify via tmux integration testing. The existing test approach (28 tests on utility functions) is correct.
**Warning signs:** Link errors mentioning `_host_run_plugin_command`, test binary failing with exit code 126.

### Pitfall 3: Render Regression Due to Off-by-One in Width Calculation
**What goes wrong:** Extracting width computation changes the arithmetic, causing status bar to wrap or truncate.
**Why it happens:** Width calculation has many interacting components (prefix, separators, tab names, indicators, padding).
**How to avoid:** After refactoring, run the full tmux test suite to visually verify output matches pre-refactoring behavior. Capture before/after screenshots.
**Warning signs:** Status bar wrapping to second line, missing trailing characters, tabs disappearing.

### Pitfall 4: Silent Error Swallowing When Adding Logging
**What goes wrong:** Adding `eprintln!()` logging but still silently returning default values, making the logging useless.
**Why it happens:** The existing pattern is `unwrap_or_default()` which hides failures.
**How to avoid:** When replacing `unwrap_or_default()`, add the log BEFORE the fallback, and include enough context (what failed, what input caused it).
**Warning signs:** Errors logged but no one checks Zellij log file.

### Pitfall 5: State Transition Validation Too Strict
**What goes wrong:** Rejecting valid state transitions that occur in practice due to timing or protocol differences between AI tools.
**Why it happens:** Different AI tools (Claude, Codex, Gemini, OpenCode) send different event sequences. Codex only sends `Stop`. OpenCode skips `PostToolUse`.
**How to avoid:** Design the transition validator to be permissive for incoming events, logging warnings for unexpected transitions but still applying them. Only truly impossible transitions should be rejected.
**Warning signs:** AI activity indicators stop updating after refactoring.

## Code Examples

### Example 1: Adding Error Logging to JSON Parse (RBST-01)

**Current code (main.rs:174-176):**
```rust
let payload: HookPayload = match serde_json::from_str(payload_str) {
    Ok(p) => p,
    Err(_) => return false,  // Silent failure
};
```

**Refactored:**
```rust
let payload: HookPayload = match serde_json::from_str(payload_str) {
    Ok(p) => p,
    Err(e) => {
        eprintln!("[zjbar] failed to parse hook payload: {e}");
        return false;
    }
};
```

### Example 2: HookPayload Validation (RBST-02)

**After deserialization, validate required fields:**
```rust
let payload: HookPayload = match serde_json::from_str(payload_str) {
    Ok(p) => p,
    Err(e) => {
        eprintln!("[zjbar] failed to parse hook payload: {e}");
        return false;
    }
};

if payload.hook_event.is_empty() {
    eprintln!("[zjbar] received payload with empty hook_event, ignoring");
    return false;
}
```

### Example 3: Replacing unwrap_or_default() with Logged Fallback (QUAL-03)

**Current code (main.rs:305-306):**
```rust
msg.message_payload =
    Some(serde_json::to_string(&self.sessions).unwrap_or_default());
```

**Refactored:**
```rust
msg.message_payload = Some(match serde_json::to_string(&self.sessions) {
    Ok(json) => json,
    Err(e) => {
        eprintln!("[zjbar] failed to serialize sessions for sync: {e}");
        String::from("{}")
    }
});
```

### Example 4: Extracting Prefix Render (QUAL-01)

**Extract from render_status_bar() lines 192-246:**
```rust
/// Render the left prefix (session pill + mode pill + powerline arrows).
/// Returns the number of columns consumed.
fn render_prefix(
    buf: &mut String,
    cols: usize,
    cfg: &BarConfig,
    session_text: &str,
    mode_bg: Color,
    mode_fg: Color,
    mode_text: &str,
) -> (usize, Option<(usize, usize)>) {
    // ... prefix rendering logic extracted from render_status_bar()
    // Returns (columns_consumed, prefix_click_region)
}
```

### Example 5: Narrow Terminal Degraded Rendering (RBST-07, RBST-08)

```rust
/// Minimal rendering for very narrow terminals (< 30 cols).
/// Shows at least: mode indicator character + truncated session name.
fn render_degraded(
    buf: &mut String,
    cols: usize,
    cfg: &BarConfig,
    mode: InputMode,
    session_name: &str,
) {
    let (_mode_bg, _mode_fg, mode_text) = cfg.mode_style(mode);
    let mode_char = &mode_text[..1]; // "N", "L", "P", etc.
    
    write_bg!(buf, cfg.bar_bg);
    write_fg!(buf, cfg.session_fg);
    
    if cols >= 5 {
        // " N session_trun..."
        let _ = write!(buf, " ");
        write_fg!(buf, cfg.mode_normal_bg); // mode color as fg
        let _ = write!(buf, "{mode_char}");
        let _ = write!(buf, " ");
        // Fill remaining with truncated session name
        let remaining = cols - 3;
        let mut used = 0;
        for c in session_name.chars() {
            let w = char_width(c);
            if used + w > remaining { break; }
            buf.push(c);
            used += w;
        }
        // Pad
        for _ in 0..(remaining - used) {
            buf.push(' ');
        }
    } else {
        // cols < 5: just fill with spaces
        let _ = write!(buf, "{:width$}", "", width = cols);
    }
    let _ = write!(buf, "{RESET}");
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single monolithic render function | Already split into render_status_bar → render_tabs → render_single_tab | Before this project | Good structure exists, needs further extraction |
| Silent JSON parse failures | Current state — all failures are silent | Current | Needs improvement (RBST-01, QUAL-04) |
| No state transition validation | Current state — any event sets any state | Current | Needs addition (QUAL-05, QUAL-06) |
| Empty bar on narrow terminals | Current state — cols < 5 shows blank | Current | Needs degraded rendering (RBST-07, RBST-08) |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `cargo test` (libtest) |
| Config file | `.cargo/config.toml` (sets default target to wasm32-wasip1) |
| Quick run command | `cargo test --target wasm32-wasip1` (requires WASM runner) |
| Full suite command | `cargo test --target wasm32-wasip1` + tmux integration test |

**Critical constraint:** Tests currently cannot run because:
1. `.cargo/config.toml` sets default target to `wasm32-wasip1`
2. WASM test binary requires Zellij WASM host to execute
3. No WASM test runner (wasmtime/wasmer) is installed
4. Native target linking fails due to `zellij_tile` extern host functions

**Workaround for Phase 1:** Tests for pure functions (width computation, state transition validation, format helpers) CAN work if they're in modules that don't import `zellij_tile` APIs. The existing 28 tests in `config.rs` and `render.rs` prove this works — they only test pure functions.

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| QUAL-01 | render_status_bar() split into smaller functions | tmux integration | tmux capture-pane verification | ❌ manual |
| QUAL-02 | Width computation extracted as pure function | unit | `cargo test` (new pure function) | ❌ Wave 0 |
| QUAL-03 | unwrap_or_default() replaced with logged error | tmux + log check | Zellij log inspection | ❌ manual |
| QUAL-04 | serde silent failures logged | tmux + log check | Zellij log inspection | ❌ manual |
| QUAL-05 | Activity state transition validation | unit | `cargo test` (new pure function) | ❌ Wave 0 |
| QUAL-06 | State transitions centralized | code review | N/A | ❌ manual |
| RBST-01 | JSON parse failure logged | tmux + mock event | tmux pipe + log check | ❌ manual |
| RBST-02 | HookPayload required field validation | unit | `cargo test` (validation function) | ❌ Wave 0 |
| RBST-07 | Minimal degraded rendering | tmux | tmux resize + capture | ❌ manual |
| RBST-08 | Narrow terminal (< 50 cols) rendering | tmux | tmux resize + capture | ❌ manual |

### Sampling Rate
- **Per task commit:** `cargo build --target wasm32-wasip1` (must compile)
- **Per wave merge:** tmux integration test (build → deploy → verify status bar)
- **Phase gate:** Full tmux test suite green + Zellij log verification

### Wave 0 Gaps
- [ ] `Activity::can_transition_to()` — pure function testable on WASM target
- [ ] `compute_tab_widths()` — pure function testable on WASM target
- [ ] `HookPayload::validate()` — pure function testable on WASM target

Note: These Wave 0 tests will be written as part of Phase 3 (TEST-01 through TEST-07). Phase 1 focuses on making the code testable, Phase 3 adds the tests.

## Open Questions

1. **WASM Test Runner**
   - What we know: Tests fail on both WASM (no host) and native (link error) targets
   - What's unclear: Whether installing `wasmtime` would allow running unit tests in WASM
   - Recommendation: Don't block on this. Pure function tests work on the WASM target if they avoid `zellij_tile` imports. Complex testing uses tmux integration.

2. **State Transition Strictness**
   - What we know: Different AI tools send different event sequences (Codex: only Stop; OpenCode: skips PostToolUse)
   - What's unclear: Full set of real-world event sequences across all integrations
   - Recommendation: Make validation log-and-allow (warn on unexpected transitions) rather than log-and-reject. Can tighten later with Phase 3 test data.

3. **Degraded Rendering Threshold**
   - What we know: Current threshold is cols < 5 for empty bar
   - What's unclear: What the practical minimum usable width is (depends on separator characters, which are configurable)
   - Recommendation: Use progressive thresholds: < 3 (fill only), < 10 (mode char only), < 30 (session + mode), < 50 (no tabs, just prefix), >= 50 (full render)

## Sources

### Primary (HIGH confidence)
- Source code analysis: `src/render.rs` (718 lines), `src/main.rs` (362 lines), `src/state.rs` (211 lines), `src/event_handler.rs` (81 lines)
- `Cargo.toml` — dependency list confirms only 3 crates
- `.cargo/config.toml` — confirms wasm32-wasip1 default target
- `rust-toolchain.toml` — confirms stable channel with wasm32-wasip1 target

### Secondary (HIGH confidence)
- `.planning/codebase/CONCERNS.md` — identified issues align with code review findings
- `.planning/codebase/ARCHITECTURE.md` — layer descriptions match code structure
- `.planning/codebase/TESTING.md` — test patterns and constraints verified by running `cargo test`

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — code is compact and fully reviewed, no dependency changes needed
- Architecture: HIGH — clear refactoring targets identified from direct code analysis
- Pitfalls: HIGH — WASM build constraint and test limitation verified empirically (cargo test failure captured)
- State machine: MEDIUM — transition matrix needs validation against real-world event sequences from all AI tools

**Research date:** 2025-03-20
**Valid until:** 2025-04-20 (stable — Rust codebase, no fast-moving dependencies)
