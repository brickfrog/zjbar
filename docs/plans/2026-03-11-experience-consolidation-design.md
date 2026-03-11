# Experience Consolidation Design

**Date:** 2026-03-11

## Problem

Debugging the OpenCode integration revealed several workflow gaps:

1. tmux test sessions lacked standardized naming, leading to leftover sessions and inconsistent workflows
2. AI assistants would ask users to manually observe status bar output instead of self-verifying
3. No documented debugging procedures for Claude Code or OpenCode integrations
4. Stale plugin caches caused hard-to-diagnose bugs (PostToolUse from old cached code)

## Changes

### AGENTS.md — Testing with tmux (updated)

- Fixed session name: `zjbar_test` for tmux, `zjbar_test` for Zellij (`-s zjbar_test`)
- Pre-cleanup: always kill existing session before starting
- Post-cleanup: always kill session after testing
- Auto-test rule: AI assistants must verify via tmux before delivering, no user confirmation needed
- Added AI integration event testing example (mock `zellij pipe` events)

### AGENTS.md — Debugging AI Integration (new section)

- WASM side: `eprintln!()` → Zellij log file, how to tail logs
- Claude Code: hook registration, script flow, manual test command
- OpenCode: source/build/cache locations, quick cache update command, env quirks, execution model difference, PostToolUse gotcha

### MEMORY.md — Updated

- Full OpenCode cache paths (3 locations)
- PostToolUse root cause and solution
- OpenCode in-process execution model vs Claude Code's separate-process model
- tmux testing conventions
