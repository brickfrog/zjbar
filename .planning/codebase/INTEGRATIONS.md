# 外部集成

**分析日期**: 2025-01-23

## API与外部服务

**AI编码工具集成**
- Claude Code (Anthropic) - 通过钩子系统发送AI活动事件
- CodeBuddy (基于Claude Code) - 共享钩子系统，增加Stop事件去抖
- Codex CLI - 通过notify回调发送agent-turn-complete事件
- Gemini CLI (Google) - 通过钩子系统发送AI活动事件，工具名称映射
- OpenCode (不同提供商) - 通过npm插件发送事件

**Zellij IPC通信**
- 方式: Zellij的 `pipe` 机制
- 入口点: 所有集成通过 `zellij pipe --name "zjbar" -- <JSON>` 发送消息到WASM插件
- 插件端处理: `src/main.rs` 中的 `pipe()` 方法接收 `PipeMessage` 并解析JSON有效负载
- 同步通道: `zjbar:sync`, `zjbar:request` (用于多实例状态同步)

## 数据存储

**数据库**
- 不使用数据库 - zjbar 是无状态的实时状态栏渲染插件

**文件存储**
- 本地文件系统 - 仅用于配置和调试日志
- 配置文件: `~/.config/zellij/plugins/zjbar.json` (可选，控制通知行为)
- 调试日志 (可选): `/tmp/zjbar-debug-<pane_id>.log` (当 `ZJBAR_DEBUG=1` 时)

**缓存** (OpenCode插件)
- OpenCode插件缓存位置:
  - `~/.config/opencode/node_modules/zjbar-opencode/dist/index.js` (项目级缓存)
  - `~/.cache/opencode/node_modules/zjbar-opencode/dist/index.js` (全局缓存)
- 注意: OpenCode可能从多处加载插件，更新后需手动更新所有缓存

## 认证与身份

**认证提供者**
- 无云认证 - zjbar 不进行身份验证或授权

**关键认证集成** (由上游工具处理)
- Claude Code - 使用自身认证 (Anthropic API密钥)
- CodeBuddy - 使用自身认证 (同Claude Code)
- Codex CLI - 使用自身认证
- Gemini CLI - 使用自身认证 (Google账号)
- OpenCode - 使用自身认证

**环境变量** (传递给钩子，非密钥)
- `ZELLIJ_SESSION_NAME` - Zellij会话名称 (必需，用于 `zellij pipe` 命令)
- `ZELLIJ_PANE_ID` - 当前窗格ID (必需)
- `ZELLIJ` - Zellij信号 (在Claude Code中可能设为0，需显式使用 `-s` 标志)
- `TERM_PROGRAM` - 终端应用名称 (用于焦点检测和通知)
- `CODEBUDDY_PROJECT_DIR` - CodeBuddy特定 (检测是否运行在CodeBuddy下)
- `CLAUDE_PLUGIN_ROOT`, `CODEBUDDY_PLUGIN_ROOT` - 插件根路径 (定位资源)
- `GEMINI_CWD`, `GEMINI_PROJECT_DIR` - Gemini特定 (获取工作目录)

## 监控与可观测性

**错误追踪**
- 无云错误追踪 - zjbar 不向外部服务发送错误

**日志**
- Zellij日志: `/tmp/zellij-<UID>/zellij-log/zellij.log`
  - 包含WASM插件的 `eprintln!()` 输出和运行时错误
  - 观察: `tail -f /tmp/zellij-$(id -u)/zellij-log/zellij.log | grep -i zjbar`
- 调试日志 (可选):
  - 文件: `/tmp/zjbar-debug-<pane_id>.log` (事件摘要)
  - 文件: `/tmp/zjbar-debug-raw-<pane_id>.log` (完整JSON有效负载)
  - 启用: 设置 `ZJBAR_DEBUG=1` 环境变量

**性能监控**
- 无性能追踪 - 状态栏在每次事件和Zellij信号变化时立即渲染

