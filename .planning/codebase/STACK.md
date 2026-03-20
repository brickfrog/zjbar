# 技术栈

**分析日期**: 2025-01-23

## 编程语言

**主要**
- Rust (Stable channel) - WASM插件核心逻辑，在 `src/*.rs` 中实现
- Bash - 集成脚本和钩子桥接，用于Claude Code、Codex CLI、Gemini CLI的事件转发
- TypeScript - OpenCode插件编写，位于 `opencode-plugin/src/index.ts`

**支持工具**
- jq (JSON query tool) - 所有Bash脚本中必需，用于解析和构建JSON有效负载

## 运行时环境

**WASM运行时**
- Zellij 终端多路复用器 (版本 0.43+) - zjbar 作为Zellij WASM插件运行

**Node.js运行时** (OpenCode插件)
- 无固定版本要求
- `opencode-plugin` 使用 `bun` 作为包管理器

## 框架

**核心框架**
- `zellij-tile` 0.43.1 - Zellij WASM插件开发框架，提供插件trait、事件处理、IPC通信
- Zellij内置渲染系统 - 状态栏通过stdout的ANSI转义码输出到Zellij

**Rust依赖**
- `serde` 1.0.228 - JSON序列化/反序列化 (derive macros)
- `serde_json` 1.0.149 - JSON解析和构建

**Node.js依赖** (OpenCode插件)
- `@opencode-ai/plugin` ^1.0.224 - OpenCode插件API和事件系统
- `zod` (peer dependency) - 类型验证 (间接依赖)

## 关键依赖项

**至关重要**
- `zellij-tile` - 定义了 `ZellijPlugin` trait、`PipeMessage`、`TabInfo` 等核心类型。所有插件逻辑依赖此框架进行事件路由和状态管理。位置: `src/main.rs`
- `serde`/`serde_json` - 处理Claude Code、Codex、Gemini、OpenCode的跨进程JSON有效负载。位置: `src/state.rs`、`src/event_handler.rs`

**基础设施**
- Bash环境 (>=4.0) - 用于所有集成脚本; jq必需
- `terminal-notifier` (macOS) 或 `notify-send` (Linux) - 桌面通知

## 构建配置

**Rust工具链**
- 文件: `rust-toolchain.toml`
- Rust版本: Stable channel
- WASM目标: `wasm32-wasip1` (在 `.cargo/config.toml` 中配置)
- 发布优化: `opt-level = "s"` (最小化二进制大小), LTO启用, 代码剥离启用, panic=abort

**Cargo配置**
- 文件: `.cargo/config.toml`
- 默认构建目标设为 `wasm32-wasip1`

**Node.js构建** (OpenCode插件)
- 文件: `opencode-plugin/package.json`
- 构建命令: `bun build src/index.ts --outdir dist --target node`
- 输出: `opencode-plugin/dist/index.js` (ESM模块)
- TypeScript编译配置: `opencode-plugin/tsconfig.json`

**Make命令**
- 文件: `Makefile`
- `make build` - 编译Rust WASM到 `target/wasm32-wasip1/release/zjbar.wasm` 并安装到 `~/.config/zellij/plugins/`
- `make install` - 安装WASM插件和Zellij布局文件
- `make install-codex-hooks` / `uninstall-codex-hooks` - Codex CLI集成
- `make install-gemini-hooks` / `uninstall-gemini-hooks` - Gemini CLI集成
- `make bump V=x.y.z` - 自动更新所有7个版本文件，构建，提交并打标签
- `make release` - 发布到GitHub和npm

## 版本管理

**版本来源** (由 `make bump` 维护)
1. `Cargo.toml` - Rust包版本
2. `README.md` - WASM下载URL中的版本
3. `README.zh-CN.md` - WASM下载URL中的版本
4. `.claude-plugin/marketplace.json` - Claude Code插件市场两处版本字段
5. `.claude-plugin/plugin.json` - Claude Code插件版本
6. `opencode-plugin/package.json` - npm包版本
7. `Cargo.lock` - 自动更新，保证依赖一致性

## 配置要求

**开发环境**
- Zellij (0.43+) 安装完成
- Rust toolchain (Stable) 与 `wasm32-wasip1` 目标
- Bash 4.0+
- jq CLI工具
- bun (用于构建OpenCode插件)
- cargo-binstall 或手动安装 `cargo-watch` (可选，用于开发)

**部署目标**
- macOS 或 Linux 系统
- Zellij 终端会话活跃
- 对于Claude Code集成: Claude Code 或 CodeBuddy 运行中
- 对于Codex集成: Codex CLI 配置了 zjbar 通知脚本
- 对于Gemini集成: Gemini CLI 配置了 zjbar 钩子
- 对于OpenCode集成: OpenCode 运行中，加载 zjbar-opencode npm包

## 平台要求

**开发**
- macOS (ARM64/Intel) 或 Linux (x86_64/ARM64)
- Git (用于版本控制和发布)
- GitHub CLI (`gh` 命令，用于 `make release`)

**运行时**
- macOS: Terminal.app, iTerm2, 或其他标准终端
- Linux: 支持Wayland/X11的终端 (xdotool可选，用于焦点检测)

---

*栈分析: 2025-01-23*
