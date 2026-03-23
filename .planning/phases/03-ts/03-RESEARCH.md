# Phase 3: 测试覆盖与 TS 改进 - Research

**Researched:** 2026-03-23
**Domain:** Rust unit testing (cargo test), TypeScript type safety (bun/node)
**Confidence:** HIGH

## Summary

Phase 3 添加全面的单元测试覆盖 Rust 核心业务逻辑（渲染管道、事件处理、状态同步），并改进 OpenCode 插件的 TypeScript 类型安全。当前代码库有 33 个通过的测试，主要覆盖工具函数（char_width、display_width、digit_count、format_elapsed、activity_priority/symbol）和状态转换验证（8 个 can_transition_to 测试 + 2 个 payload 验证测试），但缺少对核心渲染函数、事件处理器和状态同步逻辑的测试。

Phase 1 重构已将 render_status_bar() 拆分为独立的纯函数（render_prefix、compute_tab_widths、fill_remaining、render_degraded、render_single_tab、compute_tab_info），使这些函数可以在单元测试中独立调用。关键技术约束：测试必须在原生目标 (`--target aarch64-apple-darwin`) 运行，因为默认的 wasm32-wasip1 目标无法执行测试二进制。`host_run_plugin_command` 外部函数 stub 已在 Phase 1 中添加，解决了链接问题。

**Primary recommendation:** 在 render.rs、event_handler.rs、state.rs 中添加 co-located 测试，使用现有模式（`#[cfg(test)] mod tests`）；渲染测试验证输出 String 中的关键内容（ANSI 片段、标签名、活动符号），而非精确匹配完整输出；OpenCode 插件添加 parseInt 安全检查（NaN guard）和可选的 bun test 测试。

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
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

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TEST-01 | 为 render_status_bar() 添加单元测试，使用模拟 State 验证输出结构 | render_status_bar 使用 print!() 输出；测试需构造 State + 捕获 buf 内容 — 函数已写入 buf 字符串再 print，可测 buf 生成逻辑 |
| TEST-02 | 为 render_tabs() 添加参数化测试，覆盖不同列宽、标签数量、活动类型 | render_tabs 接受 &mut State + &mut String buf — 可直接调用并检查 buf 内容 |
| TEST-03 | 为 compute_tab_info() 添加测试，验证多 pane 标签页的最佳活动选择 | compute_tab_info 是纯函数，接受 &State + &[&TabInfo] — 直接可测 |
| TEST-04 | 为 handle_hook_event() 添加全面测试，覆盖所有事件类型 | handle_hook_event 接受 &mut State + HookPayload，修改 state.sessions — 直接可测 |
| TEST-05 | 为 Activity 状态转换添加测试，验证有效和无效转换 | 已有 6 个 can_transition_to 测试，需扩展覆盖更多边界 |
| TEST-06 | 为 merge_sessions() 添加测试，验证时间戳比较和状态合并逻辑 | merge_sessions 是 &mut self 方法，可构造 State + incoming BTreeMap 测试 |
| TEST-07 | 为 broadcast_sessions() 序列化逻辑添加测试 | broadcast_sessions 调用 Zellij API (pipe_message_to_plugin)，需提取纯序列化逻辑或跳过 API 调用测试 |
| TEST-08 | 改进 OpenCode 插件类型安全（pane_id 转换验证、输入校验） | parseInt(paneId!, 10) 可能返回 NaN — 需添加 isNaN guard 和 non-null assertion 替换 |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| cargo test (libtest) | Rust stable | Unit test runner | 项目唯一测试框架，无外部依赖 |
| assert_eq!/assert! | std | Assertions | 项目约束：不引入新 crate |
| bun test | bun built-in | TS 纯函数测试（可选） | 项目已使用 bun build，bun test 零额外依赖 |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| State::default() | crate | 测试状态构造 | 每个需要 State 的测试 |
| BarConfig::default() | crate | 默认配置构造 | 渲染测试的配置 |
| TabInfo::default() | zellij_tile | 标签信息构造 | compute_tab_info/render_tabs 测试 |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| 手动构造 State | factory helper fn | 更简洁但增加测试维护面；推荐使用 helper fn |
| assert! 字符串包含 | 精确匹配完整 ANSI 输出 | 精确匹配太脆弱——任何颜色常量或格式变更都会破坏测试 |
| 跳过 broadcast_sessions | cfg(test) stub | 提取序列化逻辑为纯函数更优 |