## CI/CD 与部署

**托管**
- GitHub (github.com/imroc/zjbar) - 源代码和发布
- npm 仓库 - 发布 `zjbar-opencode` npm包

**CI管道**
- 无CI/CD服务 (暂未实现) - 所有构建和测试在本地运行

**部署流程**

1. **版本管理**: 运行 `make bump V=x.y.z`
   - 自动更新所有7个版本文件
   - 构建WASM到 `target/wasm32-wasip1/release/zjbar.wasm`
   - 提交更改并创建git标签 `vx.y.z`

2. **发布**: 运行 `make release`
   - 推送main分支和标签到GitHub
   - 使用 `gh release create` 创建GitHub Release
   - 构建Release中包含的二进制文件: `target/wasm32-wasip1/release/zjbar.wasm`
   - 使用 `npm publish` 发布OpenCode插件到npm

3. **Claude Code插件分发**
   - 通过Claude Code插件市场 (`.claude-plugin/marketplace.json`)
   - 用户通过 `/plugin update` 命令获取更新

4. **CodeBuddy插件分发**
   - 通过CodeBuddy插件系统 (同Claude Code)

5. **Codex集成分发**
   - 通过 `make install-codex-hooks` 安装脚本
   - 用户手动运行或集成脚本自动运行

6. **Gemini集成分发**
   - 通过 `make install-gemini-hooks` 安装脚本
   - 自动注册钩子到 `~/.gemini/settings.json`

7. **OpenCode插件分发**
   - npm包 `zjbar-opencode` 自动安装到OpenCode的 node_modules
   - 版本同步: `opencode-plugin/package.json` 中的版本必须匹配主包版本

## Webhook与回调

**入站webhook**
- 无入站webhook - zjbar 不暴露HTTP端点

**出站Webhook**
- 无出站webhook - 所有通信通过Zellij pipe (本地IPC)

## 钩子系统

**Claude Code/CodeBuddy钩子**
- 文件: `hooks/hooks.json`
- 注册方式: Claude Code/CodeBuddy插件市场自动安装
- 钩子事件:
  - `PreToolUse` → 工具执行前，显示工具icon (⚡/◉/✎等)
  - `PostToolUse` → 工具执行后，返回Thinking状态 (●)
  - `PostToolUseFailure` → 工具失败，返回Thinking状态 (●)
  - `UserPromptSubmit` → 用户提交提示词，显示Thinking (●)
  - `PermissionRequest` → 权限请求，显示⚠icon，发送桌面通知+铃声
  - `Notification` → 通知事件，可选的桌面通知
  - `Stop` → 代理完成，显示✓icon，发送desktop通知
  - `SubagentStop` → 子代理完成 (仅CodeBuddy)
  - `SessionStart` → 会话开始，初始化 (◆)
  - `SessionEnd` → 会话结束，清除状态

- 脚本: `scripts/zjbar-hook.sh` (由Claude Code/CodeBuddy调用)
  - 输入: stdin中的JSON有效负载，包含 `hook_event_name`, `tool_name`, `session_id`, `cwd`, `transcript_path` 等
  - 处理:
    - 过滤噪声事件 (auth_success, permission_prompt, idle_prompt)
    - 忽略子代理事件 (检查 `agent_id` 字段)
    - CodeBuddy专用: Stop事件去抖5秒 (避免任务中途虚假通知)
    - 从会话文档提取摘要 (CodeBuddy: `~/.codebuddy/projects/<slug>/<session_id>.jsonl`, Claude Code: `~/.claude/projects/-<slug>/<session_id>.jsonl`)
    - 统计工具使用 (Write、Edit、Bash计数)
  - 输出: 构建JSON有效负载并通过 `zellij pipe --name "zjbar"` 发送

