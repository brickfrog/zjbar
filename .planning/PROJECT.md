# zjbar 代码优化

## What This Is

对 zjbar（Zellij WASM 状态栏插件）全代码库的质量优化项目。聚焦代码质量改进、健壮性加固和测试覆盖提升，覆盖 Rust 插件核心、Shell 集成脚本和 OpenCode TypeScript 插件三个部分。

## Core Value

在不改变现有功能行为的前提下，让代码更可靠、更可维护、更易测试。

## Requirements

### Validated

- ✓ Tokyo Night 主题 Powerline 状态栏渲染 — existing
- ✓ Claude Code / CodeBuddy 钩子集成 — existing
- ✓ Codex CLI 通知集成 — existing
- ✓ Gemini CLI 钩子集成 — existing
- ✓ OpenCode 插件集成 — existing
- ✓ 多标签页状态同步 — existing
- ✓ AI 活动状态显示（Thinking/Tool/Waiting/Done 等）— existing
- ✓ 桌面通知支持 — existing
- ✓ KDL 配置解析和颜色自定义 — existing
- ✓ 鼠标点击交互（标签切换、设置菜单）— existing
- ✓ CJK 字符宽度感知渲染 — existing

### Active

- [x] 渲染逻辑重构：拆分 render.rs 中的大函数为更小的可测试单元 — Validated in Phase 1
- [x] 错误处理改进：替换静默失败的 unwrap_or_default() 为带日志的错误处理 — Validated in Phase 1
- [x] 状态机显式化：为 Activity 状态转换添加验证逻辑 — Validated in Phase 1
- [x] IPC 消息验证：为 pipe() 中的 JSON 解析失败添加警告日志 — Validated in Phase 1
- [x] 故障降级渲染：确保渲染失败时仍显示最小化状态栏 — Validated in Phase 1
- [x] Shell 脚本健壮性：改进 JSON 解析、添加错误日志、合并 jq 调用 — Validated in Phase 2
- [x] 去抖动竞态修复：修复 zjbar-hook.sh 中 Stop 事件的 TOCTOU 问题 — Validated in Phase 2
- [x] 单元测试：为 render_status_bar()、render_tabs() 添加单元测试 — Validated in Phase 3
- [x] 事件处理测试：为 event_handler::handle_hook_event() 添加全面测试 — Validated in Phase 3
- [x] 状态同步测试：为多实例同步逻辑添加测试 — Validated in Phase 3
- [x] OpenCode 插件类型安全：改进 pane_id 转换和输入验证 — Validated in Phase 3

### Out of Scope

- 新功能开发 — 本次聚焦优化，不添加新功能
- 配置项扩展 — 不将硬编码值提取为 KDL 配置（用户未选择此方向）
- CI/CD 搭建 — 可作为后续项目
- 性能优化（缓存宽度计算等）— 当前性能可接受，不是优先级

## Context

- zjbar 是一个成熟的 Zellij WASM 插件，Phase 3 完成后有 92 个单元测试
- 核心渲染逻辑（render_status_bar、render_tabs）已有完整测试覆盖
- 事件处理器和状态同步逻辑已有全面测试
- Shell 脚本使用多次 jq 调用解析 JSON，效率低且错误处理不完善
- 代码中存在多处静默失败（unwrap_or_default、serde 反序列化忽略错误）
- Stop 事件去抖动存在 TOCTOU 竞态条件
- 已有代码库映射文档在 .planning/codebase/ 目录

## Constraints

- **WASM 限制**: wasm32-wasip1 目标不支持所有 std 功能，测试需在原生目标运行
- **无 panic**: 插件代码不能 panic，所有错误必须优雅处理
- **向后兼容**: 所有优化必须保持现有 IPC 协议和配置格式不变
- **最小依赖**: 避免引入新的 crate 依赖（保持二进制体积小）

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| 全代码库优化（Rust + Shell + TS） | 三个部分都有改进空间，统一处理 | — Pending |
| 聚焦质量/健壮性/测试三方向 | 用户选择，跳过可配置性方向 | — Pending |
| 不引入新 crate 依赖 | 保持 WASM 二进制体积最小化 | — Pending |

---
*Last updated: 2026-03-23 after Phase 3 completion — all phases complete*
