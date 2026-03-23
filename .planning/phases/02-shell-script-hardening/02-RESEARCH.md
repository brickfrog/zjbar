# Phase 02: Shell 脚本加固 - Research

**Researched:** 2026-03-23
**Domain:** Shell scripting robustness (bash, jq, atomic file operations, race conditions)
**Confidence:** HIGH

## Summary

Phase 2 targets four shell scripts that serve as bridges between AI coding agents (Claude Code, Codex CLI, Gemini CLI) and the zjbar Zellij plugin via `zellij pipe`. The scripts parse JSON input with `jq`, construct payloads, and forward them. Current issues: `zjbar-hook.sh` invokes `jq` 9 separate times on the same input (lines 38-46), none of the scripts validate required fields before building payloads, `jq` failures are silently swallowed, and the Stop event debounce in `zjbar-hook.sh` has a TOCTOU race condition.

The scope is limited to three "bridge" scripts (`zjbar-hook.sh`, `zjbar-codex-notify.sh`, `zjbar-gemini-hook.sh`) and the shared library `zjbar-lib.sh`. The three "installer" scripts (`install-hooks.sh`, `install-codex-hooks.sh`, `install-gemini-hooks.sh`) are NOT in scope — they use `jq` differently (operating on config files, not parsing hook payloads) and have no debounce/race concerns.

**Primary recommendation:** Consolidate each script's field extraction into a single `jq` call using `@sh` output format with `eval`, add structured error logging on `jq` failure, validate required fields before payload construction, and replace the debounce file token pattern with `noclobber` (`set -C`) atomic file creation.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| RBST-03 | Shell 脚本中将多次 jq 调用合并为单次调用提取所有字段 | See "jq Single-Call Field Extraction" pattern — use `@sh` format with `eval` to safely extract all fields in one invocation |
| RBST-04 | Shell 脚本中为 jq 失败添加 stderr 日志记录 | See "Structured Error Logging" pattern — wrap jq calls in functions that log script name + failure reason to stderr |
| RBST-05 | Shell 脚本中验证必需字段非空后再构建有效负载 | See "Required Field Validation" pattern — check `hook_event` and `pane_id` (from env) before payload construction |
| RBST-06 | 修复 zjbar-hook.sh 中 Stop 事件去抖动的 TOCTOU 竞态条件 | See "Atomic Debounce with noclobber" pattern — replace echo+cat token check with `set -C` atomic file creation |
</phase_requirements>

## Standard Stack

### Core
| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| jq | 1.7+ (user has 1.8.1) | JSON parsing/construction | Already a hard dependency; `@sh` output format available since 1.5 |
| bash | 4.0+ | Shell interpreter | All scripts use `#!/usr/bin/env bash`; `noclobber` support since bash 2.0 |

### Supporting
| Tool | Purpose | When to Use |
|------|---------|-------------|
| `set -C` (noclobber) | Atomic file creation | Debounce token file creation to prevent TOCTOU race |
| `eval` | Safe variable assignment from `jq @sh` output | Single-call jq extraction pattern |
| `mktemp` + `mv` | Atomic file write | Alternative to noclobber if needed |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `set -C` noclobber | `flock` | flock is Linux-only, zjbar supports macOS too |
| `set -C` noclobber | `mkdir` as lock | More complex, directory-based lock is overkill for token file |
| `jq @sh` + `eval` | `jq -r` + `read` with delimiter | `read` collapses empty tab fields (documented in zjbar-hook.sh line 35) |
| `jq @sh` + `eval` | `jq -r` + null-byte delimiter | Works but more complex; `@sh` is cleaner |

## Architecture Patterns

### Pattern 1: jq Single-Call Field Extraction with `@sh`
**What:** Extract all needed fields from JSON input in a single `jq` invocation, using `@sh` to produce shell-safe quoted strings, then `eval` to set variables.
**When to use:** Whenever a script needs multiple fields from the same JSON input.
**Why:** Eliminates N separate jq process spawns. The `@sh` format produces shell-escaped strings that are safe with `eval`, avoiding the delimiter collision issue documented in zjbar-hook.sh (bash `read` collapses consecutive tab delimiters).

