# 测试模式

**分析日期：** 2024-12-19

## 测试框架

### 运行器

- **框架**：Rust 内置 `cargo test`（基于 libtest）
- **配置**：无专门配置文件（使用默认设置）
- **目标平台**：`wasm32-wasip1`（WASM 运行环境）

### 运行命令

```bash
# 运行所有单元测试
cargo test --lib

# 在 WASM 平台上运行测试（需要特殊运行器）
cargo test --lib --target wasm32-wasip1

# 监视模式
cargo test -- --nocapture  # 显示 println! 输出

# 特定模块的测试
cargo test config::tests
cargo test render::tests
```

**输出格式：** 标准 Rust 测试输出（绿色 ✓，红色 ✗）

### 断言库

- **库**：Rust 标准库 `assert_eq!()` 和 `assert!()`
- **扩展**：无外部断言库依赖

## 测试文件组织

### 位置

- **模式**：co-located（测试与源文件同文件）
- **目录**：无单独 `tests/` 目录

**测试在以下源文件中：**
- `src/config.rs` — 11 个测试
- `src/render.rs` — 17 个测试

### 命名

- **测试函数**：`#[test] fn test_<function_name>_<scenario>()`
- **测试模块**：`#[cfg(test)] mod tests { ... }`

**示例（`src/config.rs`）：**

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn parse_hex_color_valid() { ... }

    #[test]
    fn parse_hex_color_without_hash() { ... }

    #[test]
    fn parse_hex_color_with_whitespace() { ... }

    #[test]
    fn parse_hex_color_invalid() { ... }
}
```

### 结构

**模块级结构（位于每个源文件末尾）：**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // 测试分组（按功能）
    // -- test_category_1 --

    #[test]
    fn test_function_name_case1() { ... }

    #[test]
    fn test_function_name_case2() { ... }

    // -- test_category_2 --

    #[test]
    fn test_other_function_case1() { ... }
}
```

**示例（`src/render.rs`）：**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // -- char_width --

    #[test]
    fn char_width_ascii() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width('Z'), 1);
    }

    #[test]
    fn char_width_cjk() {
        assert_eq!(char_width('中'), 2);
    }

    // -- display_width --

    #[test]
    fn display_width_ascii() {
        assert_eq!(display_width("hello"), 5);
    }
}
```

## 测试结构

### Suite 组织

**文件级套件：**

每个模块独立的测试块，按功能分类。无嵌套套件。

### 模式

#### 设置/清理

- **无显式设置/清理**：测试函数是原子的，无副作用
- **状态隔离**：每个测试创建自己的数据结构

**示例（`src/config.rs`）：**

```rust
#[test]
fn get_color_uses_config() {
    let mut config = BTreeMap::new();  // 设置：创建新 map
    config.insert("bg".into(), "#ff0000".into());
    assert_eq!(get_color(&config, "bg", (0, 0, 0)), (255, 0, 0));
    // 清理：自动（本地变量超出作用域）
}
```

#### 断言模式

**标准 Rust 断言：**

```rust
assert_eq!(actual, expected);        // 相等性测试
assert!(condition);                  // 布尔测试
assert_ne!(a, b);                    // 不相等性测试
```

**示例（`src/render.rs`）：**

```rust
#[test]
fn activity_priority_ordering() {
    assert!(activity_priority(&Activity::Waiting) > 
            activity_priority(&Activity::Tool("Bash".into())));
}

#[test]
fn digit_count_values() {
    assert_eq!(digit_count(0), 1);
    assert_eq!(digit_count(10), 2);
    assert_eq!(digit_count(100), 3);
}
```

#### 异步测试

- **不适用**：项目中无异步代码（WASM 同步运行）

## Mock 和 Fixtures

### 框架

- **Mock 框架**：无（使用直接测试）
- **Fixture 库**：无

### Mock 模式

**直接创建测试对象：**

```rust
#[test]
fn parse_hex_color_valid() {
    let result = parse_hex_color("#7aa2f7");
    assert_eq!(result, Some((122, 162, 247)));
}
```

**模拟状态结构体：**

```rust
#[test]
fn activity_symbol_tools() {
    assert_eq!(activity_symbol(&Activity::Tool("Bash".into())), "⚡");
    assert_eq!(activity_symbol(&Activity::Tool("Read".into())), "◉");
}
```

### Mock 准则

**什么需要 Mock：**
- 外部 I/O（网络、文件系统）— 项目中尽量避免
- Zellij 插件 API 调用 — 通过不调用实现隔离

**什么不需要 Mock：**
- 数据结构：直接创建
- 配置：使用 `BTreeMap::new()` 或默认值
- 枚举：直接构造

## Fixtures 和工厂

### 测试数据

**直接构造（无工厂）：**

```rust
#[test]
fn default_config_matches_from_empty() {
    let default = BarConfig::default();
    assert_eq!(default.bar_bg, D_BAR_BG);
}

#[test]
fn get_color_uses_default() {
    let config = BTreeMap::new();  // 空 map 即是 fixture
    assert_eq!(get_color(&config, "missing", (1, 2, 3)), (1, 2, 3));
}
```

### Fixture 位置

- **内联**：在测试函数体内创建
- **无专门 fixture 目录**

## 覆盖率

### 要求

- **覆盖率目标**：无强制要求
- **现状**：28 个测试覆盖关键函数

**测试覆盖的模块：**
- `src/config.rs` — 11 个测试
  - `parse_hex_color()` — 完全覆盖（有效、无 #、空格、无效）
  - `get_color()` — 覆盖默认值和配置读取
  - `BarConfig::from_kdl()` — 默认行为测试

- `src/render.rs` — 17 个测试
  - `char_width()` — ASCII、CJK、全角
  - `display_width()` — 混合内容
  - `digit_count()` — 各位数
  - `format_elapsed()` — 秒/分/小时转换
  - `activity_priority()` — 优先级排序
  - `activity_symbol()` — 工具符号映射、状态符号

### 查看覆盖率

```bash
# 生成覆盖率报告（需要 tarpaulin）
cargo tarpaulin --lib --out Html

