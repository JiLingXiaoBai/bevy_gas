# AGENTS.md — bevy_tools

## 项目概述

`bevy_tools` 是一个为 [Bevy](https://bevyengine.org/) 游戏引擎打造的
**Gameplay Ability System (GAS)** 库，设计灵感来源于虚幻引擎的 GAS 框架。
它提供了模块化的 ECS 友好架构，用于构建复杂的 RPG/MOBA/ARPG 游戏机制。

## 技术栈

- **语言：** Rust (edition 2024)
- **引擎：** Bevy 0.19
- **额外依赖：** `rand` 0.10.2
- **许可证：** MIT

## 项目目标：

- 高性能
- ECS 优先架构
- 易于维护
- 数据驱动、尽可能保持确定性（Deterministic）
- 尽量减少不必要的第三方依赖

## 开发原则

所有设计遵循以下优先级：

1. 正确性（Correctness）
2. 可读性（Readability）
3. 性能（Performance）

不要为了微小的性能收益而牺牲代码的正确性和可维护性。

当存在多种实现方案时，优先选择更简单、更容易理解的方案；除非经过性能分析（Profiling）证明存在瓶颈，否则不要进行过早优化。

## 编码规范

### 通用规范

- 遵循 `rustfmt` 格式化规范。
- 函数应保持职责单一、长度适中。
- 避免过深的嵌套逻辑。
- 命名应具有明确语义，避免无意义缩写。
- 所有公开 API（Public API）应编写文档注释。
- 所有注释应当使用英文。

### 错误处理

- 优先使用 `Result` 返回错误。
- `src/`、示例及其他运行时代码中禁止使用 `unwrap()`、`expect()` 和 `panic!()`。
- 可恢复错误使用 `Result` 和 `?` 传播；可选值使用 `let ... else`、`match`、
  `ok_or()` / `ok_or_else()` 或安全默认值显式处理。
- 内部不变量优先使用 `debug_assert!()` 记录开发期错误，但面向外部输入、配置、容量和
  ECS 状态的失败必须返回具体错误，不得依赖断言。
- 测试代码允许使用 `unwrap()` / `expect()` 作为明确的成功断言，因为测试失败本就应当
  立即终止；不得将这种写法复制到运行时代码。

### 所有权与内存

- 优先使用 Borrow，而不是 Clone。
- 避免不必要的内存分配。
- 避免无意义的堆内存（Heap）分配。

## 依赖管理

优先使用 Rust 标准库以及 Bevy 官方提供的功能。

除非确实能够带来明显收益，否则不要新增第三方依赖。

如果必须新增依赖，请说明新增原因以及带来的价值。

## Unsafe

除非明确要求，否则不要使用 `unsafe`。

始终优先选择 Safe Rust。

## ECS 设计规范

始终以 ECS 思维进行设计。

优先使用：

- Component
- Resource
- System
- Event / Message

避免使用不符合 ECS 思想的面向对象设计。

各个 System 应尽可能保持独立，减少耦合。

## 性能规范

性能是项目的重要目标。

优先考虑：

- 栈内存（Stack）
- 合适情况下使用固定大小数组
- Cache Friendly（缓存友好）的数据布局
- 连续且可预测的内存访问

避免：

- 过度 Clone
- 不必要的动态分发（Dynamic Dispatch）
- 热路径中的堆内存分配

所有性能优化应建立在实际测试和 Profiling 的基础上。

## 确定性（Determinism）

Gameplay 逻辑应尽可能保持确定性。

避免依赖：

- HashMap 等容器的遍历顺序
- 不同平台可能存在差异的浮点行为
- 隐藏的全局状态

## 序列化

如需序列化功能，优先使用：

- serde
- ron

配置应采用数据驱动方式管理。

除非确有必要，否则不要序列化 Trait Object。

## 日志

如需日志，统一使用 `tracing`。

运行时代码中不要使用 `println!()` 输出日志。

## 测试

完成开发后，应至少执行以下检查：

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
```

尽量保证代码无编译警告，并通过所有测试。

## 文档规范

所有公开 API 应说明：

- 功能
- 参数
- 返回值

复杂算法应补充设计思路或实现说明，便于后续维护。

## AI 协作规范

当 AI 修改代码时，应遵循以下原则：

- 保持现有项目架构不变。
- 保持已有代码风格一致。
- 不进行无关的重构。
- 尽量缩小修改范围。
- 对重要设计决策进行说明。
- 未经要求，不主动修改公共 API 名称。
- **修改代码后，应及时更新 `docs/` 知识库中的相关内容**，尽量使用中文，确保文档与代码保持一致。

如果需求存在歧义，应先询问，而不是擅自进行架构调整。

## 提交原则

优先进行小而明确的修改。

每次提交（Commit）只解决一个独立问题。

避免将功能开发与代码重构混在同一次修改中。

## 项目价值观

本项目始终坚持以下原则：

- 正确性（Correctness）
- 可维护性（Maintainability）
- ECS 优先
- Gameplay 确定性（Determinism）
- 高性能（Performance）
- 编写符合 Rust 风格的代码
- 尽量减少第三方依赖

## 知识库

项目的详细架构、API 参考、使用模式等文档位于 `docs/` 目录，详见 [docs/README.md](docs/README.md)。

## 编码约定

- **Rust edition 2024** — 使用新语言特性（如 `if let` 链、`use` 重导出、
  `impl Trait` 在关联类型位置等）
- **模块引用路径** — 子模块引用同一功能领域内的父模块或兄弟模块项时使用
  `use super::...`；引用其他功能领域的模块项时使用 `use crate::...`。避免使用
  `crate` 路径绕过当前功能模块边界，也避免用多层 `super::super::...` 跨领域引用
- **文件头统一导入** — 文件中使用的类型和函数应优先通过文件头的 `use` 语句导入，
  避免在函数签名、函数体或字段类型中重复书写 `super::...` 或 `crate::...` 完整路径
- **`pub use` 重导出模式** — 每个模块使用模块文件+同名目录布局，并通过
  `pub use submodule::*` 重导出其公开项
- **Component/Resource 为中心** — 游戏状态存储在 Bevy Component 和 Resource
  中，而非独立的 world 存储
- **SystemParam 作为公共 API** — 函数如 `apply_gameplay_effect()` 接收
  `&mut AbilitySystemParams` 而非单独的查询
- **Arc\<GameplayEffect\>/Arc\<GameplayAbility\>** — 效果和技能定义通过 `Arc`
  共享；规格通过 `Arc::ptr_eq` 比较
- **脏标记模式** — `Attribute` 和 `AttributeSet` 都有 `dirty: bool`，
  `recalculate_attribute_sets_system` 使用 `Changed<AttributeSet>` 过滤
- **tick 计时，非秒** — 所有时间（持续时间、周期、任务等待）以 `FixedUpdate` tick
  为单位
- **标签引用计数** — `GameplayTagContainer` 追踪每个位被设置的次数，
  避免重叠的效果授予/移除互相干扰
- **`debug_assert!`** 用于内部不变量；超出容量时返回 `Err` 而非直接 panic
  （`GameplayTagError`、`AttributeIdError`）
- **队列模式** — 技能激活和效果应用支持通过队列延迟到下一 tick 批量执行，
  避免在同一帧内递归执行导致的借用问题
