---
phase: 02-shell-script-hardening
plan: 02
subsystem: shell-scripts
tags: [bash, atomic-file-operations, race-condition, debounce, mktemp]

# Dependency graph
requires:
  - phase: 02-shell-script-hardening/01
    provides: jq @sh single-call extraction and structured error logging in zjbar-hook.sh
provides:
  - Atomic debounce token write using mktemp + mv (TOCTOU race eliminated)
  - Race-free Stop event deduplication for CodeBuddy
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns: [mktemp + mv atomic rename for file-based token writes]

key-files:
  created: []
  modified:
    - scripts/zjbar-hook.sh

key-decisions:
  - "Used mktemp + mv (atomic rename) instead of noclobber (set -C) — simpler 'last writer wins' semantics"
  - "Added $RANDOM to debounce token for extra entropy beyond PID + epoch seconds"
  - "Simplified subshell token read from [ -f ] && [ cat ] to cat || exit 0"

patterns-established:
  - "Atomic file write: mktemp on same filesystem + mv -f for rename(2) atomicity"

requirements-completed: [RBST-06]

# Metrics
duration: 2min
completed: 2026-03-23
---

# Phase 02 Plan 02: Stop Event Debounce TOCTOU Race Fix Summary

**Atomic debounce token write using mktemp + mv rename(2) replacing non-atomic echo > file, with $RANDOM entropy for CodeBuddy concurrent Stop deduplication**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-23T03:34:25Z
- **Completed:** 2026-03-23T03:36:27Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Replaced TOCTOU-vulnerable `echo "$TOKEN" > file` with `mktemp` + `mv -f` atomic rename pattern
- Added `$RANDOM` to debounce token format (`$$-$(date +%s)-$RANDOM`) for extra entropy
- Simplified subshell token read from `[ -f ] && [ cat ]` to `cat ... || exit 0`
- Preserved all existing behavior: non-Stop cancellation, IS_CODEBUDDY guard, background subshell, notification parameters

## Task Commits

Each task was committed atomically:

1. **Task 1: Replace debounce token write with mktemp + mv** - `0b57eef` (fix)

## Files Created/Modified
- `scripts/zjbar-hook.sh` - Debounce section: mktemp + mv atomic write, $RANDOM in token, simplified cat read

## Decisions Made
- Used `mktemp` + `mv` (recommended simpler approach from research) over `set -C` noclobber — mktemp + mv gives clean "last writer wins" semantics without needing fallback logic
- Added `$RANDOM` as safety belt per research recommendation — two Stop events from same PID in same second are extremely unlikely but $RANDOM makes collision effectively impossible
- Removed `[ -f "$PENDING_NOTIFY_FILE" ] &&` guard before `cat` — the `|| exit 0` after `cat` handles the missing-file case more cleanly and avoids a secondary TOCTOU between the `-f` check and the `cat`

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 02 (Shell Script Hardening) is complete — all 2 plans executed
- Ready for Phase 03: Test Coverage & TS Improvements
- Phase 03 depends on Phase 01 (Rust rendering refactor), which is already complete

## Self-Check: PASSED

All 1 modified file verified on disk. Task commit (0b57eef) verified in git log.

---
*Phase: 02-shell-script-hardening*
*Completed: 2026-03-23*
