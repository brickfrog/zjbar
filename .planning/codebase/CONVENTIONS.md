# 编码规范

**分析日期：** 2024-12-19

## 命名规范

### 文件

- **模块文件**：小写 + 下划线 (snake_case)
  - `src/main.rs` — 插件入口
  - `src/config.rs` — 配置解析
  - `src/render.rs` — 状态栏渲染
  - `src/state.rs` — 状态定义
  - `src/event_handler.rs` — 事件处理
  - `src/tab_pane_map.rs` — 标签窗格映射

### 函数和方法

- **snake_case** 命名
- 公开函数使用 `pub fn`，私有函数不加前缀
- 例如：
  - `pub fn unix_now() -> u64` — 获取当前时间戳（秒）
  - `pub fn unix_now_ms() -> u64` — 获取当前时间戳（毫秒）
  - `fn parse_hex_color(s: &str) -> Option<Color>` — 私有颜色解析
  - `fn char_width(c: char) -> usize` — 字符宽度计算
  - `fn display_width(s: &str) -> usize` — 字符串显示宽度

### 常量和类型

- **大写 + 下划线** (UPPER_SNAKE_CASE)
  - `DONE_TIMEOUT` — 活动完成后多久重置（秒）
  - `TIMER_INTERVAL` — 定时器间隔（秒）
  - `FLASH_TICK` — 闪烁帧间隔（秒）
  - `ELAPSED_THRESHOLD` — 显示经过时间的阈值（秒）

- **枚举和结构体**：PascalCase
  - `Activity` — 活动状态枚举
  - `SessionInfo` — 会话信息结构体
  - `BarConfig` — 状态栏配置结构体
  - `MenuAction` — 菜单动作枚举
  - `FlashMode` — 闪烁模式枚举

### 变量

- **snake_case** 命名
- 例如：
  - `session_text` — 会话名称文本
  - `session_pill_width` — 会话药丸宽度
  - `col` — 列位置指针
  - `tab_index` — 标签索引
  - `pane_id` — 窗格 ID

## 代码风格

### 格式化

- **工具**：Rust 默认的 rustfmt（无自定义配置）
- **目标**：遵循 Rust 社区标准
- 使用 `cargo fmt` 自动格式化代码
- `rust-toolchain.toml` 钉死 Rust stable 版本和目标平台 `wasm32-wasip1`

### 缩进

- **4 个空格**缩进（Rust 标准）
- 一行最大长度：遵循默认 rustfmt 设置（通常 100 字符）

### 代码位置和公有/私有边界

**模块组织：**

`src/main.rs` 中：
- 导入模块声明（第 1-5 行）：`mod config;`, `mod render;` 等
- 导入依赖（第 6-13 行）：`use` 语句
- 常量定义
- `register_plugin!(State);` 宏
- 主实现块：`impl ZellijPlugin for State`
- 辅助实现块：`impl State` 中的私有方法

**访问控制：**
- 结构体/枚举字段：大多数公开，无 `pub` 的字段仅在内部模块使用
- 函数：`pub fn` 用于外部 API，不加 `pub` 为私有
- 例如 `src/config.rs`：
  - `pub struct BarConfig { pub bar_bg: Color, ... }`
  - `fn parse_hex_color(...)` — 私有
  - `fn get_color(...)` — 私有助手
  - `pub fn from_kdl(...)` — 公开配置构造器
  - `pub fn mode_style(...)` — 公开查询方法
  - `pub fn activity_color(...)` — 公开颜色查询

## 导入组织

**顺序规范：**

1. 标准库导入（`std::...`）
2. 外部库（依赖包）
3. 内部模块和类型（`use crate::...`）

**示例（来自 `src/main.rs`）：**

```rust
mod config;
mod event_handler;
// ... 其他模块

use config::BarConfig;
use state::{...};
use std::collections::BTreeMap;
use zellij_tile::prelude::*;
```

**示例（来自 `src/render.rs`）：**

```rust
use crate::config::Color;
use crate::state::{...};
use std::fmt::Write;
use std::io::Write as IoWrite;
use zellij_tile::prelude::TabInfo;
```

