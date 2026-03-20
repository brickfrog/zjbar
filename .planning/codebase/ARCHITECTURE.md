# 架构

**分析日期：** 2025-01-16

## 模式概览

**总体设计：** 事件驱动的 WASM 插件架构

**关键特性：**
- 单一 ZellijPlugin trait 实现，处理所有事件和渲染
- 跨插件实例的分布式状态同步（通过 pipe 消息）
- 外部工具集成通过统一的 JSON 有效负载（claude、codex、gemini、opencode）
- 纯渲染管道（ANSI 转义码输出，无运行时状态文件）

## 分层设计

**事件层：**
- 位置：`src/main.rs`（ZellijPlugin trait impl）
- 职责：捕获 TabUpdate、PaneUpdate、ModeUpdate、Mouse、Timer、PermissionRequestResult、RunCommandResult 等事件
- 依赖项：zellij_tile 事件系统，state 模块
- 使用者：所有其他层都通过 State 变更响应事件

**状态与活动管理层：**
- 位置：`src/state.rs`、`src/event_handler.rs`
- 职责：
  - `State`：全局插件状态容器（sessions、tabs、pane_to_tab 映射、click regions、settings）
  - `Activity` 枚举：表示每个 pane 的工作流状态（Init、Thinking、Tool()、Prompting、Waiting、Notification、Done、AgentDone、Idle）
  - `SessionInfo`：单个 pane 的会话信息（pane_id、tab_index、activity、timestamps）
  - `HookPayload`：来自外部工具的统一事件格式（source、pane_id、hook_event、tool_name、cwd 等）
  - `event_handler::handle_hook_event()`：将外部 hook 事件映射到 Activity 状态
- 依赖项：serde（序列化）、SystemTime（时间戳）
- 使用者：render、main 的事件处理

**配置层：**
- 位置：`src/config.rs`
- 职责：
  - KDL 配置解析（从 layout.kdl 插件块提取）
  - 颜色管理（Tokyo Night 调色板 + 用户覆盖）
  - 图标字符和分隔符定义
  - 颜色查询函数（`mode_style()`、`activity_color()`）
- 依赖项：BTreeMap（配置映射）
- 使用者：render 模块

**标签/窗格映射层：**
- 位置：`src/tab_pane_map.rs`
- 职责：从 Zellij 的 PaneManifest（按 tab_index 索引的 pane 列表）构建 pane_id → (tab_index, tab_name) 的双向映射
- 依赖项：TabInfo、PaneManifest（来自 Zellij）
- 使用者：main（rebuild_pane_map）、event_handler（会话标签名查找）

**渲染管道层：**
- 位置：`src/render.rs`
- 职责：
  - `render_status_bar()`：主入口，编排整体布局
  - `render_tabs()`：迭代并渲染所有可见标签页
  - `render_single_tab()`：单个标签页段（index + name + indicators + powerline arrows）
  - `render_settings_menu()`：当前UI模式为Settings时，渲染菜单项和交互区域
  - `compute_tab_info()`：为每个标签页评估最佳活动、闪烁状态、等待 pane_id
  - 字符宽度计算（CJK 感知：英文=1，中文=2）
  - 宽度约束和截断逻辑（标签名最多 20 字符，ANSI 逃逸码精确跟踪）
- 依赖项：render 模块使用所有其他模块的类型
- 使用者：main 的 render() 方法

## 数据流

**初始化流程：**

1. 用户启动 Zellij：`zellij --layout layout.kdl`
2. Zellij 实例化 WASM 插件，调用 `load(configuration: BTreeMap<String, String>)`
3. 插件解析 KDL 配置 → `BarConfig::from_kdl()`
4. 订阅事件：TabUpdate、PaneUpdate、ModeUpdate、Mouse、Timer、PermissionRequestResult、RunCommandResult
5. 初始定时器设置：`set_timeout(TIMER_INTERVAL)` → 每秒打破一次以清理过期会话

**事件处理流程：**

1. Zellij 事件 → `update(event: Event)` 方法
2. 路由到特定处理器：
   - `TabUpdate(tabs)` → 更新活动标签索引、重建 pane_to_tab 映射
   - `PaneUpdate(manifest)` → 更新 pane_to_tab 映射、刷新会话标签名
   - `ModeUpdate(mode_info)` → 记录输入模式（Normal、Locked、Pane、Tab 等）
   - `Mouse(LeftClick)` → 检查点击区域（标签页、前缀、菜单项）并执行操作
   - `Timer` → 清理过期会话（30 秒后将 Done 转换为 Idle）、处理闪烁等
3. 状态变化 → 返回 true 触发重新渲染

**Hook 事件流程（Claude Code / CodeBuddy / Gemini）：**