**Example:**
```bash
# BEFORE (current zjbar-hook.sh — 9 separate jq calls):
HOOK_EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty') || exit 0
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // ""')
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // ""')
# ... 6 more calls

# AFTER (single jq call with @sh):
_JQ_OUT=$(echo "$INPUT" | jq -r '
  "HOOK_EVENT=" + (.hook_event_name // "" | @sh) + " " +
  "SESSION_ID=" + (.session_id // "" | @sh) + " " +
  "TOOL_NAME=" + (.tool_name // "" | @sh) + " " +
  "CWD=" + (.cwd // "" | @sh) + " " +
  "TRANSCRIPT_PATH=" + (.transcript_path // "" | @sh) + " " +
  "NOTIF_TYPE=" + (.notification_type // "" | @sh) + " " +
  "AGENT_ID=" + (.agent_id // "" | @sh) + " " +
  "NOTIF_MESSAGE=" + (.message // "" | @sh) + " " +
  "NOTIF_TITLE=" + (.title // "" | @sh)
') || {
  echo "zjbar-hook.sh: jq failed to parse input" >&2
  exit 1
}
eval "$_JQ_OUT"
```

**Note on the existing comment (line 33-37 of zjbar-hook.sh):** The code has a comment claiming individual jq calls are intentional to avoid tab-join + IFS read. That was correct for the `join("\t")` approach, but `@sh` + `eval` avoids both problems — no delimiter issues AND single jq invocation.

### Pattern 2: Structured Error Logging
**What:** Every jq failure logs to stderr with script name and context.
**When to use:** All jq invocations (both field extraction and payload construction).

**Example:**
```bash
# Wrap critical jq calls with error context
_JQ_OUT=$(echo "$INPUT" | jq -r '...') || {
  echo "zjbar-hook.sh: failed to parse hook JSON — input may be malformed" >&2
  exit 1
}

# For payload construction
PAYLOAD=$(jq -nc --arg ... '{...}') || {
  echo "zjbar-hook.sh: failed to build payload JSON" >&2
  exit 1
}
```

### Pattern 3: Required Field Validation
**What:** After extraction, verify required fields are non-empty before proceeding.
**When to use:** After jq extraction, before payload construction.

**Example:**
```bash
# Validate required fields
if [ -z "$HOOK_EVENT" ]; then
  echo "zjbar-hook.sh: missing required field: hook_event" >&2
  exit 0  # exit 0 = silent drop (not an error, just incomplete data)
fi

# pane_id comes from environment, already validated at script top
# But double-check it's a number for payload safety
if ! [[ "$ZELLIJ_PANE_ID" =~ ^[0-9]+$ ]]; then
  echo "zjbar-hook.sh: invalid ZELLIJ_PANE_ID: '$ZELLIJ_PANE_ID'" >&2
  exit 0
fi
```

### Pattern 4: Atomic Debounce with noclobber
**What:** Replace the TOCTOU-vulnerable token file pattern with `set -C` (noclobber) for atomic file creation.
**When to use:** Stop event debounce in zjbar-hook.sh (CodeBuddy only).

**Current TOCTOU race (lines 299-308):**
```
Process A: echo "tokenA" > $PENDING_FILE   # writes token
Process B: echo "tokenB" > $PENDING_FILE   # overwrites token (race!)
Process A's subshell: cat $PENDING_FILE → "tokenB" ≠ "tokenA" → suppressed (GOOD)
Process B's subshell: cat $PENDING_FILE → "tokenB" = "tokenB" → fires (GOOD)
# But with timing: both could read "tokenB" simultaneously → double notification
```

**Fixed pattern using noclobber:**
```bash
DEBOUNCE_TOKEN="$$-$(date +%s%N)"
PENDING_NOTIFY_FILE="/tmp/zjbar-pending-notify-${ZELLIJ_PANE_ID}"

# Remove existing pending file (this cancels any previous Stop)
rm -f "$PENDING_NOTIFY_FILE"

# Atomically create the token file with noclobber
# The subshell scope ensures noclobber doesn't affect the parent
if ! (set -C; echo "$DEBOUNCE_TOKEN" > "$PENDING_NOTIFY_FILE") 2>/dev/null; then
  # Another process just created it — we lost the race, which is fine
  # Our rm above already cancelled the previous one
  # Re-create with our token (the other process's subshell will see mismatch)
  echo "$DEBOUNCE_TOKEN" > "$PENDING_NOTIFY_FILE"
fi

(
  sleep "$ZJBAR_STOP_DEBOUNCE"
  # Atomic read-and-delete: read then remove in quick succession
  # The token comparison ensures only the latest Stop fires
  CURRENT_TOKEN=$(cat "$PENDING_NOTIFY_FILE" 2>/dev/null) || exit 0
  if [ "$CURRENT_TOKEN" = "$DEBOUNCE_TOKEN" ]; then
    rm -f "$PENDING_NOTIFY_FILE"
    zjbar_send_notification "$TITLE" "$MESSAGE" "$ICON_DIR" "$ICON_FILE"
  fi
) &
```

