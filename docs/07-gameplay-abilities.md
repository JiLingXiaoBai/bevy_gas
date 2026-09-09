# 07 — Gameplay 技能

## 职责

Gameplay Ability 描述角色能够执行的动作，例如攻击、法术和冲刺。该领域拥有技能定义、
已授予规格、完整激活数据、激活上下文、技能链、活跃实例与任务的推进和完成动作；激活、Commit 与清理由
`ability_system` 编排，跨类型请求顺序由 `gameplay_execution` 维护。

## 源码布局

```text
src/gas/
├── gameplay_abilities.rs
└── gameplay_abilities/
    ├── gameplay_ability.rs
    ├── gameplay_ability_spec.rs
    ├── activation_data.rs
    ├── activation_context.rs
    ├── ability_chain.rs
    ├── active_gameplay_ability.rs
    ├── active_gameplay_ability/
    │   └── state.rs
    ├── ability_task.rs
    └── ability_task/
        ├── context.rs
        ├── definition.rs
        ├── state.rs
        ├── completion.rs
        └── ticking.rs
```

| 文件 | 职责 |
| ---- | ---- |
| `gameplay_ability.rs` | 不可变技能定义与 `AbilityTags` |
| `gameplay_ability_spec.rs` | 某个 ASC 已授予技能的等级和活跃计数 |
| `activation_data.rs` | 一次激活共享的 source、targets 与传播 context 不可变值 |
| `ability_chain.rs` | 链 ID、深度限制和重复 Handle 检查 |
| `activation_context.rs` | Instigator、Causer、来源快照、激活原因和 Ability → Effect payload 转换 |
| `active_gameplay_ability/state.rs` | 活跃实例 Component、Handle 别名和状态 |
| `ability_task/` | startup 定义与跨 tick 运行时任务，详见 [08 — 技能任务](./08-ability-tasks.md) |

模块文件使用显式重导出；调用方应从 `gameplay_abilities`、GAS 聚合门面或 crate 根导入公共
类型，不依赖私有子模块路径。`AbilityActivationData` 是组合型 API，不进入精简 prelude。

## 技能定义与已授予规格

### `GameplayAbility`

```rust
pub struct GameplayAbility {
    ability_tags: AbilityTags,
    startup_tasks: Vec<AbilityTaskDef>,
    cooldown: Option<Arc<GameplayEffect>>,
    cost: Option<Arc<GameplayEffect>>,
    activation_effects: Vec<Arc<GameplayEffect>>,
    end_on_activation: bool,
    allow_multiple_instances: bool,
}
```

通过 `GameplayAbility::default().with_*()` 具名配置，或通过保留的 `GameplayAbility::new(...)`
创建定义，再通过 Getter 只读访问。定义通常放入 `Arc`，同一份定义可被多个
`GameplayAbilitySpec` 共享。

默认定义没有标签、任务、消耗、冷却或激活效果；`end_on_activation` 和 `allow_multiple_instances`
均为 `false`。因此默认激活会保持 Active，必须使用 `EndAbility` 任务、生命周期 API，或显式设置
`with_end_on_activation(true)` 来结束。链式方法消费并返回定义；列表配置替换原列表，不追加。

```rust
let ability = GameplayAbility::default()
    .with_tags(AbilityTags::default().with_activation_blocked_tags(vec![stun_tag]))
    .with_cost(cost_effect)
    .with_cooldown(cooldown_effect)
    .with_startup_tasks(vec![
        AbilityTaskDef::wait_ticks(
            5,
            AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget { effect: damage_effect },
        ),
        AbilityTaskDef::wait_ticks(6, AbilityTaskOnFinishedDef::EndAbility),
    ]);
```

这里的效果和标签是调用方先前创建的值，完整注册与调用见
[`ability_effect_flow`](../examples/ability_effect_flow.rs)。激活自动支付 cost/cooldown；
`with_activation_effects(...)` 配置的效果在 startup tasks 之前立即作用于捕获目标，不会等待前摇。
多个等待任务从同一激活时刻开始计时，结束 Ability 不自动移除已应用的 Effect。

### `AbilityTags`