**Installation:** 无需安装——全部使用现有工具。

**Test command:**
```bash
cargo test --target aarch64-apple-darwin
```

## Architecture Patterns

### Test Organization
```
src/
├── render.rs          # 追加测试到现有 #[cfg(test)] mod tests
├── event_handler.rs   # 新增 #[cfg(test)] mod tests
├── state.rs           # 追加测试到现有 #[cfg(test)] mod tests
└── main.rs            # merge_sessions 测试放在此处（方法在 impl State 内）
opencode-plugin/
├── src/index.ts       # pane_id 类型安全改进
└── src/__tests__/     # 可选：bun test 纯函数测试
```

### Pattern 1: 渲染函数测试 — 内容断言而非精确匹配

**What:** 调用渲染函数获取输出 String，用 `contains()` 检查关键内容（标签名、活动符号、模式文字），而非精确匹配完整 ANSI 输出。

**When to use:** 所有 render.rs 函数测试。

**Why:** ANSI 转义码（`\x1b[38;2;R;G;Bm`）嵌入具体 RGB 值，精确匹配会在任何颜色常量调整时全部失败。内容断言只验证"正确的信息出现了"。

**Example:**
```rust
#[test]
fn render_prefix_contains_session_name() {
    let cfg = BarConfig::default();
    let mut buf = String::new();
    let (col, region) = render_prefix(&mut buf, 120, &cfg, "my-session",
        cfg.mode_normal_bg, cfg.mode_normal_fg, "NORMAL");
    assert!(buf.contains("my-session"));
    assert!(buf.contains("NORMAL"));
    assert!(col > 0);
    assert!(region.is_some());
}
```

### Pattern 2: 事件处理测试 — 构造 Payload + 检查 State 变更

**What:** 构造 HookPayload，调用 handle_hook_event，检查 state.sessions 中的 activity 变更。

**When to use:** event_handler.rs 测试。

**Example:**
```rust
#[test]
fn handle_session_start_creates_session() {
    let mut state = State::default();
    let payload = HookPayload {
        source: Some("claude".into()),
        session_id: Some("test-sid".into()),
        pane_id: 42,
        hook_event: "SessionStart".into(),
        tool_name: None,
        cwd: None,
        zellij_session: None,
        term_program: None,
    };
    handle_hook_event(&mut state, payload);
    let session = state.sessions.get(&42).unwrap();
    assert_eq!(session.activity, Activity::Init);
    assert_eq!(session.pane_id, 42);
}
```

### Pattern 3: 纯计算函数测试 — 直接输入/输出验证

**What:** 对 compute_tab_info、compute_tab_widths 等纯函数直接传入数据结构，验证返回值。

**When to use:** 不依赖 State mutation 的纯计算函数。

### Pattern 4: Helper 函数简化 State 构造

**What:** 使用 helper 函数避免重复的 State/Payload/SessionInfo 构造代码。

**Example:**
```rust
fn make_payload(event: &str, pane_id: u32) -> HookPayload {
    HookPayload {
        source: Some("claude".into()),
        session_id: Some("test".into()),
        pane_id,
        hook_event: event.into(),
        tool_name: None,
        cwd: None,
        zellij_session: None,
        term_program: None,
    }
}

fn make_tool_payload(tool: &str, pane_id: u32) -> HookPayload {
    let mut p = make_payload("PreToolUse", pane_id);
    p.tool_name = Some(tool.into());
    p
}
```

### Anti-Patterns to Avoid

- **精确匹配完整 ANSI 输出**：太脆弱，任何颜色或排版调整都会破坏测试
- **直接测试 render_status_bar 的 stdout 输出**：函数使用 print!() 输出，改为测试 buf 中间产物（render_prefix/render_tabs 等子函数已将内容写入 buf 参数）
- **在测试中依赖 unix_now()**：时间戳依赖会导致 flaky 测试——传入固定的 now_s/now_ms 参数（compute_tab_info 已支持）
- **为 broadcast_sessions/pipe_message_to_plugin 写单元测试**：这些调用 Zellij host API，在原生目标上是 no-op stub——只测试序列化逻辑

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Mock Zellij API | 自己实现 mock 框架 | 跳过 API 调用，只测纯逻辑 | Zellij API 是 WASM host import，mock 成本远超收益 |
| 参数化测试框架 | 自己实现参数化宏 | 手动列举 test cases 或 for 循环 | 项目约束：不引入新 crate |
| ANSI 解析器 | 解析并比较 ANSI 结构 | contains() 检查关键文本 | 渲染输出有大量 ANSI 码，解析成本不合理 |

