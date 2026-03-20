# 代码库结构

**分析日期：** 2025-01-16

## 目录布局

```
zjbar/
├── src/                           # Rust WASM 插件源代码
│   ├── main.rs                    # ZellijPlugin trait 实现，事件路由，全局状态管理
│   ├── config.rs                  # BarConfig 结构体，KDL 解析，颜色管理（Tokyo Night）
│   ├── render.rs                  # 状态栏渲染管道，ANSI 转义码输出，字符宽度计算
│   ├── state.rs                   # 状态类型：Activity、SessionInfo、Settings、HookPayload
│   ├── event_handler.rs           # Hook 事件处理（claude、codex、gemini、opencode）
│   └── tab_pane_map.rs            # Pane ID → (tab_index, tab_name) 映射构建
├── opencode-plugin/               # OpenCode npm 包（TypeScript 源）
│   ├── src/index.ts               # OpenCode plugin hooks → zellij pipe 桥接
│   ├── package.json               # npm 包配置（名称：zjbar-opencode）
│   ├── tsconfig.json              # TypeScript 编译选项
│   ├── assets/opencode-logo.png   # OpenCode 标志（通知用）
│   └── dist/                      # 构建输出（npm publish 用）
├── scripts/                       # 外部工具集成脚本
│   ├── zjbar-hook.sh              # Claude Code hook → zellij pipe 桥接（主脚本）
│   ├── zjbar-lib.sh               # Hook 脚本共享函数库（通知、摘录提取）
│   ├── zjbar-codex-notify.sh      # Codex CLI notify → zellij pipe 桥接
│   ├── install-codex-hooks.sh     # Codex 集成安装脚本
│   ├── zjbar-gemini-hook.sh       # Gemini CLI hook → zellij pipe 桥接
│   ├── install-gemini-hooks.sh    # Gemini 集成安装脚本
│   └── install-hooks.sh           # Claude Code 集成安装脚本（已弃用，使用插件市场）
├── assets/                        # 静态资源（标志和图标）
│   ├── claude-logo.png            # Claude Code 标志（桌面通知用）
│   ├── codebuddy-logo.png         # CodeBuddy 标志
│   ├── codex-logo.png             # Codex CLI 标志
│   ├── gemini-logo.png            # Gemini CLI 标志
│   └── opencode-logo.png          # OpenCode 标志（符号链接至 opencode-plugin/assets/）
├── hooks/                         # Claude Code 插件市场元数据
│   └── hooks.json                 # Claude Code hook 事件定义
├── .claude-plugin/                # Claude Code 插件市场包
│   ├── marketplace.json           # 市场列表元数据
│   └── plugin.json                # 插件清单（版本、权限、配置架构）
├── .codebuddy/                    # CodeBuddy 插件目录（符号链接至 .claude-plugin）
├── .cargo/                        # Cargo 构建配置
│   └── config.toml                # 默认构建目标：wasm32-wasip1
├── .planning/codebase/            # GSD 文档输出目录
│   ├── ARCHITECTURE.md            # 架构和数据流分析
│   ├── STRUCTURE.md               # 此文件
│   ├── CONVENTIONS.md             # 代码风格和命名规范
│   ├── TESTING.md                 # 测试框架和模式
│   ├── STACK.md                   # 技术栈
│   └── INTEGRATIONS.md            # 外部集成
├── layout.kdl                     # 默认 Zellij 布局，带 zjbar 插件配置
├── layout.swap.kdl                # Swap 布局定义（AI、垂直、水平、堆叠）
├── Cargo.toml                     # Rust 包清单（v1.1.40）
├── Cargo.lock                     # 锁定的依赖版本
├── rust-toolchain.toml            # 固定 Rust 工具链（stable + wasm32-wasip1 target）
├── Makefile                       # 构建、测试、版本发布目标
├── install.sh                     # 引导安装脚本（检查先决条件，委托给 make）
├── AGENTS.md                      # 项目开发指南（zjbar 特定）
├── AGENTS.local.md                # 本地覆盖（git 忽略）
├── CLAUDE.md                      # 符号链接至 AGENTS.md（全局 Claude 指令）
├── CODEBUDDY.md                   # 符号链接至 AGENTS.md（CodeBuddy 指令）
├── GEMINI.md                      # 符号链接至 AGENTS.md（Gemini 指令）
├── README.md                      # 英文说明书
├── README.zh-CN.md                # 中文说明书
└── LICENSE                        # 许可证

```

