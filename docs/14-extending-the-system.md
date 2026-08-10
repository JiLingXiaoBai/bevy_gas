# 14 — 扩展系统

## 先确定所有权边界

新增代码前先选择拥有它的领域：Tag、Attribute、Modifier、Effect、Ability、Targeting、
Execution 或 Ability System。实现文件保持私有，由同名领域门面显式公开；不要把新类型放进
全局 `components/`、`systems/` 或 `utils/`。

公开 API 的固定检查顺序：

1. 在 owning domain facade 中使用 `pub use submodule::{Type, function}` 显式重导出；
2. 确认是否需要从 `src/gas.rs` 的 GAS 聚合门面公开；
3. 只有确实需要保持/建立 crate-root 路径时，才更新 `src/lib.rs` 的显式兼容重导出；
4. 只有高频、低歧义、能明显改善常见用法的项才进入 `src/gas/prelude.rs`；
5. 不使用 `pub use *`，避免内部 `pub` 项自动扩大 API；
6. 同步更新 rustdoc、对应知识库章节和公共路径测试。

`bevy_tools::gas::<domain>` 是专项 API 的规范所有者，`bevy_tools::gas` 是显式聚合门面，
crate root 主要承担兼容路径。`Random` 与 `UniqueName*` 是例外：它们由 crate root 直接公开，
不属于 GAS prelude。

## 新增 Modifier 操作

1. 在 `src/gas/modifiers/definition.rs` 的 `ModifierOperation` 添加变体；
2. 在 `src/gas/attributes/aggregation.rs` 的 `Aggregator::apply_modifier_spec()` 添加对应存储；
3. 更新 `remove_modifiers_by_source()`、`reset()` 与 `modifier_count()`，保持清理和计数对称；
4. 在 `default_executor()` 中明确该操作相对 Override/Add/PercentAdd/Multiply 的求值顺序；
5. 在 `src/gas/attributes/attribute_set/state.rs` 的即时 base 修改逻辑中处理新操作；
6. 添加 Aggregator 纯行为测试，以及 Instant/Duration Effect 集成测试；
7. 更新 [05 — 修饰器与聚合器](./05-modifiers-and-aggregator.md)。

Modifier 是独立共享领域：Effect 负责把定义求值为 `ModifierSpec`，Attribute 只消费 spec 与
`ModifierSourceId`。不得让 `modifiers/` 反向依赖 Active Effect runtime 类型。

## 扩展动态幅度上下文

自定义计算实现 `ModifierMagnitudeCalculation`，并只读取 `ModifierEvaluationContext`。若需要
新增上下文数据：

1. 优先添加具有 Gameplay 语义的只读方法，不暴露 Bevy `Query`、完整 ASC 或 Effect 容器；
2. 在 `src/gas/gameplay_effects/gameplay_effect/context.rs` 的 `EffectContext` 实现该方法；
3. 评估 trait 方法对下游实现者的兼容影响；能提供安全语义时优先增加默认实现；
4. 添加自定义 calculation 测试，覆盖数据存在与缺失时的明确 fallback；
5. 不把 `EffectContext` 重新写回 `ModifierMagnitudeCalculation::calculate()` 签名。

## 新增堆叠策略变体

1. 在 `src/gas/gameplay_effects/gameplay_effect/stacking.rs` 的对应 `Stack*` 枚举添加变体；
2. 在 `src/gas/gameplay_effects/active_gameplay_effect/application.rs` 的
   `find_stackable_active_effect()` 更新匹配决策；
3. 在 `src/gas/gameplay_effects/active_gameplay_effect/execution.rs` 的
   `execute_stack_existing_effect()` 更新执行逻辑；
4. 检查 Duration、Period、Magnitude、Overflow 与 Expiration 策略的组合语义；
5. 在 `tests/gas_test/effects_test/stacking_test.rs` 添加成功、拒绝、刷新与回滚边界测试；
6. 显式更新拥有该枚举的 Gameplay Effects 门面以及需要的聚合/根重导出。

不要依赖 `HashMap` 遍历顺序选择堆叠目标；继续使用稳定 handle/slot 语义。

## 注册新的 Gameplay 属性

新增游戏属性通常不需要修改 library enum：

1. 在初始化系统中调用
   `AttributeIdRegister::request_or_register_attribute_id(name, AttributeRegion::Hot/Cold)`；
2. 同一名称在所有注册点必须选择相同区域；
3. 从 `AttributeIdManager` Resource 取得注册表；
4. 在实体的 `AttributeSet` 上调用
   `initialize_attribute(&manager, id, base_value, executor)`；
5. 保存返回的普通 `AttributeId`，所有访问继续通过同一个 manager 定位冷热槽位；
6. 高频且广泛读取的值才放入 Hot，其他值优先 Cold；调整编译期容量前必须运行完整测试。

只需要属性的实体可以单独附加 `AttributeSet`。需要完整 ASC/Tags/Attributes/Active Effects 的
实体使用 `GameplayAbilitySystemBundle`，不要恢复 Attribute 到 Effect 的 Required Component
反向依赖。

## 注册新的 Gameplay Tag

1. 通过 `GameplayTagRegister::request_or_register_tag("Parent.Child")` 注册；
2. 父标签由 registry 递归注册，并预计算继承位集；
3. 保存返回的 `GameplayTag`，不要手工构造索引；
4. 为容量错误、父子继承与引用计数添加测试；
5. 纯 Tag 实体只需 `GameplayTagContainer`，不要恢复对 `ActiveGameplayEffects` 的
   Required Component。