**Even simpler approach (recommended):** Since the debounce only needs "last writer wins" semantics, and the only real race was concurrent writes, use `mv` (atomic rename) instead:

```bash
DEBOUNCE_TOKEN="$$-$(date +%s%N)"
PENDING_NOTIFY_FILE="/tmp/zjbar-pending-notify-${ZELLIJ_PANE_ID}"
TEMP_FILE=$(mktemp "/tmp/zjbar-pending-notify-${ZELLIJ_PANE_ID}.XXXXXX")

echo "$DEBOUNCE_TOKEN" > "$TEMP_FILE"
mv -f "$TEMP_FILE" "$PENDING_NOTIFY_FILE"  # atomic rename on same filesystem

(
  sleep "$ZJBAR_STOP_DEBOUNCE"
  CURRENT_TOKEN=$(cat "$PENDING_NOTIFY_FILE" 2>/dev/null) || exit 0
  if [ "$CURRENT_TOKEN" = "$DEBOUNCE_TOKEN" ]; then
    rm -f "$PENDING_NOTIFY_FILE"
    zjbar_send_notification "$TITLE" "$MESSAGE" "$ICON_DIR" "$ICON_FILE"
  fi
) &
```

**Why `mktemp` + `mv`:** Both the write and the rename happen atomically. No process will ever read a half-written token file. The `mv` on the same filesystem (`/tmp` → `/tmp`) is a single `rename(2)` syscall — atomic and instantaneous.

### Anti-Patterns to Avoid
- **`IFS=$'\t' read` with jq `join("\t")`**: Bash `read` collapses consecutive tab delimiters when middle fields are empty. Already documented in zjbar-hook.sh but still used in zjbar-codex-notify.sh and zjbar-gemini-hook.sh.
- **Silent jq failure**: Current `|| exit 0` after jq discards the failure reason. Must log before exiting.
- **`echo "token" > file` for concurrent writes**: Not atomic. Two processes can interleave writes, producing corrupt content. Use `mktemp` + `mv` instead.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Atomic file creation | `if [ ! -f ]; then echo >` | `set -C` + `echo >` (noclobber) | OS-level O_EXCL atomicity |
| Atomic file write | `echo content > file` | `mktemp` + `mv` | rename(2) is atomic on same FS |
| JSON field extraction | Multiple `jq -r` calls | Single `jq` with `@sh` + `eval` | One process spawn vs N |
| File-based locking | PID file check-and-create | `mkdir` or `flock` | Atomic kernel-level operations |

## Common Pitfalls

### Pitfall 1: `eval` Injection via Untrusted Input
**What goes wrong:** Using `eval` on data not properly escaped could execute arbitrary code.
**Why it happens:** `eval` interprets shell metacharacters.
**How to avoid:** `jq`'s `@sh` format produces properly shell-escaped strings (single-quoted). It handles all special characters including `$`, backticks, single quotes, etc. As long as the `@sh` output is used directly with `eval`, it's safe. The JSON input comes from trusted sources (Claude Code, Codex, Gemini).
**Warning signs:** If you ever construct the eval string by concatenation instead of using `@sh`, that's a bug.

### Pitfall 2: `// empty` vs `// ""` in jq
**What goes wrong:** `// empty` causes the entire jq output to be empty if ANY field uses it and that field is null. `// ""` produces an empty string.
**Why it happens:** `empty` is a jq "nothing" value that can suppress output in certain contexts.
**How to avoid:** Use `// ""` for optional fields that should default to empty string. Use `// empty` only when you want to suppress the entire output (e.g., for the single required field `hook_event` where absence means "skip this event").
**Warning signs:** Current code uses `// empty` for `HOOK_EVENT` (correct) and `// ""` for others (correct). When switching to `@sh` single-call pattern, use `// ""` for all fields and check emptiness after extraction.

