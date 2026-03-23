# Roadmap: zjbar 代码优化

## 概述

对 zjbar 全代码库进行质量优化，分三阶段推进：先重构 Rust 核心代码（渲染拆分、错误处理、状态机、降级渲染），再加固 Shell 脚本健壮性（JSON 解析合并、错误日志、竞态修复），最后补齐测试覆盖（渲染测试、事件处理测试、状态同步测试、OpenCode TS 类型安全）。全程不改变现有功能行为，不引入新依赖。

## Phases

**Phase 编号说明：**
- 整数阶段 (1, 2, 3)：计划内的里程碑工作
- 小数阶段 (2.1, 2.2)：紧急插入（标记 INSERTED）

小数阶段按数字顺序在相邻整数之间执行。

- [ ] **Phase 1: Rust 核心质量与健壮性** - 重构渲染逻辑、改进错误处理、显式化状态机、实现降级渲染
- [ ] **Phase 2: Shell 脚本加固** - 合并 jq 调用、添加错误日志、验证必需字段、修复 TOCTOU 竞态
- [ ] **Phase 3: 测试覆盖与 TS 改进** - 为渲染、事件处理、状态同步添加全面测试，改进 OpenCode 类型安全

## Phase Details

### Phase 1: Rust 核心质量与健壮性
**Goal**: Rust 插件代码具备清晰的函数边界、可靠的错误报告、显式的状态转换验证，以及在异常情况下的优雅降级能力
**Depends on**: Nothing（首阶段）
**Requirements**: QUAL-01, QUAL-02, QUAL-03, QUAL-04, QUAL-05, QUAL-06, RBST-01, RBST-02, RBST-07, RBST-08
**Success Criteria**（完成后必须为真）:
  1. render_status_bar() 已拆分为多个单一职责函数，每个函数可独立调用和测试
  2. render_tabs() 的宽度计算逻辑已提取为独立计算层，与渲染输出解耦
  3. 所有 unwrap_or_default() 和静默失败的 serde 反序列化已替换为带 eprintln! 日志的错误处理，Zellij 日志中可见错误信息
  4. Activity 状态转换通过显式验证函数控制，无效转换被拒绝并记录日志
  5. 在极窄终端（< 50 列）或渲染异常时，状态栏仍显示最小化信息（session pill + mode 指示器）而非空白
**Plans**: 2 plans

Plans:
- [ ] 01-01-PLAN.md — 渲染重构（提取 render_prefix/compute_tab_widths/fill_remaining）与窄终端降级渲染
- [ ] 01-02-PLAN.md — 错误处理改进（eprintln! 日志）、状态机验证（can_transition_to）、HookPayload 校验

### Phase 2: Shell 脚本加固
**Goal**: 所有 Shell 集成脚本在面对畸形输入、jq 失败、并发执行时能可靠运行并提供有用的诊断信息
**Depends on**: Nothing（与 Phase 1 无技术依赖，但建议顺序执行）
**Requirements**: RBST-03, RBST-04, RBST-05, RBST-06
**Success Criteria**（完成后必须为真）:
  1. 每个 Shell 脚本中只调用一次 jq 即可提取所有需要的字段，不再有多次独立 jq 调用
  2. jq 解析失败时 stderr 中有包含脚本名和失败原因的日志消息
  3. 必需字段（hook_event、pane_id）为空时脚本提前退出并记录警告，而非发送不完整的负载
  4. zjbar-hook.sh 中 Stop 事件的去抖动使用原子文件操作，并发 Stop 事件不会导致重复通知
**Plans**: TBD

Plans:
- [ ] 02-01: Shell 脚本 JSON 解析与错误处理改进
- [ ] 02-02: Stop 事件去抖动竞态修复

### Phase 3: 测试覆盖与 TS 改进
**Goal**: 核心业务逻辑（渲染、事件处理、状态同步）有全面的自动化测试保护，OpenCode 插件具备安全的类型转换
**Depends on**: Phase 1（渲染重构完成后才能为拆分后的函数编写测试）
**Requirements**: TEST-01, TEST-02, TEST-03, TEST-04, TEST-05, TEST-06, TEST-07, TEST-08
**Success Criteria**（完成后必须为真）:
  1. cargo test --lib 覆盖 render_status_bar()、render_tabs()、compute_tab_info() 的核心路径，包含不同列宽和标签数量的参数化用例
  2. handle_hook_event() 对所有事件类型（SessionStart、PreToolUse、PostToolUse、Stop 等）有测试覆盖，验证状态转换正确性
  3. Activity 状态转换的有效路径和无效路径均有测试验证
  4. merge_sessions() 和 broadcast_sessions() 的序列化/合并逻辑有测试覆盖
  5. OpenCode 插件的 pane_id 转换有范围验证，无效输入不会产生错误的数值
**Plans**: TBD

Plans:
- [ ] 03-01: 渲染与事件处理测试
- [ ] 03-02: 状态同步测试与 OpenCode TS 改进

## Progress

**执行顺序：**
阶段按数字顺序执行：1 → 2 → 3

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Rust 核心质量与健壮性 | 0/2 | Not started | - |
| 2. Shell 脚本加固 | 0/2 | Not started | - |
| 3. 测试覆盖与 TS 改进 | 0/2 | Not started | - |