可使用 `AbilityTags::default()` 配合 `with_ability_asset_tags`、`with_cancel_abilities_with_tags`、
`with_block_abilities_with_tags`、`with_activation_required_tags` 和 `with_activation_blocked_tags`
按需配置，避免在 `new(...)` 的五个同类型位置参数间辨认含义；原构造入口继续可用。

| 字段 | 语义 |
| ---- | ---- |
| `ability_asset_tags` | 技能身份；供其他技能按标签取消 |
| `cancel_abilities_with_tags` | 激活成功前需要取消的活跃技能身份标签 |
| `block_abilities_with_tags` | 技能活跃期间写入来源 ASC 的阻止标签 |
| `activation_required_tags` | 来源有 `GameplayTagContainer` 时必须全部拥有 |
| `activation_blocked_tags` | 来源有 `GameplayTagContainer` 时必须全部不拥有 |

ASC 的阻止标签与实体自己的 `GameplayTagContainer` 是两套状态：前者只服务技能互斥，后者
用于 Gameplay Tag、Cooldown granted tag 和 Requirement。来源没有标签容器时，required、
blocked 与 cooldown tag 检查会跳过；需要这些能力的实体应使用
`GameplayAbilitySystemBundle` 或显式附加标签容器。

### `GameplayAbilitySpec` 与 `AbilitySpecHandle`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbilitySpecHandle(u32);

pub struct GameplayAbilitySpec {
    handle: AbilitySpecHandle,
    ability: Arc<GameplayAbility>,
    level: u32,
    active_count: u32,
}
```

`GameplayAbilitySpec::new(handle, ability, level)` 只接收授予 Handle、共享定义和等级。
`active_count` 用于多实例检查；仍有活跃实例时，ASC 不允许清除对应规格。规格不保存输入编号或
按下状态；逻辑动作与 Handle 的映射由独立的
[`AbilityInputBindings<Action>`](./18-ability-input-bindings.md) Component 管理。

## 激活数据、上下文与技能链

### `AbilityActivationData`

```rust
pub struct AbilityActivationData {
    source: Entity,
    targets: AbilityActivationTargets,
    context: AbilityActivationContext,
}
```

该不可变值是一次激活中 `source + targets + context` 的唯一组合边界：Request 和 Active Ability
不再各自重复声明这三个字段。`AbilityActivationData::new(source, targets, context)` 接收
`impl Into<AbilityActivationTargets>`；`get_source()`、`get_targets()`、兼容的 `get_target()` 与
`get_context()` 提供只读访问。

`AbilityActivationRequest` 在排队期间拥有该值；创建长期运行的 `ActiveGameplayAbility` 时克隆
整份数据。两种容器的旧 `new(...)` 与字段级 getter 保留并委托给该值，新代码在已有完整数据时
可使用 `from_data(...)` 和 `get_activation_data()`，避免拆开再组装。

该类型由 `gameplay_abilities` 领域门面、GAS 聚合门面和 crate root 显式重导出，但不会加入
精简 prelude。

### `AbilityActivationContext`

```rust
pub struct AbilityActivationContext {
    chain: Option<AbilityChainContext>,
    instigator: Entity,
    causer: Option<Entity>,
    source_snapshot: Option<AttributeSetSnapshot>,
    reason: AbilityActivationReason,
}
```

公共构造与变换 API：

| API | 作用 |
| --- | ---- |
| `AbilityActivationContext::direct(source, chain)` | 创建 `Direct` 上下文，默认 Instigator 为 `source` |
| `AbilityActivationContext::input(source, chain)` | 创建 `Input` 上下文，默认 Instigator 为 `source` |
| `with_instigator(entity)` | 指定实际发起者 |
| `with_causer(Option<Entity>)` | 指定直接造成行为的物理实体 |
| `with_source_snapshot(snapshot)` | 固定来源属性快照 |
| `child_for_chained_ability(parent, handle)` | 继承上下文并推进技能链，原因改为 `Chained` |

Cost、Cooldown、activation effects 和 Ability Task 完成动作都通过同一个 crate 内部转换
函数构造 `EffectPayload`。`source` 和 `level` 由当前执行路径传入，`instigator`、`causer`
与来源快照在存在 `AbilityActivationContext` 时从中继承；独立 `commit_ability()` 没有激活
上下文，因此使用默认 instigator 且不带 causer/快照。该函数是内部一致性边界，不是
公共 API。
`source` 和目标不属于传播用的 `AbilityActivationContext`，而由
`AbilityActivationData` 与 Context 组合。一次激活只携带一个
`AbilityActivationTargets`：`single(entity)` 表示单目标，`acquired(target_data)` 表示抓取得到的
有序多目标；后者在 Target Data 为空时返回 `AbilityActivationTargetsError::EmptyTargetData`。
Request 与 Active Ability 都通过 `AbilityActivationData` 持有该值，startup/task 完成动作借用
它，链式激活则用父数据中的 source、targets 与派生 Context 组装新的激活数据；不再同时维护
独立 `target: Entity` 与 Context 内 Target Data 两份可能冲突的状态。

消耗与冷却仍应用到技能 `source`。`activation_effects` 统一按
`AbilityActivationTargets::entities()` 的确定顺序逐个应用：single 产生一个实体，acquired 按
Target Data 的 Hit 顺序产生实体。

公共构造路径可产生 `Direct` 或 `Input`，链式 API 产生 `Chained`；`TaskEvent` 和
`GameplayEffect` 原因目前没有公共 Setter 或专用构造器。`AbilityActivationReason::Input`
只标记激活来源，不携带输入编号；具体动作、按下/释放和长按状态仍属于游戏输入层。

### `AbilityChainContext`

```rust
impl AbilityChainContext {
    pub const MAX_DEPTH: u8;

