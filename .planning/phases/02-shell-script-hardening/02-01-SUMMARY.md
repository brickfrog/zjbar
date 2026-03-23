---
phase: 02-shell-script-hardening
plan: 01
subsystem: shell-scripts
tags: [jq, bash, shell, json-parsing, error-handling]

# Dependency graph
requires:
  - phase: none
    provides: none
provides:
  - Single-call jq @sh extraction pattern for all three bridge scripts
  - Structured error logging on jq failures with script filename
  - Required field validation before payload construction
affects: [02-shell-script-hardening]

# Tech tracking
tech-stack:
  added: []
  patterns: [jq @sh + eval single-call extraction, structured stderr error logging]

key-files:
  created: []
  modified:
    - scripts/zjbar-hook.sh
    - scripts/zjbar-codex-notify.sh
    - scripts/zjbar-gemini-hook.sh

key-decisions:
  - "Used jq @sh + eval pattern instead of tab-join + IFS read to avoid delimiter collision on empty fields"
  - "Transcript-parsing jq calls in extract_summary() left unchanged — they operate on streaming JSONL where parse errors are expected"

patterns-established:
  - "jq @sh + eval: Single jq invocation with @sh output format for multi-field extraction from JSON"
  - "Structured error logging: script-name-prefixed messages to stderr on jq failure"

requirements-completed: [RBST-03, RBST-04, RBST-05]

# Metrics
duration: 4min
completed: 2026-03-23
---

# Phase 02 Plan 01: jq Consolidation, Error Logging & Field Validation Summary

**Single-call jq @sh extraction replacing 9+3+3 separate jq invocations across all three bridge scripts, with structured stderr error logging and required field validation**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-23T03:25:44Z
- **Completed:** 2026-03-23T03:29:27Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Replaced 9 separate `jq -r` calls in zjbar-hook.sh with a single `jq @sh` + `eval` extraction
- Replaced `IFS=$'\t' read` + `join("\t")` pattern in zjbar-codex-notify.sh and zjbar-gemini-hook.sh with `@sh` + `eval`
- Added structured error logging (script filename + context) for all jq parse failures and payload construction failures
- Added required field validation (hook_event, event_type) with early exit and warning log
- Maintained Gemini CLI requirement: `{}` output to stdout on all error/early-exit paths

## Task Commits

Each task was committed atomically:

1. **Task 1: Consolidate jq calls in zjbar-hook.sh** - `6e3ab96` (feat)
2. **Task 2: Consolidate jq calls in codex and gemini scripts** - `2ce11f7` (feat)

## Files Created/Modified
- `scripts/zjbar-hook.sh` - Single @sh extraction for 9 fields, error logging, field validation
- `scripts/zjbar-codex-notify.sh` - Single @sh extraction for 5 fields, error logging, field validation
- `scripts/zjbar-gemini-hook.sh` - Single @sh extraction for 5 fields, error logging, {} stdout on errors

## Decisions Made
- Used `// ""` (empty string default) for all fields in @sh extraction instead of `// empty`, checking emptiness after eval — avoids jq `empty` suppressing entire output
- Kept `2>/dev/null` on transcript-parsing jq calls in `extract_summary()` — these operate on streaming JSONL where partial-line parse errors are expected and normal
- Exit code 0 on missing required fields (silent drop, not an error — just incomplete data), exit code 1 on malformed JSON (actual error)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Ready for 02-02-PLAN.md: Stop event debounce TOCTOU race fix (mktemp + mv atomic operations)
- All three bridge scripts now have consistent error handling patterns that 02-02 can build on

## Self-Check: PASSED

All 3 modified files verified on disk. Both task commits (6e3ab96, 2ce11f7) verified in git log.

---
*Phase: 02-shell-script-hardening*
*Completed: 2026-03-23*
