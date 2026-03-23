# Phase 3: 测试覆盖与 TS 改进 - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Add comprehensive automated test coverage for core Rust business logic (rendering pipeline, event handling, state synchronization) and improve OpenCode TypeScript plugin type safety. No new features — testing and hardening only.

Requirements: TEST-01 through TEST-08 from REQUIREMENTS.md.

</domain>

<decisions>
## Implementation Decisions

### Rust test scope
- **Full coverage of render.rs** — all 15 functions get tests, not just utilities
- Key targets: `render_status_bar`, `render_tabs`, `compute_tab_info`, `render_prefix`, `fill_remaining`, `render_degraded`, `render_single_tab`, `compute_tab_widths`, `render_menu_item`, `render_settings_menu`
- Existing 18 tests for utility functions (char_width, display_width, etc.) remain untouched

### Claude's Discretion
- **Event handler test depth**: Claude decides coverage depth for `handle_hook_event` — per-event-type tests, edge cases (unknown events, missing fields, state transition conflicts), or a mix
- **State sync test approach**: Claude decides how to handle `merge_sessions`/`broadcast_sessions` testing — test pure logic only, extract testable functions, or use cfg(test) stubs
- **Test data construction**: Claude decides between factory functions (e.g., `make_test_state()`) vs inline `State::default()` + field overrides — whichever produces cleaner, more maintainable tests
- **Assertion granularity**: Claude decides whether to verify exact ANSI output strings or check for key content presence (session name, tab name, activity symbol) — balance strictness vs maintenance cost
- **WASM API boundary**: Claude decides how to handle functions calling Zellij API (pipe, set_timeout, subscribe) in unit tests — skip them, use cfg(test) no-op stubs, or extract pure logic
- **OpenCode TS improvement scope**: Claude decides depth — minimum: fix pane_id type safety (remove `!` assertions, add runtime checks). Optionally: add bun test suite for pure functions, add hook payload validation

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project instructions
- `AGENTS.md` — zjbar-specific development guidelines, build commands, test commands, tmux workflow

### Testing patterns
- `.planning/codebase/TESTING.md` — Existing test framework, patterns, naming conventions, coverage status
- `.planning/codebase/CONVENTIONS.md` — Code style and naming conventions

### Architecture
- `.planning/codebase/ARCHITECTURE.md` — Data flow, module responsibilities, Zellij API usage
- `.planning/codebase/STRUCTURE.md` — File layout, key file locations

### Requirements
- `.planning/REQUIREMENTS.md` — TEST-01 through TEST-08 acceptance criteria

### Phase 1 artifacts (test targets created by refactoring)
- `.planning/phases/01-rust-core-quality/01-01-SUMMARY.md` — Render refactoring details (new function signatures)
- `.planning/phases/01-rust-core-quality/01-02-SUMMARY.md` — Error handling and state machine changes

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `State::default()` — already implemented, provides baseline test state
- `BarConfig::default()` — provides Tokyo Night default colors for render tests
- Existing 18 render.rs tests — demonstrate the co-located test pattern to follow
- Existing 8 state.rs tests — demonstrate type construction patterns
- Existing 7 config.rs tests — demonstrate config mock patterns with BTreeMap

### Established Patterns
- Co-located tests: `#[cfg(test)] mod tests { use super::*; }` at file end
- Naming: `#[test] fn test_<function>_<scenario>()`
- Grouping: `// -- category --` comments between test groups
- No external test dependencies (no proptest, no mockall) — standard assert_eq!/assert! only
- No new crate dependencies allowed (PROJECT.md constraint)

### Integration Points
- render.rs functions take `&mut State` + dimensions → return nothing (write to internal buffer via print!())
- event_handler takes `&mut State` + `HookPayload` → mutates state
- merge_sessions takes `&mut self` + `BTreeMap<u32, SessionInfo>` → mutates self.sessions
- OpenCode plugin: standalone npm package, bun build, no Rust integration needed for testing

### Testing Constraints
- Tests run on native target (`cargo test --lib`), NOT wasm32-wasip1
- Functions using Zellij API (pipe(), set_timeout(), subscribe()) won't link in native tests
- render functions use `print!()` macro — output capture needed or refactor to return String

</code_context>

<specifics>
## Specific Ideas

No specific requirements — user delegated most implementation decisions to Claude. Key constraint: maintain existing test patterns and don't introduce new crate dependencies.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 03-ts*
*Context gathered: 2026-03-23*