**路径别名：** 无（直接使用 `crate::` 前缀）

## 错误处理

### Result 和 Option 使用

**原则：** 使用 `Option` 和 `?` 运算符处理可恢复错误，对不可恢复的错误使用默认值。

**模式示例：**

1. **颜色解析（`src/config.rs`）**：
   ```rust
   fn parse_hex_color(s: &str) -> Option<Color> {
       let s = s.trim().trim_start_matches('#');
       if s.len() != 6 {
           return None;
       }
       let r = u8::from_str_radix(&s[0..2], 16).ok()?;
       let g = u8::from_str_radix(&s[2..4], 16).ok()?;
       let b = u8::from_str_radix(&s[4..6], 16).ok()?;
       Some((r, g, b))
   }
   ```
   — 使用 `?` 链式传播错误

2. **配置读取（`src/config.rs`）**：
   ```rust
   fn get_color(config: &BTreeMap<String, String>, key: &str, default: Color) -> Color {
       config
           .get(key)
           .and_then(|v| parse_hex_color(v))
           .unwrap_or(default)
   }
   ```
   — 失败时使用默认值

3. **JSON 解析（`src/main.rs`）**：
   ```rust
   let payload: HookPayload = match serde_json::from_str(payload_str) {
       Ok(p) => p,
       Err(_) => return false,
   };
   ```
   — 显式 `match` 错误处理，返回 `false` 表示处理失败

### unwrap() 使用

**允许情况：**
- 使用 `unwrap_or()` 和 `unwrap_or_default()` — 提供 fallback 值
  ```rust
  .unwrap_or("zellij")        // 字符串默认值
  .unwrap_or_default()         // 类型的 Default 实现
  .unwrap_or(0)                // 数字默认值
  ```

- 在 JSON 序列化中（不会失败）：
  ```rust
  serde_json::to_string(&self.sessions).unwrap_or_default()
  ```

**禁止情况：**
- 直接 `unwrap()` — 如果生产环境可能失败，不允许
- `expect()` — 用 `unwrap_or()` 替代

## 日志记录

### 调试输出

- **标准输出**：生产级输出使用 `print!()` 和 `println!()`
- **标准错误**：日志使用 `eprintln!()`（可重定向到日志文件）
- **WASM 日志**：输出被 Zellij 捕获，写入 `/tmp/zellij-<UID>/zellij-log/zellij.log`

**示例（`src/render.rs`）：**

```rust
pub fn render_status_bar(state: &mut State, _rows: usize, cols: usize) {
    // ... 准备输出
    print!("{buf}");  // 直接输出到标准输出
    let _ = std::io::stdout().flush();
}
```

**调试时添加日志：**
- 临时 `eprintln!()` 调试 WASM 问题（写入 Zellij 日志）
- **清除规则**：提交前移除所有调试输出

## 注释

### 文档注释

- **函数级**：使用 `/// ` 格式的 doc comments
- **描述**：一行简述函数功能
- **参数和返回值**：按需说明

**示例（`src/render.rs`）：**

```rust
/// Number of decimal digits in a positive integer (e.g. 1→1, 10→2, 100→3).
fn digit_count(n: usize) -> usize { ... }

/// Render a single toggle menu item: "● Label" or "○ Label".
/// Returns false if there was not enough space.
fn render_menu_item(...) -> bool { ... }
```

### 行内注释

- **结构说明**：使用 `//` 与代码段关联
- **复杂逻辑**：标记关键步骤

**示例（`src/render.rs`）：**

```rust
// === Left prefix: [session pill][arrow][mode pill][arrow] ===

// Session pill (clickable to toggle settings)
let prefix_start = col;
...

// Powerline arrow: idx_bg → tab_bg
write_fg!(buf, idx_bg);
...
```

### 宏注释

- **宏文档**：在定义处添加 `///` 说明用途

**示例（`src/config.rs`）：**

```rust
/// Macro to reduce boilerplate in `from_kdl`. Each arm maps a field name and KDL
/// config key to a default color constant, calling `get_color` once per field.
macro_rules! color_fields { ... }
```