# 简单输出
cargo tarpaulin --lib
```

**当前覆盖范围：**
- 数据转换函数 ✓（`parse_hex_color`, `format_elapsed`, `digit_count`）
- 查询函数 ✓（`get_color`, `char_width`, `display_width`）
- 枚举映射 ✓（`activity_symbol`, `activity_priority`）
- 未覆盖：UI 渲染逻辑（需要集成测试或 tmux 验证）

## 测试类型

### 单元测试

**范围：** 纯函数，无状态副作用

**示例：**

```rust
#[test]
fn parse_hex_color_valid() {
    assert_eq!(parse_hex_color("#7aa2f7"), Some((122, 162, 247)));
}

#[test]
fn format_elapsed_minutes() {
    assert_eq!(format_elapsed(60), "1m");
    assert_eq!(format_elapsed(120), "2m");
}
```

### 集成测试

**位置**：无专门 `tests/` 目录

**现状：** 集成测试通过外部 tmux 脚本验证（见 `AGENTS.md`）
- `cargo build --release` 编译 WASM
- tmux 启动 Zellij 会话
- 捕获状态栏输出验证渲染

**示例（来自 AGENTS.md）：**

```bash
# 构建并部署
cargo build --release --target wasm32-wasip1
cp target/wasm32-wasip1/release/zjbar.wasm ~/.config/zellij/plugins/

# 启动测试会话
zellij -s zjbar_test -n layout.kdl

# 检查输出
tmux capture-pane -t zjbar_test -p | tail -1
```

### E2E 测试

**框架**：无标准 E2E 框架

**验证方法：** 手动和 tmux 脚本
- 创建标签和窗格
- 发送 mock hook 事件
- 验证状态栏图标和颜色

**示例（来自 AGENTS.md）：**

```bash
# 发送 mock 事件
zellij -s zjbar_test pipe --name zjbar -- \
  '{"source":"claude","pane_id":1,"session_id":"test-session","hook_event":"PreToolUse","tool_name":"Bash"}'

sleep 1
tmux capture-pane -t zjbar_test -p | tail -1
# 应该显示 ⚡ 图标
```

## 常用模式

### 异步测试

- **不适用**：项目全同步
- 所有测试函数为 `fn test_xxx()` 而非 `async fn test_xxx()`

### 错误测试

**模式：** 测试 `Option::None` 返回

```rust
#[test]
fn parse_hex_color_invalid() {
    assert_eq!(parse_hex_color(""), None);
    assert_eq!(parse_hex_color("#fff"), None);      // 太短
    assert_eq!(parse_hex_color("#gggggg"), None);   // 无效 hex
}
```

### 边界值测试

```rust
#[test]
fn digit_count_values() {
    assert_eq!(digit_count(0), 1);    // 边界：0
    assert_eq!(digit_count(1), 1);    // 边界：1
    assert_eq!(digit_count(9), 1);    // 边界：9
    assert_eq!(digit_count(10), 2);   // 转折：两位数
    assert_eq!(digit_count(99), 2);   // 边界：99
    assert_eq!(digit_count(100), 3);  // 转折：三位数
}
```

### 属性测试

- **无依赖**：未使用 `proptest` 或 `quickcheck`
- 采用手动枚举关键场景

## 编写新测试指南

### 步骤

1. **定位模块**：在要测试的模块文件末尾
2. **创建测试块**（如果不存在）：
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       // tests here
   }
   ```

3. **添加测试函数**：
   ```rust
   #[test]
   fn test_<function>_<case>() {
       // Arrange: 设置数据
       let input = ...;
       
       // Act: 调用函数
       let result = some_function(input);
       
       // Assert: 验证结果
       assert_eq!(result, expected);
   }
   ```

4. **命名规范**：
   - `test_<function_name>_<scenario>`
   - 场景：`valid`, `invalid`, `edge_case`, `with_whitespace` 等

5. **运行**：
   ```bash
   cargo test --lib
   ```

### 示例（假设为 `src/event_handler.rs` 添加测试）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_hook_event_session_start() {
        let mut state = State::default();
        let payload = HookPayload {
            pane_id: 1,
            hook_event: "SessionStart".into(),
            ..Default::new()
        };
        
        handle_hook_event(&mut state, payload);
        
        assert_eq!(state.sessions.get(&1).unwrap().activity, Activity::Init);
    }

    #[test]
    fn handle_hook_event_tool_use() {
        let mut state = State::default();
        let payload = HookPayload {
            pane_id: 1,
            hook_event: "PreToolUse".into(),
            tool_name: Some("Bash".into()),
            ..Default::new()
        };
        
        handle_hook_event(&mut state, payload);
        
        match &state.sessions.get(&1).unwrap().activity {
            Activity::Tool(name) => assert_eq!(name, "Bash"),
            _ => panic!("Expected Activity::Tool"),
        }
    }
}
```

## CI/CD

### 测试在 CI 中的角色

**当前状态**：无自动化 CI 管道

**建议流程**（如要添加）：

```bash
# 在 pull request 时自动运行
cargo test --lib
cargo clippy -- -D warnings
cargo fmt -- --check
```

### 本地测试命令

```bash
# 完整验证周期
cargo test --lib          # 运行所有单元测试
cargo clippy -- -W        # Rust linter 警告
cargo fmt -- --check      # 格式检查
cargo build --release     # WASM 编译
```

---

*测试分析：2024-12-19*
