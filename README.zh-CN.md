# zjbar

[English](README.md) | 简体中文

一个 Zellij 状态栏插件，采用 Tokyo Night powerline 主题，并可选集成 Claude Code 活动状态显示。

## 功能特性

- **Powerline 标签栏** — Tokyo Night 主题标签栏，段落之间使用尖锐的 powerline 箭头
- **Session 和模式显示** — 显示会话名称和输入模式（NORMAL、LOCKED、PANE 等），带有颜色编码的标签
- **可点击标签** — 点击任意标签即可切换
- **可选的 Claude Code 集成** — 实时活动指示器、权限闪烁、桌面通知、点击聚焦
- **多实例同步** — 所有 Zellij 标签页展示所有 Claude 会话的统一视图

## 安装

### 前置条件

- [Zellij](https://zellij.dev)

### 方式一：Claude Code 插件（推荐）

安装为 Claude Code 插件，可自动注册 hook 并一键安装：

```
/plugin marketplace add imroc/zjbar
/plugin install zjbar@zjbar
```

然后下载 WASM 插件和布局文件：

```
/zjbar:install
```

重启 Claude Code 使 hook 生效，然后启动 Zellij：

```bash
zellij --layout zjbar
```

### 方式二：仅 Zellij 布局

直接在 Zellij 布局文件中添加插件（无 Claude Code 集成）：

```kdl
default_tab_template {
    children
    pane size=1 borderless=true {
        plugin location="https://github.com/imroc/zjbar/releases/download/v1.0.4/zjbar.wasm"
    }
}
```

### 方式三：从源码构建

前置条件：[Rust](https://rustup.rs)、[jq](https://jqlang.github.io/jq/)（用于 hook）

```bash
git clone https://github.com/imroc/zjbar.git
cd zjbar
./install.sh
```

或直接使用 make 命令：

```bash
make               # 构建 wasm + 更新插件
make install       # 构建 + 安装布局文件
make install-hooks # 注册 Claude Code hooks
make uninstall     # 移除插件和布局文件
make release       # 创建 GitHub release（需要 HEAD 上有 tag）
```

hook 安装脚本会自动检测设置文件路径（`~/.claude-internal/settings.json` 或 `~/.claude/settings.json`）。如需指定自定义路径：

```bash
CLAUDE_SETTINGS=~/.codebuddy/settings.json make install-hooks
```

### 可选：点击聚焦通知

```bash
brew install terminal-notifier
```

安装后，默认会在 `PermissionRequest`、`Notification` 和 `Stop` 事件时发送桌面通知。通知包含从 Claude Code 对话记录中提取的**上下文感知消息摘要**：

- **Stop** — 最后一条助手消息 + 工具使用统计（如 `📝2 ✏️3 ▶5`）
- **PermissionRequest** — 请求权限的具体命令或文件路径
- **Notification** — Claude Code 发送的通知消息

可以通过 `~/.config/zellij/plugins/zjbar.json` 自定义通知事件和通知模式：

```json
{
  "notify_events": ["PermissionRequest", "Notification", "Stop"],
  "notifications": "always"
}
```

- **`notify_events`** — 触发通知的 Claude Code hook 事件数组（默认：`["PermissionRequest", "Notification", "Stop"]`）
- **`notifications`** — `always` | `unfocused` | `off`（默认：`always`）。设为 `unfocused` 时，仅在终端不在前台时发送通知。

## Claude Code 活动符号

| 符号 | 含义                |
| ---- | ------------------- |
| ◆    | 会话启动中          |
| ●    | 思考中              |
| ⚡   | 执行 Bash 命令      |
| ◉    | 读取/搜索文件       |
| ✎    | 编辑/写入文件       |
| ⊜    | 生成子代理          |
| ◈    | 网页搜索/获取       |
| ⚙    | 其他工具            |
| ▶    | 等待用户输入        |
| ⚠    | 等待权限确认        |
| ✓    | 完成                |

## 配置

所有视觉和行为设置均通过 KDL 布局文件配置。每个选项都是可选的 — 默认使用 Tokyo Night 主题。

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

    // 行为
    flash         "brief"    // persist | brief | off
    elapsed_time  "true"     // true | false
}
```

完整配置选项及默认值请参见 [layout.kdl](layout.kdl)。

## 工作原理

1. **WASM 插件** — 在 Zellij 内运行，渲染状态栏，管理状态
2. **Hook 脚本**（可选） — bash 桥接脚本，通过 `zellij pipe` 转发 Claude Code 事件

```
Claude Code hook → zjbar-hook.sh → zellij pipe → 插件 → 渲染
```

## 卸载

```bash
make uninstall
```

或者，如果是作为 Claude Code 插件安装的：

```
/plugin uninstall zjbar@zjbar
```

## 许可证

MIT