    pub fn root(handle: AbilitySpecHandle, chain_id: u64) -> Self;
    pub fn next(&self, handle: AbilitySpecHandle) -> Result<Self, AbilityChainError>;
    pub fn validate_for_handle(&self, handle: AbilitySpecHandle)
        -> Result<(), AbilityChainError>;
}
```

`GameplayExecutionQueue::new_root_chain(handle)` 是常规根链入口，它按入队顺序分配队列局部
chain ID。`next()` 拒绝重复 Handle，并限制深度不超过
`GameplayAbilitySystemSettings::ABILITY_CHAIN_MAX_DEPTH`。消费激活请求时还会用
`validate_for_handle()` 检查上下文末尾 Handle 与请求 Handle 是否一致。

## 两种激活入口

### 推荐：统一执行队列

运行时生产系统应位于 `GameplayAbilitySystemSet::RequestProducers`，并写入统一队列：

```rust
let chain = execution_queue.new_root_chain(handle);
let context = AbilityActivationContext::direct(source, chain);
let targets = AbilityActivationTargets::single(target);
execution_queue.push_activation(source, targets, handle, context);
```

请求会在当前 `FixedUpdate` 的 `GameplayResolve` 阶段与效果应用请求按跨类型 FIFO 结算。详细
阶段契约见 [16 — Gameplay 执行模块](./16-gameplay-execution.md)。

### 独立同步入口

```rust
pub fn try_activate_ability_by_handle(
    source: Entity,
    targets: impl Into<AbilityActivationTargets>,
    handle: AbilitySpecHandle,
    activation_context: AbilityActivationContext,
    params: &mut AbilitySystemParams,
) -> Result<(), AbilityActivationError>;
```

该入口先把参数归一为 `AbilityActivationRequest`，收敛 Active Effect Requirement，再使用一个
局部 `GameplayExecutionQueue` 执行根激活，并在返回前 drain 根激活的 startup `Instant`
派生请求。它不会查看或消费全局队列，所以不要在同一逻辑阶段把它与尚未消费的全局请求混用。
派生请求失败由 resolver 记录日志，不会改写已经成功的根激活返回值。

“同步”只表示这条逻辑路径在返回前完成本地结算，不表示 `Commands` 已 flush，也不表示事务式
回滚。

## 激活关键流程

根激活按以下顺序执行：

1. 验证可选技能链上下文与请求 Handle。
2. 查询来源 ASC 和已授予规格，检查多实例限制。
3. 检查 ASC 阻止标签、来源 required/blocked tag 和 cooldown granted tag。
4. 准备 Cost 与 Cooldown Effect Plan，并确认 Cost 可支付。
5. 预验证本技能需要写入的阻止标签。
6. 取消身份标签匹配 `cancel_abilities_with_tags` 的活跃技能。
7. 写入阻止标签、递增 `active_count`，通过 `Commands` 创建 `ActiveGameplayAbility`，同时写入
   pending overlay。
8. 执行已准备的 Cost 与 Cooldown；失败时回滚新技能的启动 bookkeeping。
9. 收敛 Commit 产生的 Requirement 变化。
10. 尽力应用 `activation_effects`；单个目标或效果拒绝不会使根激活失败。
11. 按定义顺序启动 startup tasks；`Instant` 直接派发，`WaitTicks` 创建任务实体。
12. startup `EndAbility` 或 `end_on_activation` 将实例标记为 `Ending`。

验证与 Plan 准备失败发生在取消旧技能之前。进入取消阶段后，后续启动或 Commit 失败不会恢复
已经取消的旧技能；Commit 也不提供数据库式的全部副作用回滚。

`AbilityActivationError::is_rejection()` 将正常玩法拒绝与结构性错误区分开：多实例限制、激活
条件和可恢复的 Cost/Cooldown 拒绝属于 rejection；缺少 ASC/规格、无效链、标签容量和执行错误
属于运行时或配置错误。

## 活跃实例与生命周期

```rust
pub type ActiveAbilityHandle = Entity;

