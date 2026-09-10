# 13 — 测试指南

## 测试与示例组织

```text
tests/
├── gas_test.rs                         # GAS 集成测试 crate 门面
├── gas_test/
│   ├── ability_input_test.rs            # Logical input bindings and fixed-tick buffering
│   ├── support_test.rs                       # App、builder、tick 和查询 helper
│   ├── effects_test.rs                       # Effect 行为测试门面
│   ├── effects_test/
│   │   ├── application_test.rs               # 应用校验、错误与执行边界
│   │   ├── stacking_test.rs                  # 堆叠、上限与刷新策略
│   │   ├── requirements_test.rs              # 免疫、抑制与固定点收敛
│   │   ├── ticking_test.rs                   # Duration 与 Period tick
│   │   └── removal_test.rs                   # 显式与标签驱动移除
│   ├── abilities_test.rs                     # Ability 行为测试门面
│   ├── abilities_test/
│   │   ├── activation_test.rs                # 激活条件与实例策略
│   │   ├── commit_test.rs                    # Cost 与 Cooldown
│   │   ├── lifecycle_test.rs                 # 取消、结束与清理
│   │   ├── tasks_test.rs                     # WaitTicks 与任务结束
│   │   └── chaining_test.rs                  # 链上下文、深度与循环限制
│   ├── attributes_test.rs               # 属性注册、冷热槽位、重算与聚合
│   ├── gameplay_tags_test.rs            # 标签注册、位集与引用计数
│   ├── gameplay_targeting_test.rs       # 目标管线、确定性与技能集成
│   ├── queues_test.rs                   # 跨类型 FIFO、运行条件与批次语义
│   └── runtime_paths_test.rs            # Plugin 管线、Bundle 与公共运行路径
├── randoms_test.rs                     # RNG 种子确定性与概率边界
└── unique_names_test.rs                # 驻留复用与名称区分

examples/
├── ability_input_bindings.rs           # Buffered input and slot rebinding without a window
├── ability_effect_flow.rs              # 无窗口的完整技能、伤害与冷却时间线
└── tag_registration.rs                 # 完整 App 中的标签注册
```

集成测试按 crate 顶层功能领域归属组织。`src/gas/` 内功能的集成测试统一放在
`tests/gas_test/`，由 `tests/gas_test.rs` 声明和加载；即使功能可选或只有一个测试文件，
也不单独创建顶层测试目标。例如，输入绑定属于 GAS，其测试位于
`tests/gas_test/ability_input_test.rs`。`randoms_test.rs` 和 `unique_names_test.rs` 则分别
对应 crate 顶层的 `randoms` 和 `unique_names` 领域，保留独立测试目标。

领域内部的测试按外部行为拆分，不镜像私有实现文件，也不要求叶子测试文件递归套用门面结构。
移动私有函数不应迫使测试目录改名；新增行为时应放入最接近其 Gameplay 语义的模块。
跨领域执行顺序、Bundle 组合和公共导入路径优先放在 `runtime_paths_test.rs` 或
`queues_test.rs`。

### 测试路径命名约定

`tests/` 下的所有文件和子目录都必须以 `_test` 结尾。文件扩展名不参与后缀判断，例如
`queues_test.rs` 合规，而 `queues.rs` 和 `queues_tests.rs` 均不合规。新增或移动测试时，应同步
更新 `#[path = "..."]`、`mod` 声明、模块导入、文档路径以及 `cargo test --test <target>` 中的
集成测试目标名，避免文件路径与 Rust 模块名不一致。

## 运行命令

```bash
# All unit, integration, and doc tests
cargo test

# GAS integration-test crate
cargo test --test gas_test

# Input binding and buffering integration tests
cargo test --test gas_test ability_input_test

# One nested behavior module
cargo test --test gas_test effects_test::requirements_test

# Supporting infrastructure only
cargo test --test randoms_test
cargo test --test unique_names_test

# One test-name filter with captured output visible
cargo test test_name -- --nocapture

# List discoverable tests before choosing a filter
cargo test --test gas_test -- --list

# Compile or run the checked example
cargo run --example ability_effect_flow
cargo run --example ability_input_bindings
cargo check --example tag_registration
cargo run --example tag_registration
```

`cargo run --example tag_registration` 会启动 Bevy App，适合人工验证；CI 只需编译示例，
不应等待窗口退出。

## 提交前检查