## 新增内置运行时系统

只有 library 内建且必须参与 GAS 固定管线的系统，才加入
`GameplayAbilitySystemRuntimePlugin::build()`：

1. 将实现放在 owning domain；
2. 从领域门面只公开确实属于用户 API 的 system；
3. 在 `src/gas/runtime_plugin.rs` 注册到语义正确的 `GameplayAbilitySystemSet`；
4. 使用 `.before()` / `.after()` 或 `.chain()` 明确跨系统可见性；
5. 只有运行条件不改变 Gameplay 语义时才添加 `run_if`；
6. 添加运行完整 `FixedUpdate` 的集成测试，而不仅是 `RunSystemOnce`。

游戏项目自身的生产系统无需修改 Runtime Plugin，直接注册到公共 SystemSet 即可。

### 生产 Gameplay 请求

调用 `GameplayExecutionQueue::push_activation()` 或 `push_application()` 的生产系统，应进入
`GameplayAbilitySystemSet::RequestProducers`：

```rust
app.add_systems(
    FixedUpdate,
    produce_ai_ability_requests.in_set(GameplayAbilitySystemSet::RequestProducers),
);
```

`RequestProducers` 和后续 `Targeting` 阶段产生的请求会在同一个 `FixedUpdate` 的
`GameplayResolve` 消费；resolver drain 期间追加的派生请求也在本次完整 drain 中处理。
`GameplayResolve` 之后才入队的请求保留到下一 tick。

多个生产系统的入队顺序具有玩法语义时，必须通过 `.chain()`、`.before()` 或 `.after()`
排序。同处一个 SystemSet 只提供阶段边界，不保证并行系统之间的 FIFO 插入顺序。

## 新增 Resource 或 SystemParam

新增 Resource 时：

1. 明确由基础插件、Runtime Plugin 还是游戏应用拥有；
2. 在对应 Plugin 中 `init_resource`，并记录单独安装插件时的依赖；
3. 内部收敛状态保持 crate-private；若因公共 Bevy system 签名必须公开，使用
   `#[doc(hidden)]` 并避免把它宣传成 Gameplay API；
4. 不把可由 Component 表达的每实体状态放进全局 Resource。

新增 SystemParam 时按最窄职责组合访问：

- Effect 准备、应用、移除和 Requirement 使用 `EffectSystemParams`；
- Ability 激活、commit、生命周期和 `Commands` 使用 `AbilitySystemParams`；
- 请求生产系统通常只声明 `ResMut<GameplayExecutionQueue>`；
- 不要为了调用一个 Effect API 就让 Effect 模块重新依赖完整 ASC/Ability 查询。

## 新增 AbilityTask 类型

1. 在 `src/gas/gameplay_abilities/ability_task/definition.rs` 的 `AbilityTaskDef` 添加定义变体；
2. 若任务跨 tick，在 `src/gas/gameplay_abilities/ability_task/state.rs` 的 `AbilityTaskKind`
   添加运行时状态，并更新接收 `AbilityTaskExecutionContext` 的
   `AbilityTaskDef::instantiate()`；
3. 在 `src/gas/ability_system/activation/startup.rs` 的 `start_startup_ability_tasks()` 明确它是
   startup 内立即完成，还是生成任务实体后由后续 tick 推进；
4. 如需完成动作，在 `AbilityTaskOnFinishedDef` 与 `AbilityTaskOnFinished` 添加对应变体，
   并更新实例化；运行时变体只保存动作专属数据，不要重复 source、targets、
   spec handle 或 level；
5. 在 `src/gas/gameplay_abilities/ability_task/completion.rs` 的
   `dispatch_ability_task_completion()` 实现完成分派；从单独传入的 execution context 读取
   source/spec/level，从父 Active Ability 的 `AbilityActivationData` 借用唯一
   `AbilityActivationTargets`；
6. 保持 `src/gas/gameplay_abilities/ability_task/ticking.rs` 的
   `tick_ability_tasks_system()` 只负责稳定顺序推进与调用 completion；
7. 分别添加 startup 与 runtime tick 路径测试，验证执行 tick、FIFO 顺序和结束/取消语义；
8. 只从 `src/gas/gameplay_abilities/ability_task.rs` 与 `src/gas/gameplay_abilities.rs` 门面
   显式公开稳定用户类型。

## 完成前检查

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
```

还应扫描：

- 源码是否新增 `pub use *`；
- Effect-only API 是否误用 `AbilitySystemParams`；
- Tag/Attribute 是否重新反向 require Active Effects；
- 新公开项是否同时更新 owning facade、聚合门面、兼容根路径与必要的 prelude；
- `docs/`、测试路径和 example 是否仍指向已移动文件。

## 设计约束

- **正确性优先**：可恢复失败返回 `Result`，内部不变量使用 `debug_assert!`；
- **运行时代码无 panic 路径**：禁止 `panic!()`、`unwrap()` 和 `expect()`；
- **ECS 优先**：Component、Resource、System、Event/Message 保持职责清晰；
- **最小依赖**：新增第三方 crate 必须说明不可替代的价值；
- **确定性**：避免未排序的哈希遍历、未排序的并行入队和隐藏全局状态；
- **Safe Rust**：除非用户明确要求且充分论证，不使用 `unsafe`。
