---
phase: 02
slug: shell-script-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-23
---

# Phase 02 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | bash manual verification + shellcheck |
| **Config file** | none — shell scripts tested via integration |
| **Quick run command** | `shellcheck scripts/zjbar-hook.sh scripts/zjbar-codex-notify.sh scripts/zjbar-gemini-hook.sh` |
| **Full suite command** | `shellcheck scripts/*.sh && bash -n scripts/*.sh` |
| **Estimated runtime** | ~2 seconds |

---

## Sampling Rate

- **After every task commit:** Run `shellcheck` on modified scripts
- **After every plan wave:** Run full shellcheck + syntax check
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 2 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | RBST-03 | integration | `grep -c 'jq ' scripts/zjbar-hook.sh` (expect 1-2) | N/A | pending |
| 02-01-02 | 01 | 1 | RBST-04 | integration | `grep -c 'stderr\|>&2' scripts/zjbar-hook.sh` (expect >0) | N/A | pending |
| 02-01-03 | 01 | 1 | RBST-05 | integration | `grep 'exit\|return' scripts/zjbar-hook.sh` (verify early-exit on empty fields) | N/A | pending |
| 02-02-01 | 02 | 1 | RBST-06 | integration | `grep 'mv\|mktemp' scripts/zjbar-hook.sh` (verify atomic ops) | N/A | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements — shellcheck + bash -n available.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Concurrent Stop events don't duplicate notifications | RBST-06 | Race condition timing requires real concurrent execution | Run two `zjbar-hook.sh Stop` in parallel, verify only one notification |
| Gemini hook outputs '{}' on all error paths | RBST-04 | Requires simulating jq failure with piped input | Pipe malformed JSON to zjbar-gemini-hook.sh, verify stdout is '{}' |

---

## Validation Sign-Off

- [ ] All tasks have automated verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 2s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
