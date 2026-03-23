---
phase: 3
slug: ts
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-23
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust std test (cargo test) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test --lib --target aarch64-apple-darwin` |
| **Full suite command** | `cargo test --lib --target aarch64-apple-darwin && cd opencode-plugin && bun run build` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib --target aarch64-apple-darwin`
- **After every plan wave:** Run `cargo test --lib --target aarch64-apple-darwin && cd opencode-plugin && bun run build`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirements | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-T1 | 01 | 1 | TEST-01, TEST-02, TEST-03 | unit | `cargo test --target aarch64-apple-darwin render::tests` | ✅ | ⬜ pending |
| 03-01-T2 | 01 | 1 | TEST-01, TEST-02 | unit | `cargo test --target aarch64-apple-darwin render::tests` | ✅ | ⬜ pending |
| 03-02-T1 | 02 | 1 | TEST-04, TEST-05 | unit | `cargo test --target aarch64-apple-darwin event_handler::tests state::tests` | ✅ | ⬜ pending |
| 03-02-T2 | 02 | 1 | TEST-06, TEST-07, TEST-08 | unit + build | `cargo test --target aarch64-apple-darwin tests && cd opencode-plugin && bun run build` | ✅ | ⬜ pending |

**Task-to-requirement mapping:**

- **03-01-T1** (render_prefix, fill_remaining, compute_tab_info, compute_tab_widths tests):
  - TEST-01: render_status_bar smoke covered via compute_tab_info and compute_tab_widths (sub-functions)
  - TEST-02: render_tabs parameterized via compute_tab_info parameterized tests
  - TEST-03: compute_tab_info activity selection tests (priority, elapsed, flash)

- **03-01-T2** (render_single_tab, render_tabs, render_status_bar, render_menu_item, render_settings_menu, render_degraded tests):
  - TEST-01: render_status_bar smoke tests (3 tests: default, with tabs, narrow/degraded)
  - TEST-02: render_tabs tests (5 tests: empty, single, multiple, activity, narrow)

- **03-02-T1** (event handler + state transition tests):
  - TEST-04: handle_hook_event 12 tests — all 8 event types (SessionStart, PreToolUse, PostToolUse, PostToolUseFailure, UserPromptSubmit, PermissionRequest, Notification, Stop, SessionEnd) + 3 edge cases
  - TEST-05: Activity state transition 4 new tests (Prompting, Waiting, AgentDone, invalid paths)

- **03-02-T2** (merge_sessions, serialization, OpenCode TS):
  - TEST-06: merge_sessions 6 tests (new entry, newer-wins, older-loses, equal ts, pane_to_tab, multiple)
  - TEST-07: round-trip serialization 2 tests (standard, all activity types)
  - TEST-08: OpenCode pane_id NaN guard + remove `!` assertions + bun build verification

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*No Wave 0 requirements. All test infrastructure (Rust cargo test) already exists. OpenCode TS changes are production code fixes verified by `bun run build`, not a test suite.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify commands
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] No Wave 0 MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 5s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