#[derive(Component, Clone)]
pub struct ActiveGameplayAbility {
    spec_handle: AbilitySpecHandle,
    activation_data: AbilityActivationData,
    status: AbilityActivationStatus,
}
```

`ActiveGameplayAbility::from_data(spec_handle, activation_data, status)` 接收已经捕获的数据；旧
`new(source, spec_handle, targets, status, activation_context)` 保留为兼容便捷入口。
`get_activation_data()` 返回完整值，`get_source()`、`get_targets()`、`get_target()` 和
`get_activation_context()` 均委托给它。需要应用范围效果时使用 targets 的 `entities()`，避免
在不同执行路径重新解释目标。

```text
Active ──► Ending ──► Cleanup/despawn
   └────► Cancelled ──► Cleanup/despawn
```

`end_ability()` 与 `cancel_ability()` 只在 Handle 存在且实例来源匹配时更新状态并返回 `true`。
默认运行时随后在 `Cleanup` 阶段递减规格活跃计数、移除阻止标签，并递归销毁活跃实例及其任务
子实体。

技能激活时由 `cancel_abilities_with_tags` 触发的取消是内部批处理路径：它会在同一 batch 更新
状态和 ASC bookkeeping，并用 pending overlay 处理尚未 flush 的实例，避免同一实例被重复清理。

## FixedUpdate 边界

- 在 `AbilityTasks`、`RequestProducers` 或 `Targeting` 阶段产生的 Gameplay 请求可由当前 tick 的
  `GameplayResolve` 消费。
- `GameplayResolve` 正在 drain 时，startup `Instant` 追加的效果或链式激活继续由当前 drain
  消费。
- `GameplayResolve` 创建的 startup `WaitTicks` 任务错过了本 tick 的 `AbilityTasks` 阶段，最早
  在下一次 `FixedUpdate` 推进。
- 在 `GameplayResolve` 返回后才写入统一队列的请求留到下一次 `FixedUpdate`。

## 测试导航

| 测试文件 | 覆盖范围 |
| -------- | -------- |
| `tests/gas_test/abilities_test/activation_test.rs` | 多实例、required/blocked tag、阻止标签和缺失规格 |
| `tests/gas_test/abilities_test/commit_test.rs` | Cost、Cooldown、准备失败与 activation effect 容错 |
| `tests/gas_test/abilities_test/chaining_test.rs` | 循环/深度保护、上下文继承和 pending 父技能取消 |
| `tests/gas_test/abilities_test/lifecycle_test.rs` | 标签取消、活跃计数、清除规格和清理幂等性 |
| `tests/gas_test/runtime_paths_test.rs` | 插件阶段、startup `Instant` 与 Bundle 组合 |
| `tests/gas_test/gameplay_targeting_test.rs` | Target Data 与多目标 activation effects |

继续阅读：[08 — 技能任务](./08-ability-tasks.md)、
[09 — 技能系统组件](./09-ability-system-component.md)、
[15 — Gameplay 目标抓取](./15-gameplay-targeting.md)。