**Key insight:** 这个项目有"不引入新 crate 依赖"的硬约束。所有测试必须用 std 的 assert_eq!/assert! 和手动构造。保持简单。

## Common Pitfalls

### Pitfall 1: 在默认 WASM 目标运行测试
**What goes wrong:** `cargo test` 默认使用 `.cargo/config.toml` 中的 `wasm32-wasip1` 目标，生成的 `.wasm` 二进制无法直接执行。
**Why it happens:** 项目配置了 `[build] target = "wasm32-wasip1"`。
**How to avoid:** 始终使用 `cargo test --target aarch64-apple-darwin`。
**Warning signs:** `cannot execute binary file` 错误。

### Pitfall 2: render_status_bar 使用 print!() 输出
**What goes wrong:** 直接调用 render_status_bar 后无法捕获输出——它通过 print!() 写入 stdout。
**Why it happens:** WASM 插件通过 stdout 与 Zellij 通信，这是设计如此。
**How to avoid:** 测试子函数（render_prefix、render_tabs、render_single_tab 等）——它们写入 `&mut String` buf 参数。对 render_status_bar 整体流程，验证它不 panic 且 state 副作用正确（click_regions 被填充）。
**Warning signs:** 测试中看不到任何输出断言。

### Pitfall 3: unix_now() 导致 flaky 测试
**What goes wrong:** 使用真实时间戳的测试可能因执行时间差异而间歇性失败。
**Why it happens:** compute_tab_info 使用 now_s/now_ms 参数计算 elapsed；handle_hook_event 使用 unix_now() 更新 last_event_ts。
**How to avoid:** compute_tab_info 已接受 now_s/now_ms 参数——传入固定值。handle_hook_event 后需要手动设置 session.last_event_ts 为已知值再做时间相关断言。
**Warning signs:** 测试在 CI 中偶尔失败。

### Pitfall 4: render_tabs/render_settings_menu 需要 &mut State
**What goes wrong:** 这些函数不仅写入 buf，还修改 state.click_regions 和 state.menu_click_regions。
**Why it happens:** 点击区域注册发生在渲染期间。
**How to avoid:** 测试后同时验证 buf 内容和 state 副作用（click_regions 数量和范围）。

### Pitfall 5: OpenCode parseInt NaN 静默传播
**What goes wrong:** `parseInt(paneId!, 10)` 当 ZELLIJ_PANE_ID 为非数字字符串时返回 NaN，JSON 中 NaN 被序列化为 null，Rust 端 serde 反序列化 pane_id: u32 失败。
**Why it happens:** process.env 值总是 string | undefined，parseInt 不会抛出异常。
**How to avoid:** 添加 `isNaN` 检查，早期退出或使用默认值 0。

## Code Examples

### 示例 1: 测试 compute_tab_info 的活动优先级选择
```rust
#[test]
fn compute_tab_info_selects_highest_priority_activity() {
    let mut state = State::default();
    // 两个 session 在同一 tab，不同 activity
    state.sessions.insert(1, SessionInfo {
        session_id: "s1".into(),
        pane_id: 1,
        activity: Activity::Thinking,
        tab_index: Some(0),
        tab_name: Some("Tab 1".into()),
        last_event_ts: 100,
        cwd: None,
    });
    state.sessions.insert(2, SessionInfo {
        session_id: "s2".into(),
        pane_id: 2,
        activity: Activity::Tool("Bash".into()),
        tab_index: Some(0),
        tab_name: Some("Tab 1".into()),
        last_event_ts: 100,
        cwd: None,
    });
    let tab = TabInfo { position: 0, name: "Tab 1".into(), active: true, ..Default::default() };
    let tabs = vec![&tab];
    let infos = compute_tab_info(&state, &tabs, 200, 200_000);
    // Tool has higher priority than Thinking
    assert_eq!(infos[0].best_activity, Some(Activity::Tool("Bash".into())));
}
```