项目最低检查集：

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
```

`cargo clippy --all-targets` 会覆盖 library、测试与 example。只做只读格式检查时可使用
`cargo fmt --check`；如果新增或调整 example，也可以先执行 `cargo check --examples` 获得更快
反馈。

运行时代码禁止 `unwrap()`、`expect()` 和 `panic!()`。集成测试中的 `unwrap()` / `expect()`
可作为“此步骤必须成功”的断言；不要把测试 helper 中的写法复制到 `src/` 或 example。

## 测试 App 与 Fixture

集成测试的通用 App 与当前 `support_test.rs` 保持一致：

```rust
use bevy::prelude::*;
use bevy_gas::prelude::*;

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, GameplayAbilitySystemPlugin));
    app
}
```

注册标签/属性的 helper 使用 `RunSystemOnce` 执行 `GameplayTagRegister` 或
`AttributeIdRegister`。测试可以用 `unwrap()` 明确断言注册成功；生产系统仍必须传播或处理
错误。

Fixture 应按验证目标最小化：

- 纯 Tag 测试只生成 `GameplayTagContainer`；
- 纯 Attribute 测试只生成 `AttributeSet`；
- Duration/Infinite Effect 的目标显式包含 `ActiveGameplayEffects`；
- 完整 Ability/Effect Actor 使用 `GameplayAbilitySystemBundle`；
- 不得依赖插入 Tag 或 Attribute Component 时隐式生成 Active Effect 存储。

当前共享 `spawn_attribute_set` helper 为 Effect 测试方便，同时生成 `AttributeSet` 与
`ActiveGameplayEffects`；纯属性测试不应使用它来证明组件独立性。

Effect-only 测试的新 system 应优先声明 `EffectSystemParams`：

```rust
fn converge_test_effects(mut params: EffectSystemParams) {
    bevy_gas::gas::gameplay_effects::resolve_active_effect_tag_requirements(&mut params);
}
```

`AbilitySystemParams` 因实现了到 `EffectSystemParams` 的 `DerefMut`，现有综合 helper 仍能调用
Effect API；这属于 Ability 编排兼容路径，不应成为新的 Effect-only 测试默认写法。

## 推进 FixedUpdate

所有 Gameplay 时间使用 tick。精确推进一个 tick：

```rust
for _ in 0..tick_count {
    app.world_mut().run_schedule(FixedUpdate);
}
```

不要用 `app.update()` 断言精确 tick 边界，因为它受 Bevy 固定时间累积影响。需要只验证某个
系统的局部行为时，可以通过 `RunSystemOnce` 调用对应公开 system；验证插件排序、请求同 tick
可见性或 Requirement 收敛时，必须运行完整 `FixedUpdate` schedule。

队列测试至少区分三种语义：

1. `RequestProducers` 或 `Targeting` 在 resolver 前入队，请求在当前 tick 消费；
2. resolver drain 期间追加的派生请求仍在本次 drain 消费；
3. `GameplayResolve` 之后入队，请求明确保留到下一 tick。

## 公共 API 路径测试

prelude 是精简入口，不是完整 API 镜像。测试常见用法可导入 `bevy_gas::prelude::*`；错误、
handle/spec、管理器和系统函数应从 `bevy_gas::gas::<domain>` 或 crate root 显式导入。

公开 API 调整时至少验证：

- owning domain facade 的路径可用；
- `bevy_gas::gas` 聚合路径与 crate-root 兼容路径符合设计；
- 只有高频、低歧义项进入 prelude；
- 新增内部 `pub` 项不会因通配重导出意外泄漏；
- `Random` 与 `UniqueName*` 仍由 crate root 公开，而不是误放入 GAS prelude。

`examples/tag_registration.rs` 当前使用 crate-root 兼容导入，验证既有根路径；知识库示例优先
使用精简 prelude，专项 API 则展示领域门面路径。

## 关键测试领域

| 领域 | 应验证的内容 |
| --- | --- |
| 标签注册 | 父标签自动注册、容量、无效句柄与冲突区分 |
| 标签容器 | 引用计数、继承、`has_tag` / `has_all` / `has_any` |
| 属性 | 冷热注册、初始化、dirty 位图、快照与延迟重算 |
| 修饰器 | `Override → Add → PercentAdd → Multiply`、中立 Source ID、计算上下文 |
| 即时效果 | Base 修改、post-execute、无需 Active Effect 存储 |
| 持续/周期效果 | Modifier 生命周期、到期、Period 与 `execute_on_applied` |
| 堆叠 | 来源/目标聚合、上限、幅度、Duration/Period 策略 |
| Requirement/免疫 | 抑制、恢复、移除、固定点收敛与 fail-closed |
| 技能 | 激活、Cost、Cooldown、阻止/取消标签与实例策略 |
| 技能任务/链 | startup、WaitTicks、完成动作、深度与循环检测 |
| Targeting | 管线校验、稳定排序、多目标 continuation |
| 统一 FIFO | Ability/Effect 跨类型顺序、完整 drain、阶段边界 |
| 组件组合 | Bundle 四组件齐全，Tag/Attribute 独立存在 |
| 支撑设施 | RNG 同种子序列、名称复用、空名称与不同字符串区分 |
