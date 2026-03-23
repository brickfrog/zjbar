---
phase: 3
slug: ts
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-23
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust std test (cargo test) + bun test |
| **Config file** | Cargo.toml (Rust), opencode-plugin/package.json (TS) |
| **Quick run command** | `cargo test --lib --target aarch64-apple-darwin` |
| **Full suite command** | `cargo test --lib --target aarch64-apple-darwin && cd opencode-plugin && bun test` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib --target aarch64-apple-darwin`
- **After every plan wave:** Run `cargo test --lib --target aarch64-apple-darwin && cd opencode-plugin && bun test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | TEST-01 | unit | `cargo test --lib render --target aarch64-apple-darwin` | ✅ | ⬜ pending |
| 03-01-02 | 01 | 1 | TEST-02 | unit | `cargo test --lib render --target aarch64-apple-darwin` | ✅ | ⬜ pending |
| 03-01-03 | 01 | 1 | TEST-03 | unit | `cargo test --lib event --target aarch64-apple-darwin` | ✅ | ⬜ pending |
| 03-01-04 | 01 | 1 | TEST-04 | unit | `cargo test --lib state --target aarch64-apple-darwin` | ✅ | ⬜ pending |
| 03-02-01 | 02 | 1 | TEST-05 | unit | `cargo test --lib state --target aarch64-apple-darwin` | ✅ | ⬜ pending |
| 03-02-02 | 02 | 1 | TEST-06 | unit | `cargo test --lib state --target aarch64-apple-darwin` | ✅ | ⬜ pending |
| 03-02-03 | 02 | 1 | TEST-07 | unit | `cd opencode-plugin && bun test` | ❌ W0 | ⬜ pending |
| 03-02-04 | 02 | 1 | TEST-08 | unit | `cd opencode-plugin && bun test` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `opencode-plugin/__tests__/index.test.ts` — test stubs for pane_id validation (TEST-07, TEST-08)
- [ ] bun test configured in `opencode-plugin/package.json`

*Rust test infrastructure already exists and covers all Rust-side requirements.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