### Pitfall 3: The `read` Delimiter Trap
**What goes wrong:** `IFS=$'\t' read -r A B C <<< "$TAB_JOINED"` loses empty middle fields.
**Why it happens:** When field B is empty, bash `read` sees two consecutive tabs and treats them as one delimiter, shifting C into B.
**How to avoid:** Don't use `read` with tab/delimiter joining. The `@sh` + `eval` pattern avoids this entirely.
**Warning signs:** Currently present in `zjbar-codex-notify.sh` (line 42) and `zjbar-gemini-hook.sh` (line 48).

### Pitfall 4: Subshell Variable Scope in Debounce
**What goes wrong:** The background subshell `( sleep N; ... ) &` cannot access variables modified after it's spawned.
**Why it happens:** Subshells get a copy of the environment at spawn time.
**How to avoid:** Pass all needed values (TITLE, MESSAGE, paths) before spawning the subshell. Current code already does this correctly — just ensure the refactored version preserves this.
**Warning signs:** If notification title/message are empty when they shouldn't be, check subshell scope.

### Pitfall 5: Gemini CLI stdout Requirement
**What goes wrong:** Gemini CLI hooks MUST output valid JSON to stdout. Any non-JSON output causes Gemini to fail.
**Why it happens:** Gemini CLI parses hook stdout as JSON response.
**How to avoid:** Ensure all error exits in `zjbar-gemini-hook.sh` still echo `'{}'` to stdout. Error messages go to stderr only.
**Warning signs:** Hook stops working after adding new error logging — check if the log went to stdout instead of stderr.

## Code Examples

### Script-by-Script Change Map

#### zjbar-hook.sh (Claude Code / CodeBuddy)
**Fields extracted from stdin JSON (9 fields):**
`hook_event_name`, `session_id`, `tool_name`, `cwd`, `transcript_path`, `notification_type`, `agent_id`, `message`, `title`

**Current:** 9 separate `echo "$INPUT" | jq -r` calls (lines 38-46)
**Target:** 1 `jq` call with `@sh` output + `eval`
**Validation needed:** `HOOK_EVENT` must be non-empty (already checked at line 48, keep it)
**Debounce fix:** Replace lines 299-308 with `mktemp` + `mv` atomic pattern
**Note:** The `extract_summary()` function (lines 159-252) also has multiple jq calls on transcript data. These are on a DIFFERENT input (transcript file, not stdin) and involve `tail | jq` pipeline. These are NOT in scope for RBST-03 (which targets the stdin JSON extraction). However, adding error logging (RBST-04) to these jq calls is in scope.

#### zjbar-codex-notify.sh (Codex CLI)
**Fields extracted from `$1` JSON (4 fields):**
`type`, `thread-id`, `turn-id`, `cwd`, `last-assistant-message`

**Current:** 1 `jq -r` for `type`, then tab-join `jq` + `IFS read` for 3 fields, then 1 more `jq -r` for `last-assistant-message` (total: 3 jq calls)
**Target:** 1 `jq` call with `@sh` output + `eval` for all 5 fields
**Validation needed:** `EVENT_TYPE` must be `agent-turn-complete` (already checked)
**No debounce to fix.**

#### zjbar-gemini-hook.sh (Gemini CLI)
**Fields extracted from stdin JSON (4 fields):**
`hook_event_name`, `session_id`, `cwd`, `tool_name`, `prompt_response`

**Current:** 1 `jq -r` for `hook_event_name`, then tab-join `jq` + `IFS read` for 3 fields, then 1 more `jq -r` for `prompt_response` (total: 3 jq calls)
**Target:** 1 `jq` call with `@sh` output + `eval` for all 5 fields
**Validation needed:** `HOOK_EVENT` must be non-empty (already checked)
**IMPORTANT:** All error exits must still echo `'{}'` to stdout.
**No debounce to fix.**

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Multiple `jq` calls per field | Single `jq` with `@sh`/`@json`/`@csv` | jq 1.5+ (2015) | Reduces process spawns from N to 1 |
| `echo > file` for token writes | `mktemp` + `mv` atomic rename | POSIX standard | Eliminates partial-write races |
| `[ -f ] && cat` TOCTOU pattern | `set -C` noclobber or `mktemp` + `mv` | POSIX standard | Eliminates check-then-act races |
| Tab-joined `read` | `@sh` + `eval` | jq 1.5+ (2015) | No delimiter collision on empty fields |

