# zjbar

[English](README.md) | 简体中文

一个 Zellij 状态栏插件，采用 Tokyo Night powerline 主题，并可选集成 AI 编程工具活动状态显示。

## 功能特性

- **Powerline 标签栏** — Tokyo Night 主题标签栏，段落之间使用尖锐的 powerline 箭头
- **Session 和模式显示** — 显示会话名称和输入模式（NORMAL、LOCKED、PANE 等），带有颜色编码的标签
- **可点击标签** — 点击任意标签即可切换
- **可选的 AI 编程工具集成** — 实时活动指示器、权限闪烁、桌面通知、点击聚焦（支持 Claude Code、CodeBuddy、Codex、OpenCode、Gemini CLI 及其他兼容工具）
- **多实例同步** — 所有 Zellij 标签页展示所有 AI 会话的统一视图

## 安装

### 前置条件

- [Zellij](https://zellij.dev)

### 方式一：使用发布版二进制文件

在你的 Zellij 布局文件（如 `~/.config/zellij/layouts/zjbar.kdl`）中添加 zjbar 插件：

```kdl
layout {
    default_tab_template {
        children
        pane size=1 borderless=true {
            plugin location="https://github.com/imroc/zjbar/releases/download/v1.1.37/zjbar.wasm"
        }
    }
}
```

然后使用该布局启动 Zellij：

```bash
zellij --layout ~/.config/zellij/layouts/zjbar.kdl
```

> 完整配置示例请参见 [layout.kdl](layout.kdl)，其中包含所有可用的颜色和样式选项。

### 方式二：从源码构建

前置条件：[Rust](https://rustup.rs)

```bash
git clone https://github.com/imroc/zjbar.git
cd zjbar
make install
```

这会构建 WASM 二进制文件，将其复制到 `~/.config/zellij/plugins/`，并安装布局文件。然后启动 Zellij：

```bash
zellij --layout zjbar
```

## AI 工具集成（可选）

zjbar 支持多种 AI 编程工具。每种工具通过自己的桥接方式将事件转发给 zjbar 插件（通过 `zellij pipe`）。

### Claude Code / CodeBuddy

安装 zjbar 插件以自动注册 hook：

```
/plugin marketplace add imroc/zjbar
/plugin install zjbar@zjbar
```

重启 Claude Code / CodeBuddy 使 hook 生效。

更新到最新版本：

```
/plugin update
```

### OpenCode

在 `opencode.json` 中添加 zjbar 插件：

```json
{
  "plugin": ["zjbar-opencode@latest"]
}
```

然后在使用 zjbar 布局的 Zellij 会话中启动 OpenCode，活动指示器将自动显示。

更新时重启 OpenCode 即可——因为配置了 `@latest`，它会自动拉取最新版本。

### Codex CLI

配置 Codex 使用 zjbar 通知脚本：

```bash
make install-codex-hooks
```

这会将通知脚本和图标复制到 `~/.codex/zjbar/`，并在 Codex 配置文件（`~/.codex/config.toml`）中添加 `notify` 配置项。当 Codex 完成一个 turn 时，zjbar 会显示 Done 指示器并发送包含任务摘要的桌面通知。安装后可安全删除 repo。可通过 `CODEX_HOME` 环境变量覆盖 Codex 配置目录。

更新时从最新 repo 重新运行 `make install-codex-hooks` 即可。

卸载：

```bash
make uninstall-codex-hooks
```

### Gemini CLI

首先，克隆仓库（或下载）：

```bash
git clone https://github.com/imroc/zjbar.git
cd zjbar
```

然后安装 hooks：

```bash
make install-gemini-hooks
```

这会将 hook 脚本复制到 `~/.gemini/zjbar/`，并在 `~/.gemini/settings.json` 中注册 hooks。安装后仓库可以安全删除。

卸载：

```bash
make uninstall-gemini-hooks
```

zjbar 会追踪 Gemini 的完整代理生命周期：思考中、工具使用和完成状态。

### 其他工具

zjbar 使用统一的 JSON 事件协议。任何 AI 编程工具都可以通过 `zellij pipe --name zjbar` 发送事件来集成。详见[工作原理](#工作原理)部分。

### 桌面通知（可选）

```bash
brew install terminal-notifier
```

安装后，默认会在 `PermissionRequest`、`Notification` 和 `Stop` 事件时发送桌面通知。通知包含从 AI 工具对话记录中提取的**上下文感知消息摘要**：

- **Stop** — 最后一条助手消息 + 工具使用统计（如 `📝2 ✏️3 ▶5`）
- **PermissionRequest** — 请求权限的具体命令或文件路径
- **Notification** — AI 工具发送的通知消息

可以通过 `~/.config/zellij/plugins/zjbar.json` 或**设置菜单**（点击状态栏中的 session 名称）自定义通知事件和通知模式：

```json
{
  "flash": "brief",
  "elapsed_time": true,
  "notifications": "always",
  "notify_events": ["PermissionRequest", "Notification", "Stop"]
}
```

- **`flash`** — `brief` | `persist` | `off`（默认：`brief`）。权限请求时 tab 背景闪烁模式。
- **`elapsed_time`** — `true` | `false`（默认：`true`）。在每个 tab 上显示距上次 AI 工具活动的耗时。
- **`notifications`** — `always` | `unfocused` | `off`（默认：`always`）。设为 `unfocused` 时，仅在终端不在前台时发送通知。
- **`notify_events`** — 触发通知的 hook 事件数组（默认：`["PermissionRequest", "Notification", "Stop"]`）

## 活动符号

集成 AI 编程工具（Claude Code、Codex、OpenCode、Gemini CLI 等）后，zjbar 在每个标签上显示实时活动指示器：

| 符号 | 含义           | Claude Code | CodeBuddy | Gemini CLI | OpenCode | Codex |
| ---- | -------------- | :---------: | :-------: | :--------: | :------: | :---: |
| ◆    | 会话启动中     |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ●    | 思考中         |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ⚡   | 执行 Bash 命令 |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ◉    | 读取/搜索文件  |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ✎    | 编辑/写入文件  |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ⊜    | 生成子代理     |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ◈    | 网页搜索/获取  |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ⚙    | 其他工具       |      ✔      |     ✔     |     ✔      |    ✔     |       |
| ▶    | 等待用户输入   |      ✔      |     ✔     |            |          |       |
| ⚠    | 等待权限确认   |      ✔      |     ✔     |            |    ✔     |       |
| ✓    | 完成           |      ✔      |     ✔     |     ✔      |    ✔     |   ✔   |

> **注意：** Codex CLI 仅提供 `agent-turn-complete` 事件（对应 ✓ 完成），不支持其他细粒度状态。Gemini CLI 没有权限确认和等待用户输入的 hook 事件。

## 配置

所有视觉设置均通过 KDL 布局文件配置。每个选项都是可选的 — 默认使用 Tokyo Night 主题。

行为设置（`flash`、`elapsed_time`、`notifications`）存储在 `~/.config/zellij/plugins/zjbar.json` 中，可通过设置菜单（点击 session 名称）在运行时修改。

```kdl
plugin location="zjbar.wasm" {
    // 颜色：任意 "#rrggbb" 十六进制值
    bar_bg          "#1a1b26"
    session_bg      "#7aa2f7"
    session_fg      "#16161e"
    tab_active_bg   "#292e42"
    tab_active_fg   "#c0caf5"
    tab_inactive_bg "#16161e"
    tab_inactive_fg "#a9b1d6"

    // 模式颜色：mode_<name>_bg / mode_<name>_fg
    // 模式：normal, locked, pane, tab, resize, move, scroll,
    //       search, entersearch, session, prompt, renametab,
    //       renamepane, tmux

    // 活动图标颜色
    activity_thinking_color "#bb9af7"
    activity_tool_color     "#ff9e64"

    // 分隔符（powerline 字符）
    separator_left ""     // \ue0b0
    separator_tab  ""     // \ue0b1

    // Tab 指示器（全屏/浮动窗格时显示）
    tab_fullscreen_indicator " 󰊓"
    tab_floating_indicator " 󰹙"
}
```

完整配置选项及默认值请参见 [layout.kdl](layout.kdl)。

## 工作原理

1. **WASM 插件** — 在 Zellij 内运行，渲染状态栏，管理状态
2. **Hook / 插件桥接**（可选） — 通过 `zellij pipe` 转发 AI 编程工具事件

```
Claude Code hook → zjbar-hook.sh        → zellij pipe → 插件 → 渲染
Codex notify     → zjbar-codex-notify.sh → zellij pipe → 插件 → 渲染
OpenCode plugin  → zjbar-opencode-plugin → zellij pipe → 插件 → 渲染
Gemini CLI hook  → zjbar-gemini-hook.sh  → zellij pipe → 插件 → 渲染
```

所有集成使用统一的 JSON payload，通过 `source` 字段标识 AI 工具来源。

## 卸载

```bash
make uninstall
```

## 许可证

MIT