## 目录用途详解

**`src/`：**
- 用途：Rust WASM 插件源代码
- 包含内容：6 个 .rs 源文件，每个处理一个关键职责
- 编译目标：wasm32-wasip1（WASM Components 标准）
- 关键文件：`main.rs` 是入口点（实现 `register_plugin!(State)`）

**`opencode-plugin/`：**
- 用途：OpenCode npm 包源代码
- 包含内容：TypeScript 源、package.json、构建输出
- 构建命令：`cd opencode-plugin && bun run build` → 输出到 `dist/index.js`
- npm 发布：与 WASM 插件一同发布，版本同步
- 安装方式：npm install 或 global cache（`~/.config/opencode/node_modules/` 和 `~/.cache/opencode/node_modules/`）

**`scripts/`：**
- 用途：外部工具集成脚本（hook 桥接）
- Claude Code：`zjbar-hook.sh` 注册为 hook 命令，接收 JSON on stdin，发送 pipe 消息
- Codex CLI：`zjbar-codex-notify.sh` 注册为 notify 脚本，接收 JSON 作为 $1
- Gemini CLI：`zjbar-gemini-hook.sh` 注册为 hook，接收 JSON on stdin
- 共享库：`zjbar-lib.sh` 提供通知、摘录提取、设置加载函数
- 安装：各脚本包含 install-*.sh 来配置 AI 工具的集成点

**`hooks/`：**
- 用途：Claude Code 插件市场 hook 注册
- 内容：`hooks.json` 定义 hook 事件名和命令（由 Claude Code 插件自动安装）
- 自动化：Claude Code 插件扫描此文件并在首次启动时注册 hook

**`.claude-plugin/`：**
- 用途：Claude Code 插件市场包
- 内容：marketplace.json（市场列表）和 plugin.json（插件清单）
- 版本：与 Cargo.toml 版本同步（`make bump V=x.y.z` 自动更新）
- 发布：通过 Claude Code 插件市场 UI 或 API

**`assets/`：**
- 用途：桌面通知用的应用标志
- 内容：PNG 图标（claude、codebuddy、codex、gemini、opencode）
- 使用：Hook 脚本通过 terminal-notifier（macOS）或 notify-send（Linux）在通知中嵌入图标

**`.planning/codebase/`：**
- 用途：GSD（代码库编排）工具生成的文档
- 内容：ARCHITECTURE.md、STRUCTURE.md、CONVENTIONS.md、TESTING.md、STACK.md、INTEGRATIONS.md
- 维护：由 gsd-codebase-mapper 工具自动生成

**`layout.kdl` 和 `layout.swap.kdl`：**
- 用途：Zellij 布局定义
- 使用：`zellij --layout layout.kdl` 启动带 zjbar 的会话
- layout.kdl：default_tab_template 中内嵌 zjbar 插件配置块（KDL 变量）
- layout.swap.kdl：额外的 swap 布局（AI、垂直、水平、堆叠、浮动）

## 关键文件位置

**入口点：**
- `src/main.rs` —— ZellijPlugin trait 实现，事件分发
- `opencode-plugin/src/index.ts` —— OpenCode 事件处理导出
- `layout.kdl` —— 默认使用的 Zellij 布局

**配置：**
- `src/config.rs` —— Tokyo Night 颜色常数和 KDL 解析
- `Cargo.toml` —— 依赖版本（zellij-tile、serde、serde_json）
- `rust-toolchain.toml` —— Rust stable + wasm32-wasip1 target

**核心逻辑：**
- `src/state.rs` —— Activity、SessionInfo、Settings 定义
- `src/event_handler.rs` —— Hook 事件 → Activity 映射
- `src/render.rs` —— ANSI 转义码生成和布局

**测试：**
- `src/config.rs`（第 288-337 行） —— 颜色解析测试
- `src/render.rs`（第 610-717 行） —— 字符宽度、数字计数、活动优先级测试