1. 外部工具（Claude Code hook、zjbar-hook.sh）发送 JSON 有效负载
2. `zellij pipe --name zjbar -- '<JSON>'` → 调用 `pipe(pipe_message)` 方法
3. 解析有效负载 → `HookPayload` 结构体
4. `event_handler::handle_hook_event(state, payload)` 映射事件：
   - `SessionStart` → Activity::Init
   - `PreToolUse` → Activity::Tool(tool_name)
   - `PostToolUse` / `PostToolUseFailure` → Activity::Thinking
   - `UserPromptSubmit` → Activity::Thinking
   - `PermissionRequest` → Activity::Waiting（可能闪烁）
   - `Notification` → Activity::Notification
   - `Stop` → Activity::Done（30 秒后转为 Idle）
   - `SessionEnd` → 移除会话
5. 对于 Waiting/Notification：根据 FlashMode 设置闪烁截止时间
6. 广播会话同步 → 所有插件实例收到 `zjbar:sync` 消息，合并状态

**OpenCode 事件流程：**

1. OpenCode 加载 zjbar npm 包（`opencode-plugin/src/index.ts`）
2. 监听事件：session.created、session.status、session.idle、session.deleted、permission.asked、message.updated
3. 映射到 hook 事件：
   - session.status: busy → UserPromptSubmit
   - tool.execute.before → PreToolUse
   - session.idle → Stop（并获取会话摘要）
   - permission.asked → PermissionRequest
4. 生成 JSON 有效负载并 spawn detached zellij process 发送 pipe 消息（避免阻塞）
5. 根据 notify_events 设置发送桌面通知

**状态同步流程（多实例）：**

1. 单个 Zellij 会话中的多个标签页各有自己的插件实例
2. 任何实例接收 hook 事件后调用 `broadcast_sessions()`
3. 发送 `zjbar:sync` 消息：所有实例都收到并合并 SessionInfo（时间戳较新的获胜）
4. 多实例通过 `pane_to_tab` 映射保持标签页和窗格关联的一致性

## 关键抽象

**Activity 枚举：**
- 目的：表示单个 pane 中 AI 工作流的阶段
- 示例：见 `src/state.rs`，行 24-34
- 模式：每个 hook 事件映射到一个 Activity 状态；优先级排序用于多 pane 标签页选择最佳图标

**SessionInfo 结构体：**
- 目的：封装单个 pane 的运行时会话信息
- 字段：pane_id、session_id、activity、tab_index、tab_name、last_event_ts、cwd
- 序列化：通过 serde 用于状态同步消息

**HookPayload 结构体：**
- 目的：统一外部工具的事件格式
- 来源：claude（Claude Code）、codex（Codex CLI）、gemini（Gemini CLI）、opencode（OpenCode）
- 字段：source、pane_id、session_id、hook_event、tool_name、cwd、zellij_session、term_program

**BarConfig 结构体：**
- 目的：存储所有视觉和行为配置参数
- 颜色字段：Tokyo Night 调色板常数 + 用户 KDL 覆盖
- 模式：宏 `color_fields!` 简化字段初始化

**ClickRegion 和 MenuClickRegion：**
- 目的：将屏幕列范围映射到可交互元素
- 位置：`src/state.rs`，行 182-188 和 174-178
- 更新：render 填充这些区间，update() 的 Mouse 事件处理器检查点击

## 入口点

**主插件入口：**
- 位置：`src/main.rs`
- 触发条件：Zellij 启动并加载 `.wasm` 文件
- 职责：实现 `ZellijPlugin` trait，实例化并维护 `State`

**渲染入口：**
- 位置：`src/render.rs::render_status_bar()`
- 触发条件：每个 update() 返回 true 或每个计时器滴答
- 职责：生成 ANSI 转义码并打印到 stdout

**Pipe 消息入口（外部集成）：**
- 位置：`src/main.rs::pipe()`，行 167-217
- 触发条件：`zellij pipe --name zjbar -- '<JSON>'` 或同步消息（zjbar:sync、zjbar:settings、zjbar:request）
- 职责：解析并路由消息到事件处理器或状态合并

## 错误处理

**策略：** 宽泛容许（fail-safe），无 panic

**模式：**
- serde 反序列化失败 → 返回 false（忽略消息）
- 文件 I/O 失败（配置加载） → 使用默认值
- jq/bash 工具链缺失（hook 脚本） → exit 1 不影响状态栏
- 无效颜色值 → 回退到默认 Tokyo Night 常数

## 跨切面关注点

**日志记录：** 不使用（WASM 插件无日志系统）
- 调试：使用 `eprintln!()` 输出到 Zellij 日志（`/tmp/zellij-<uid>/zellij-log/zellij.log`）
- Hook 脚本调试：设置 `ZJBAR_DEBUG=1` 以获得 `/tmp/zjbar-debug-<pane_id>.log`

**验证：**
- HookPayload：在 pipe() 中进行 serde 验证
- ClickRegion：鼠标事件中进行边界检查
- 颜色值：在 config.rs 中进行十六进制解析验证（测试覆盖）

**身份验证：** 无（Zellij 插件沙箱提供隔离）

**状态持久化：**
- Settings（flash、elapsed_time、notifications）存储在 `~/.config/zellij/plugins/zjbar.json`
- 通过 `run_command()` 加载/保存（异步 shell 执行）
- 多实例通过 `zjbar:settings` pipe 消息同步设置

---

*架构分析日期：2025-01-16*