## 函数设计

### 大小

- **目标**：单个函数在 50-150 行之间
- **复杂函数**：拆分为多个私有助手函数

**示例：**
- `render_status_bar()` — 90 行，顶级编排
- `render_single_tab()` — 150 行，单个标签的详细渲染
- `render_tabs()` — 70 行，多标签逻辑
- `compute_tab_info()` — 50 行，计算标签活动信息

### 参数

- **约定**：使用引用传递，避免大量克隆
- **可变性**：需要修改状态时使用 `&mut`

**示例（`src/main.rs` 的 `ZellijPlugin impl`）：**

```rust
fn update(&mut self, event: Event) -> bool { ... }  // 自身可变
fn pipe(&mut self, pipe_message: PipeMessage) -> bool { ... }
```

### 返回值

- **布尔值**：表示是否需要重新渲染
  ```rust
  fn update(&mut self, event: Event) -> bool  // true = 需要渲染
  fn pipe(&mut self, pipe_message: PipeMessage) -> bool
  ```

- **Options**：用于可选结果
  ```rust
  fn parse_hex_color(s: &str) -> Option<Color>
  ```

- **Units**：修改状态的内部函数
  ```rust
  fn refresh_session_tab_names(&mut self)  // 无返回值，修改 state
  ```

## 模块设计

### 导出

- **模块头部**：根据需要使用 `pub use` 重导出关键类型

**示例（`src/state.rs`）：**

```rust
pub fn unix_now() -> u64 { ... }  // 时间工具函数
pub struct SessionInfo { ... }     // 会话结构体
pub enum Activity { ... }          // 活动枚举
```

### 桶形文件

- **无桶形文件**：每个模块独立导出，无 `mod.rs` 或 `lib.rs` 中的重导出

## 空格和换行

- **逻辑段落**：函数内部使用 1-2 行空白分隔逻辑块
- **结构体字段**：相关字段间无空行，但在语义转变处加空行

**示例（`src/state.rs`）：**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Activity {
    Init,
    Thinking,
    Tool(String),
    Prompting,
    Waiting,
    Notification,
    Done,
    AgentDone,
    Idle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub pane_id: u32,
    pub activity: Activity,
    // ...
    pub cwd: Option<String>,
}
```

## 宏

### 使用场景

**专用宏：**

1. **`write_fg!()` 和 `write_bg!()`**（`src/render.rs`）
   - 用于 ANSI 颜色代码生成
   - 减少重复的转义序列输出

2. **`color_fields!()`**（`src/config.rs`）
   - 声明结构体所有颜色字段的样板代码
   - 减少 `from_kdl()` 方法中的重复

## 共同模式

### 状态管理

- **中央状态结构**：`State` 结构体（`src/state.rs`）
- **不可变引用**：查询操作
- **可变引用**：更新操作

**示例（`src/main.rs`）：**

```rust
impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) { ... }
    fn update(&mut self, event: Event) -> bool { ... }
    fn render(&mut self, rows: usize, cols: usize) { ... }
}
```

### 循环和映射

- **使用迭代器**：优于显式循环

**示例（`src/tab_pane_map.rs`）：**

```rust
let tab_name_by_position: HashMap<usize, String> = tabs
    .iter()
    .map(|t| (t.position, t.name.clone()))
    .collect();
```

### 枚举匹配

- **详尽匹配**：对已知枚举穷尽所有分支
- **通配符 `_`**：未来扩展的枚举分支

**示例（`src/config.rs`）：**

```rust
pub fn mode_style(&self, mode: zellij_tile::prelude::InputMode) -> (Color, Color, &'static str) {
    use zellij_tile::prelude::InputMode;
    match mode {
        InputMode::Normal => (self.mode_normal_bg, self.mode_normal_fg, "NORMAL"),
        InputMode::Locked => (self.mode_locked_bg, self.mode_locked_fg, "LOCKED"),
        // ... 所有变体都覆盖
    }
}
```

---

*编码规范分析：2024-12-19*