### 示例 2: 测试 handle_hook_event 全事件覆盖
```rust
#[test]
fn handle_hook_event_session_end_removes_session() {
    let mut state = State::default();
    // 先创建一个 session
    state.sessions.insert(1, SessionInfo {
        session_id: "s1".into(), pane_id: 1,
        activity: Activity::Thinking,
        tab_index: None, tab_name: None,
        last_event_ts: 100, cwd: None,
    });
    let payload = HookPayload {
        source: Some("claude".into()),
        session_id: Some("s1".into()),
        pane_id: 1,
        hook_event: "SessionEnd".into(),
        tool_name: None, cwd: None,
        zellij_session: None, term_program: None,
    };
    handle_hook_event(&mut state, payload);
    assert!(state.sessions.get(&1).is_none());
}
```

### 示例 3: 测试 merge_sessions 时间戳竞争
```rust
#[test]
fn merge_sessions_newer_wins() {
    let mut state = State::default();
    state.sessions.insert(1, SessionInfo {
        session_id: "s1".into(), pane_id: 1,
        activity: Activity::Thinking,
        tab_index: None, tab_name: None,
        last_event_ts: 100, cwd: None,
    });
    let mut incoming = BTreeMap::new();
    incoming.insert(1, SessionInfo {
        session_id: "s1".into(), pane_id: 1,
        activity: Activity::Done,
        tab_index: None, tab_name: None,
        last_event_ts: 200, // newer
        cwd: None,
    });
    state.merge_sessions(incoming);
    assert_eq!(state.sessions.get(&1).unwrap().activity, Activity::Done);
}
```

### 示例 4: OpenCode pane_id 安全改进
```typescript
// Before (unsafe):
pane_id: parseInt(paneId!, 10),

// After (safe):
const numericPaneId = parseInt(paneId ?? "", 10);
if (isNaN(numericPaneId) || numericPaneId < 0) return {};
// ... use numericPaneId throughout
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 只测工具函数 | 测试所有公开/内部函数 | Phase 3 | 核心逻辑有回归保护 |
| render_status_bar 不可测 | 测试子函数 buf 输出 | Phase 1 refactor | 渲染逻辑可单元测试 |
| 无 event_handler 测试 | 全事件类型覆盖 | Phase 3 | 状态转换有测试守护 |

## Open Questions

1. **render_status_bar 整体测试深度**
   - What we know: 它调用 print!() 输出，不能直接捕获 stdout
   - What's unclear: 是否值得用 state 副作用（click_regions）来验证整体流程
   - Recommendation: 用 render_status_bar 做 smoke test（不 panic + click_regions 被填充），详细测试留给子函数

2. **broadcast_sessions 测试策略**
   - What we know: 它调用 pipe_message_to_plugin (Zellij API)，在原生 stub 上是 no-op
   - What's unclear: 是否需要单独测试序列化
   - Recommendation: 序列化逻辑很简单（serde_json::to_string），直接测 merge_sessions 的反向操作即可——验证 sessions 能正确序列化/反序列化（round-trip test）

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust libtest (cargo test) + bun test (optional for TS) |
| Config file | .cargo/config.toml (default target override needed) |
| Quick run command | `cargo test --target aarch64-apple-darwin` |
| Full suite command | `cargo test --target aarch64-apple-darwin && cd opencode-plugin && bun run build` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TEST-01 | render_status_bar 输出结构 | unit | `cargo test --target aarch64-apple-darwin render::tests` | ✅ (追加) |
| TEST-02 | render_tabs 参数化 | unit | `cargo test --target aarch64-apple-darwin render::tests` | ✅ (追加) |
| TEST-03 | compute_tab_info 活动选择 | unit | `cargo test --target aarch64-apple-darwin render::tests` | ✅ (追加) |
| TEST-04 | handle_hook_event 全事件 | unit | `cargo test --target aarch64-apple-darwin event_handler::tests` | ❌ Wave 0 |
| TEST-05 | Activity 状态转换 | unit | `cargo test --target aarch64-apple-darwin state::tests` | ✅ (扩展) |
| TEST-06 | merge_sessions 合并逻辑 | unit | `cargo test --target aarch64-apple-darwin tests` | ❌ Wave 0 |
| TEST-07 | broadcast_sessions 序列化 | unit | `cargo test --target aarch64-apple-darwin tests` | ❌ Wave 0 |
| TEST-08 | OpenCode pane_id 类型安全 | manual/build | `cd opencode-plugin && bun run build` | ✅ (改进) |

### Sampling Rate
- **Per task commit:** `cargo test --target aarch64-apple-darwin`
- **Per wave merge:** `cargo test --target aarch64-apple-darwin && cd opencode-plugin && bun run build`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/event_handler.rs` 中添加 `#[cfg(test)] mod tests` — 覆盖 TEST-04
- [ ] `src/main.rs` 中 merge_sessions 测试 — 覆盖 TEST-06, TEST-07 (通过 round-trip 测试间接覆盖 broadcast 序列化)