**Codex CLI集成**
- 通知方式: `~/.codex/config.toml` 中的 `notify` 回调配置
- 安装: `make install-codex-hooks` 或 `scripts/install-codex-hooks.sh`
  - 将 `scripts/zjbar-codex-notify.sh` 复制到 `$CODEX_HOME/zjbar/`
  - 将 codex-logo.png 复制到图标目录
  - 在 `config.toml` 中添加 `notify = ["$CODEX_HOME/zjbar/zjbar-codex-notify.sh"]`

- 脚本: `scripts/zjbar-codex-notify.sh`
  - 输入: 命令行参数 `$1` 中的JSON (Codex agent-turn-complete通知)
  - 唯一事件: `agent-turn-complete` 映射到 `Stop`
  - 提取: `last-assistant-message` 用于通知摘要
  - 限制: 仅支持Done (✓) 状态，无PreToolUse/Thinking/Waiting细粒度事件

**Gemini CLI集成**
- 钩子注册: `~/.gemini/settings.json` (不使用 `hooks/hooks.json`)
- 安装: `make install-gemini-hooks` 或 `scripts/install-gemini-hooks.sh`
  - 将 `scripts/zjbar-gemini-hook.sh` 复制到 `~/.gemini/zjbar/`
  - 将 gemini-logo.png 复制到图标目录
  - 在 `settings.json` 的 `hooks` 字段注册钩子

- 脚本: `scripts/zjbar-gemini-hook.sh`
  - 输入: stdin中的JSON (Gemini钩子事件), 环境变量 `ZJBAR_GEMINI_EVENT`
  - 输出: **必须**输出有效JSON到stdout (Gemini要求)，所有调试输出到stderr
  - 事件映射:
    | Gemini钩子 | zjbar事件 | 意义 |
    |----------|---------|------|
    | SessionStart | SessionStart | 会话开始 |
    | SessionEnd | SessionEnd | 会话结束 |
    | BeforeAgent | UserPromptSubmit | 用户提示词/Thinking (●) |
    | BeforeTool | PreToolUse | 工具执行前 |
    | AfterTool | PostToolUse | 工具执行后 |
    | AfterAgent | Stop | 代理完成 |

  - 工具名称映射 (Gemini → zjbar标准):
    - `run_shell_command` → Bash
    - `read_file`, `read_many_files` → Read
    - `write_file` → Write
    - `edit_file`, `replace` → Edit
    - `web_fetch` → WebFetch
    - `google_web_search` → WebSearch
    - `save_memory` → Task
    - `glob` → Glob
    - `grep` → Grep
    - `list_directory` → Read

**OpenCode插件集成**
- 源代码: `opencode-plugin/src/index.ts` (TypeScript)
- npm包: `zjbar-opencode` (@opencode-ai/plugin依赖)
- 事件处理 (在进程内实现):
  - `session.created` → SessionStart
  - `session.status` (status.type == "busy") → UserPromptSubmit (Thinking ●)
  - `session.idle` → Stop (✓), 包括获取会话摘要进行通知
  - `session.deleted` → SessionEnd
  - `permission.asked` → PermissionRequest (⚠)
  - `message.updated` (user role) → 追踪当前会话ID
  - `tool.execute.before` → PreToolUse (工具icon)
  - **故意省略** `tool.execute.after` - OpenCode在进程内运行，before/after在毫秒内触发，PostToolUse会立即覆盖工具icon为Thinking (●)

- 单例守卫:
  - OpenCode可能从多个源加载插件 (项目本地 `./opencode-plugin` 和全局npm缓存)
  - 使用环境变量 `ZJBAR_OPENCODE_ACTIVE` 实现单例: 本地插件始终优先，其他实例变为no-op
  - 两层缓存同步:
    - `~/.config/opencode/node_modules/zjbar-opencode/dist/index.js`
    - `~/.cache/opencode/node_modules/zjbar-opencode/dist/index.js`

## 桌面通知集成

**通知系统架构**