**Deprecated/outdated:**
- `IFS read` with `join("\t")`: Broken for empty middle fields — replace with `@sh` + `eval`

## Open Questions

1. **Scope of `extract_summary()` jq calls**
   - What we know: `extract_summary()` in zjbar-hook.sh has ~5 jq calls operating on transcript files (not the stdin JSON)
   - What's unclear: Whether RBST-03 ("合并为单次调用提取所有字段") applies to these transcript-parsing jq calls
   - Recommendation: RBST-03 targets the "field extraction from hook input JSON" pattern. The transcript jq calls are a different concern (streaming JSONL parsing where single-call consolidation is impractical). Only apply RBST-04 (error logging) to transcript jq calls. Leave RBST-03 for the main field extraction.

2. **`date +%s%N` portability on macOS**
   - What we know: macOS `date` does not support `%N` (nanoseconds). Current code uses `date +%s` (seconds).
   - What's unclear: Whether second-level granularity is sufficient for debounce token uniqueness.
   - Recommendation: Use `$$-$(date +%s)` (PID + epoch seconds) as currently done. Two Stop events from the same PID in the same second are impossible (the script would need to finish and restart within the same second). Adding `$RANDOM` as extra entropy is a safe belt: `$$-$(date +%s)-$RANDOM`.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Manual shell script testing via tmux + zellij pipe |
| Config file | none — tests run via tmux workflow per AGENTS.md |
| Quick run command | `bash -n scripts/zjbar-hook.sh` (syntax check) |
| Full suite command | tmux-based integration test (send mock events, capture status bar) |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| RBST-03 | Single jq call per script | manual inspection + smoke test | `bash -n scripts/zjbar-hook.sh && echo '{"hook_event_name":"Stop"}' \| bash scripts/zjbar-hook.sh 2>&1` | N/A (code review) |
| RBST-04 | jq failure logs to stderr | unit-style | `echo 'invalid json' \| ZELLIJ_SESSION_NAME=test ZELLIJ_PANE_ID=1 bash scripts/zjbar-hook.sh 2>&1 \| grep -q 'zjbar-hook.sh'` | ❌ Wave 0 |
| RBST-05 | Missing fields → early exit with warning | unit-style | `echo '{}' \| ZELLIJ_SESSION_NAME=test ZELLIJ_PANE_ID=1 bash scripts/zjbar-hook.sh 2>&1 \| grep -q 'missing'` | ❌ Wave 0 |
| RBST-06 | Atomic debounce (no TOCTOU) | code review + concurrent test | Manual: spawn 2 concurrent Stop events, verify single notification | N/A (code review) |

### Sampling Rate
- **Per task commit:** `bash -n scripts/zjbar-hook.sh && bash -n scripts/zjbar-codex-notify.sh && bash -n scripts/zjbar-gemini-hook.sh` (syntax validation)
- **Per wave merge:** Full tmux integration test with mock events per AGENTS.md testing workflow
- **Phase gate:** All scripts pass syntax check + error logging verification

### Wave 0 Gaps
- [ ] Shell script testing harness (simple bash test that feeds mock JSON and checks stderr output)
- [ ] Consider creating `tests/test-shell-scripts.sh` for automated regression

## Sources

### Primary (HIGH confidence)
- Direct code analysis of `scripts/zjbar-hook.sh`, `scripts/zjbar-codex-notify.sh`, `scripts/zjbar-gemini-hook.sh`, `scripts/zjbar-lib.sh`
- Greg's Wiki BashFAQ/045 — authoritative reference on atomic file operations in bash
- jq manual — `@sh` format documentation (builtin since jq 1.5)

### Secondary (MEDIUM confidence)
- LinuxVox — `set -C` (noclobber) atomic file creation patterns
- SQLPey — Shell script locking comparison (mkdir vs flock vs noclobber vs mktemp+ln)

### Tertiary (LOW confidence)
- None — all findings verified against primary sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — jq and bash are the only tools involved, both well-understood
- Architecture: HIGH — patterns verified against authoritative sources (Greg's Wiki, jq manual)
- Pitfalls: HIGH — each pitfall identified from direct code analysis of existing bugs
- Debounce fix: HIGH — `mktemp` + `mv` is POSIX-standard atomic rename, well-documented

**Research date:** 2026-03-23
**Valid until:** 2026-04-23 (stable domain, bash/jq don't change frequently)