**外部集成：**
- `scripts/zjbar-hook.sh` —— Claude Code / CodeBuddy hook 桥接
- `scripts/zjbar-codex-notify.sh` —— Codex notify 脚本
- `scripts/zjbar-gemini-hook.sh` —— Gemini hook 脚本
- `hooks/hooks.json` —— Claude Code hook 事件注册

## 命名规范

**文件命名：**
- 源文件：全小写，分隔符用下划线（`tab_pane_map.rs`、`event_handler.rs`）
- 脚本：前缀 `zjbar-`，后跟功能名（`zjbar-hook.sh`、`zjbar-codex-notify.sh`）
- 资源：功能名 + 扩展名（`claude-logo.png`、`opencode-logo.png`）

**目录命名：**
- 源代码：`src/`
- 集成脚本：`scripts/`
- 资源：`assets/`
- 嵌套项目：功能名（`opencode-plugin/`）
- 元数据：`.前缀`（`.claude-plugin/`、`.cargo/`、`.planning/`）

**类型和函数命名：**
- 结构体：PascalCase（`State`、`BarConfig`、`SessionInfo`、`HookPayload`）
- 枚举：PascalCase（`Activity`、`FlashMode`、`ViewMode`）
- 函数：snake_case（`build_pane_to_tab_map()`、`handle_hook_event()`、`render_status_bar()`）
- 常数：SCREAMING_SNAKE_CASE（`DONE_TIMEOUT`、`TIMER_INTERVAL`、`MAX_TAB_NAME_WIDTH`）
- 模块：与文件名同（mod config、mod render）

**变量命名：**
- 本地变量：snake_case（`session_pill_width`、`region_start`、`best_activity`）
- 字段：snake_case（`pane_id`、`tab_index`、`last_event_ts`）
- 缩写：标准（`cfg` for config、`buf` for buffer、`col` for column、`msg` for message）

## 添加新代码的指南

**新特性（功能性改动）：**
- 主要逻辑：`src/main.rs`（event 处理）或 `src/state.rs`（状态定义）
- 事件映射：`src/event_handler.rs` 中添加 hook 事件处理
- 渲染元素：`src/render.rs` 中添加渲染函数（参考 `render_single_tab()` 模式）
- 配置选项：`src/config.rs` 中添加 BarConfig 字段和 color_fields! 宏条目
- 测试：同一文件的 `#[cfg(test)]` 模块

**新组件/模块：**
- 创建 `src/new_module.rs`
- 在 `src/main.rs` 中声明：`mod new_module;`
- 导入到其他模块：`use crate::new_module::*;` 或特定项

**外部工具集成：**
- 脚本：在 `scripts/` 中创建 `zjbar-<tool>-<function>.sh`
- 安装脚本：创建 `scripts/install-<tool>-hooks.sh`，配置 AI 工具以调用脚本
- Hook 事件映射：在 `src/event_handler.rs` 中对应 hook 事件名
- JSON 有效负载：确保脚本输出与 `HookPayload` 结构兼容

**共享工具函数：**
- 位置：`scripts/zjbar-lib.sh`（被所有脚本 source）
- 模式：定义 bash 函数（如 `zjbar_clean_and_truncate()`），在各脚本中调用

**测试：**
- 单元测试：在各模块末尾（`#[cfg(test)] mod tests`）
- 集成测试：tmux 测试（见 AGENTS.md 中的 tmux 工作流）
- 运行：`cargo test --target wasm32-wasip1`

## 特殊目录

**`target/`：**
- 用途：Cargo 构建输出
- 包含：wasm32-wasip1 和 aarch64-apple-darwin 的编译工件
- 生成：自动生成，git 忽略
- 已提交：否

**`.git/`：**
- 用途：Git 版本控制
- 已提交：是（仓库元数据）

**`opencode-plugin/node_modules/` 和 `opencode-plugin/dist/`：**
- 用途：npm 依赖和构建输出
- 生成：`cd opencode-plugin && bun install && bun run build`
- 已提交：node_modules 否，dist/ 是

**`opencode-plugin/dist/`：**
- 用途：编译的 OpenCode 插件
- 生成方式：TypeScript → JavaScript（tsconfig.json 配置）
- 已提交：是（npm publish 用）

---

*结构分析日期：2025-01-16*