所有钩子脚本 (Claude Code、Codex、Gemini) 都使用 `scripts/zjbar-lib.sh` 中的共享通知逻辑:
- 文件: `scripts/zjbar-lib.sh`
- 函数:
  - `zjbar_load_notify_settings()` - 从 `~/.config/zellij/plugins/zjbar.json` 加载设置
  - `zjbar_is_notify_event(EVENT)` - 检查事件是否在通知列表中
  - `zjbar_is_terminal_focused()` - 检测终端应用是否处于焦点 (macOS/Linux)
  - `zjbar_check_should_notify()` - 根据模式和焦点状态决定是否发送通知
  - `zjbar_send_notification(TITLE, MESSAGE, ICON_DIR, ICON_FILE)` - 发送系统通知
  - `zjbar_clean_and_truncate(TEXT, MAXLEN)` - 清理markdown并截断摘要

**通知配置**

文件: `~/.config/zellij/plugins/zjbar.json` (用户创建，可选)

```json
{
  "notifications": "always|unfocused|off",
  "notify_events": ["PermissionRequest", "Notification", "Stop"]
}
```

- `notifications`:
  - `always` (默认) - 总是发送通知
  - `unfocused` - 仅在终端未获焦点时发送 (macOS和Linux支持焦点检测)
  - `off` - 禁用所有通知

- `notify_events` (默认: `["PermissionRequest", "Notification", "Stop"]`)
  - 列表中的事件才会触发桌面通知

**通知提供者**
- macOS:
  - 优先使用: `terminal-notifier` (需自行安装或通过Homebrew)
  - 回退: `osascript` (内置，通知中心集成)
  - 铃声: 权限请求时发送 `\x07` (BEL字符)
  - 焦点检测: 通过AppleScript (`osascript`) 获取当前焦点应用名称

- Linux:
  - 使用: `notify-send` (D-Bus/freedesktop通知)
  - 焦点检测: 可选 (`xdotool` 如果可用)

**CodeBuddy Stop事件去抖**

仅CodeBuddy有此行为 (检测 `CODEBUDDY_PROJECT_DIR` 环境变量):
- 单个用户请求可能跨越多个API调用，每个API后都发送Stop事件
- Stop通知延迟5秒: 如果期间收到非Stop事件，则取消pending通知
- 状态栏icon更新**不**去抖 - 仅通知去抖
- 文件: `/tmp/zjbar-pending-notify-<pane_id>` (存储去抖令牌)

**会话摘要提取**

从JSONL格式的会话文档中提取最后一条助手消息:

- Claude Code格式:
  ```json
  {"type":"assistant","message":{"content":[{"type":"text","text":"..."}]}}
  ```

- CodeBuddy格式:
  ```json
  {"type":"message","role":"assistant","content":[{"type":"output_text","text":"..."}]}
  ```

- 提取位置:
  - Claude Code: `~/.claude/projects/-<slug>/<session_id>.jsonl`
  - CodeBuddy: `~/.codebuddy/projects/<slug>/<session_id>.jsonl`
  - 目录slug转换: `/Users/roc/dev/zjbar` → `Users-roc-dev-zjbar`

- 处理:
  - 重试机制: 如果立即未找到文本，重试3次，延迟0.1秒(处理Stop事件与文档写入的竞态)
  - 清理: 去除markdown格式 (`**`, `*`, `` ` ``, 链接等)
  - 截断: 最多120字符，在词边界处截断，添加 `...`
  - 工具统计 (Stop事件): 计数 Write、Edit、Bash使用，格式: `Text [📝1 ✏️2 ▶3]`

**图标文件**

位置: `assets/` 目录
- `claude-logo.png` - Claude Code通知
- `codebuddy-logo.png` - CodeBuddy通知
- `codex-logo.png` - Codex通知
- `gemini-logo.png` - Gemini通知
- `opencode-logo.png` - OpenCode通知 (复制到 `opencode-plugin/assets/`)

---

*集成审计: 2025-01-23*