## Implementation Decisions (Claude's Discretion Recommendations)

### Event Handler Test Depth → 全面覆盖
**Recommendation:** 为每种事件类型编写独立测试（SessionStart、PreToolUse、PostToolUse、UserPromptSubmit、PermissionRequest、Notification、Stop、SessionEnd），加上边界测试（unknown event 被忽略、missing tool_name 使用空字符串、missing session_id 使用空字符串）。这是中等深度，覆盖所有代码路径。

**Rationale:** handle_hook_event 只有 95 行，8 种事件 + 3 个边界 = 11 个测试函数，工作量合理。

### State Sync Test Approach → 测试纯逻辑 merge_sessions
**Recommendation:** 直接测试 merge_sessions（它是 `&mut self` 方法），不测试 broadcast_sessions（Zellij API 依赖）。用 round-trip test 验证序列化正确性：`serde_json::to_string(&state.sessions)` + `serde_json::from_str` + `merge_sessions`。

**Rationale:** broadcast_sessions 本质是 `serde_json::to_string` + `pipe_message_to_plugin`，序列化用 round-trip 验证，API 调用不可测。

### Test Data Construction → Helper 函数
**Recommendation:** 在 event_handler 和 main.rs 测试中使用 `make_payload()` 和 `make_session()` helper 函数，减少每个测试的样板代码。render.rs 测试继续使用 `TabInfo::default()` + field override（现有模式）。

**Rationale:** HookPayload 有 8 个字段，SessionInfo 有 7 个字段，每个测试都手写会产生大量重复。

### Assertion Granularity → 关键内容检查
**Recommendation:** 用 `buf.contains("session-name")`、`buf.contains("NORMAL")`、`buf.contains(activity_symbol)` 验证渲染输出包含正确内容。对宽度相关逻辑，验证 col 返回值的合理范围。不做精确 ANSI 匹配。

**Rationale:** Tokyo Night 颜色常量（6 字节 RGB）频繁出现在 ANSI 码中，任何颜色调整都会破坏精确匹配。

### WASM API Boundary → 跳过 API 调用测试
**Recommendation:** 不测试包含 `pipe_message_to_plugin`、`set_timeout`、`run_command` 等 Zellij API 调用的函数。这些函数（broadcast_sessions、load_config、save_config、request_sync）在原生 target 上是 no-op stub。只测试纯逻辑函数和可通过 state 副作用验证的函数。

**Rationale:** host_run_plugin_command stub 使链接成功但不提供真实行为，测试 API 调用没有意义。

### OpenCode TS Improvement Scope → 中等深度
**Recommendation:**
1. **必须做**: 修复 pane_id 类型安全 — 将 `parseInt(paneId!, 10)` 替换为带 NaN 检查的安全版本，在插件入口处尽早验证并转为数值
2. **必须做**: 消除所有 `!` non-null assertions（`paneId!`, `zellijSession!`），用早期 null check + 类型窄化替代
3. **可选但推荐**: 为 `cleanAndTruncate`、`capitalize`、`TOOL_MAP` 映射添加 bun test

**Rationale:** `!` assertions 是 TypeScript 中的 type safety 逃生舱，用显式检查替换可以在运行时捕获错误。

## Sources

### Primary (HIGH confidence)
- 直接阅读源代码：src/render.rs, src/event_handler.rs, src/state.rs, src/main.rs, opencode-plugin/src/index.ts
- 实际运行 `cargo test --target aarch64-apple-darwin` — 33 tests pass
- .planning/codebase/TESTING.md — 现有测试模式和命名规范
- .planning/codebase/ARCHITECTURE.md — 数据流和模块职责
- Phase 1 Summaries — 重构后的函数签名和测试 stub

### Secondary (MEDIUM confidence)
- .planning/codebase/CONVENTIONS.md — 代码风格约束

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - 全部使用项目现有工具，无新依赖
- Architecture: HIGH - 直接阅读源码，完全理解函数签名和依赖
- Pitfalls: HIGH - 已实际运行测试验证了 WASM 目标问题和原生目标解法
- OpenCode TS: HIGH - 直接阅读源码，确认了 parseInt NaN 问题

**Research date:** 2026-03-23
**Valid until:** 2026-04-23 (稳定项目，测试模式不会快速变化)
