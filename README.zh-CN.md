# zjbar

[English](README.md) | 简体中文

一个 Zellij 状态栏插件，采用 Tokyo Night powerline 主题，并从 [choir](https://github.com/brickfrog/choir) 获取权威的代理状态。

这是面向 choir 工作流的 [imroc/zjbar](https://github.com/imroc/zjbar) 自有 fork。fork 名称仍为 `zjbar`；上游 MIT 署名保留在 [LICENSE](LICENSE) 中。

## 功能特性

- **统一 Powerline 导航栏** — Tokyo Night 主题、可点击的单行导航，先显示原生 Zellij tab，再显示 choir 叶子入口
- **Session 和模式显示** — 显示会话名称和输入模式（NORMAL、LOCKED、PANE 等），带有颜色编码的标签
- **Choir 状态源** — 轮询 `.choir/server.sock` 的类型化 `status_bar_state` 快照；不使用进程名推断
- **Choir 状态折叠** — 在对应 tab 或叶子 pane 旁渲染生命周期符号，并折叠解析到同一个 Zellij pane 的重复代理
- **原生快捷键栏** — 保留 Zellij 原来的底部 status-bar 插件，用于显示快捷键提示
- **点击聚焦窗格** — 点击任意合成的 choir 叶子入口即可聚焦对应的 Zellij pane
- **优雅降级** — 保持原生 tab 导航可用，显示紧凑的 choir 源错误，并对不支持的 schema fail closed

## 安装

### 前置条件

- [Zellij](https://zellij.dev)
- `PATH` 中可用的 Python 3（WASM 插件通过宿主侧 UDS 桥接使用）
- choir 在 `.choir/server.sock` 上暴露 `status_bar_state` MCP 命令

### 方式一：使用发布版二进制文件

在你的 Zellij 布局文件（如 `~/.config/zellij/layouts/zjbar.kdl`）中添加 zjbar 插件：

```kdl
layout {
    default_tab_template {
        pane size=1 borderless=true {
            plugin location="https://github.com/brickfrog/zjbar/releases/download/v1.3.0/zjbar.wasm" {
                choir_socket ".choir/server.sock"
            }
        }
        children
        pane size=1 borderless=true {
            plugin location="zellij:status-bar"
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
git clone https://github.com/brickfrog/zjbar.git
cd zjbar
make install
```

这会构建 WASM 二进制文件，将其复制到 `~/.config/zellij/plugins/`，并安装布局文件。然后启动 Zellij：

```bash
zellij --layout zjbar
```

## Choir 集成

zjbar 每秒轮询一次 choir Unix domain socket，并发送换行分帧的 JSON-RPC MCP 请求：

```json
{"jsonrpc":"2.0","id":"zjbar-status-bar-state","method":"tools/call","params":{"name":"status_bar_state","arguments":{}}}
```

响应 payload 必须匹配 [protocol/status_bar_state.json](protocol/status_bar_state.json)。渲染器只消费这个类型化快照，并使用 Zellij pane manifest 获取 tab/pane 标题；它不会检查进程名，也不会从 Claude/Codex/OpenCode/Gemini hook 事件中推断状态。

Choir 生命周期状态会折叠进主导航行。真实 Zellij tab 保留原生 tab 编号和点击行为；非顶层 choir pane 会追加为可聚焦入口，并优先使用 Zellij 暴露的 pane 标题。如果多个 choir 代理解析到同一个 Zellij pane，zjbar 会为该 pane 选择优先级最高或最新的状态，而不是渲染重复的 `agent-*` 入口。

在 choir 暴露 `status_bar_state` 之前，或 `.choir/server.sock` 不可达时，zjbar 会保持原生 tab 行可用，并渲染紧凑的 `no choir` 指示器，不渲染推断的 pane 字段。如果服务端返回的 `schema_version` 大于客户端支持版本，zjbar 会渲染 `schema ahead vN`，并拒绝渲染任何 pane 字段。

旧 hook 脚本仍保留在仓库中，方便上游兼容维护；但这个 fork 的状态渲染由 choir 状态驱动。

运行时设置（`flash`、`elapsed_time`、`notifications`）仍存储在 `~/.config/zellij/plugins/zjbar.json` 中，可通过设置菜单（点击 session 名称）修改：

```json
{
  "flash": "brief",
  "elapsed_time": true,
  "notifications": "always",
  "notify_events": ["PermissionRequest", "Notification", "Stop"]
}
```

- **`flash`** — `brief` | `persist` | `off`（默认：`brief`）。Choir attention 脉冲使用 flash 颜色。
- **`elapsed_time`** — `true` | `false`（默认：`true`）。为旧 tab 渲染辅助逻辑保留。
- **`notifications`** — `always` | `unfocused` | `off`（默认：`always`）。为旧 hook 脚本保留；choir attention 触发 notify-send 不在此 fork 范围内。
- **`notify_events`** — 旧 hook 通知事件数组（默认：`["PermissionRequest", "Notification", "Stop"]`）

## 生命周期符号

Choir lifecycle 值渲染为：

| 符号 | Lifecycle              |
| ---- | ---------------------- |
| ⏳   | `working`              |
| 👁   | `review_owned`         |
| ✎    | `changes_requested`    |
| ✓    | `done`                 |
| ✗    | `failed`               |
| ⊖    | `exitable`             |
| 🔴   | `waiting_for_red_gate` |

合成的 choir 叶子入口还会在字段存在时显示 PR 编号、未解决 review thread 数量和 CI 汇总。当 `attention_needed` 为 true 时，对应导航入口会使用配置的 flash 颜色脉冲约两秒。

## 配置

所有视觉设置均通过 KDL 布局文件配置。每个选项都是可选的 — 默认使用 Tokyo Night 主题。

行为设置（`flash`、`elapsed_time`、`notifications`）存储在 `~/.config/zellij/plugins/zjbar.json` 中，可通过设置菜单（点击 session 名称）在运行时修改。

```kdl
plugin location="zjbar.wasm" {
    // Choir 状态源
    choir_socket ".choir/server.sock"

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

    // 生命周期图标颜色
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

1. **WASM 插件** — 作为顶部 chrome 在 Zellij 内运行，渲染统一的 tab/choir 导航行，管理点击区域和 flash timing。
2. **宿主侧 UDS 桥接** — Zellij `run_command` 调用 Python 3 连接 `.choir/server.sock`，发送 `status_bar_state` JSON-RPC 请求，并返回一条换行分帧响应。
3. **类型化快照渲染器** — Rust 按 v1 schema 解析响应，只渲染已识别字段。默认布局把 zjbar 放在顶部，恢复 Zellij 原生快捷键/状态栏到底部，并把 choir 状态折叠到与 Zellij tab 相同的可选中行中。所有失败都会 fail closed。

## 卸载

```bash
make uninstall
```

## 许可证

MIT
