# Requirements: zjbar 代码优化

**Defined:** 2025-03-20
**Core Value:** 在不改变现有功能行为的前提下，让代码更可靠、更可维护、更易测试

## v1 Requirements

### 代码质量

- [x] **QUAL-01**: 将 render.rs 中的 render_status_bar() 拆分为更小的独立函数，每个函数职责单一
- [x] **QUAL-02**: 将 render_tabs() 的宽度计算逻辑提取为独立的计算层
- [x] **QUAL-03**: 替换 main.rs 中的 unwrap_or_default() 为带 eprintln! 日志的错误处理
- [x] **QUAL-04**: 替换 serde 反序列化静默失败为带警告日志的处理
- [x] **QUAL-05**: 为 Activity 状态转换添加显式验证函数，拒绝无效转换
- [x] **QUAL-06**: 将分散的状态转换逻辑集中到 event_handler.rs 的状态机实现中

### 健壮性

- [x] **RBST-01**: 在 pipe() 中为 JSON 解析失败添加 eprintln! 警告日志，包含失败原因
- [x] **RBST-02**: 验证 HookPayload 中的必需字段（hook_event），对不完整负载记录日志
- [ ] **RBST-03**: Shell 脚本中将多次 jq 调用合并为单次调用提取所有字段
- [ ] **RBST-04**: Shell 脚本中为 jq 失败添加 stderr 日志记录
- [ ] **RBST-05**: Shell 脚本中验证必需字段非空后再构建有效负载
- [ ] **RBST-06**: 修复 zjbar-hook.sh 中 Stop 事件去抖动的 TOCTOU 竞态条件（使用原子文件操作）
- [x] **RBST-07**: 实现最小化降级渲染——当渲染出错时至少显示 session pill + mode 指示器
- [x] **RBST-08**: 为极窄终端（< 50 列）提供有意义的最小渲染而非空白

### 测试覆盖

- [ ] **TEST-01**: 为 render_status_bar() 添加单元测试，使用模拟 State 验证输出结构
- [ ] **TEST-02**: 为 render_tabs() 添加参数化测试，覆盖不同列宽、标签数量、活动类型
- [ ] **TEST-03**: 为 compute_tab_info() 添加测试，验证多 pane 标签页的最佳活动选择
- [ ] **TEST-04**: 为 event_handler::handle_hook_event() 添加全面测试，覆盖所有事件类型
- [ ] **TEST-05**: 为 Activity 状态转换添加测试，验证有效转换和拒绝无效转换
- [ ] **TEST-06**: 为 merge_sessions() 添加测试，验证时间戳比较和状态合并逻辑
- [ ] **TEST-07**: 为 broadcast_sessions() 序列化逻辑添加测试
- [ ] **TEST-08**: 改进 OpenCode 插件的类型安全（pane_id 转换验证、输入校验）

## v2 Requirements

### 可配置性

- **CONF-01**: 将 DONE_TIMEOUT、TIMER_INTERVAL 等硬编码常量提取为 KDL 配置项
- **CONF-02**: 将 MAX_TAB_NAME_WIDTH、MIN_TAB_COLS 等渲染参数提取为可配置项

### 基础设施

- **INFR-01**: 搭建 CI/CD 管道（cargo test + clippy + fmt check）
- **INFR-02**: 添加代码覆盖率报告生成
- **INFR-03**: Shell 脚本的 POSIX 兼容性改造

## Out of Scope

| Feature | Reason |
|---------|--------|
| 新功能开发 | 本次聚焦优化，不添加新功能 |
| 性能优化（宽度缓存、增量同步） | 当前性能可接受，不是优先级 |
| Unicode 宽度库替换 | 需要引入新 crate 依赖，与最小依赖约束冲突 |
| 进程间通信重构（替换文件锁为 Unix socket） | 改动过大，超出优化范围 |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| QUAL-01 | Phase 1 | Complete |
| QUAL-02 | Phase 1 | Complete |
| QUAL-03 | Phase 1 | Complete |
| QUAL-04 | Phase 1 | Complete |
| QUAL-05 | Phase 1 | Complete |
| QUAL-06 | Phase 1 | Complete |
| RBST-01 | Phase 1 | Complete |
| RBST-02 | Phase 1 | Complete |
| RBST-03 | Phase 2 | Pending |
| RBST-04 | Phase 2 | Pending |
| RBST-05 | Phase 2 | Pending |
| RBST-06 | Phase 2 | Pending |
| RBST-07 | Phase 1 | Complete |
| RBST-08 | Phase 1 | Complete |
| TEST-01 | Phase 3 | Pending |
| TEST-02 | Phase 3 | Pending |
| TEST-03 | Phase 3 | Pending |
| TEST-04 | Phase 3 | Pending |
| TEST-05 | Phase 3 | Pending |
| TEST-06 | Phase 3 | Pending |
| TEST-07 | Phase 3 | Pending |
| TEST-08 | Phase 3 | Pending |

**Coverage:**
- v1 requirements: 22 total
- Mapped to phases: 22
- Unmapped: 0

---
*Requirements defined: 2025-03-20*
*Last updated: 2025-03-20 after roadmap creation*
