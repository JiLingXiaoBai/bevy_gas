# 07 — Gameplay 技能

## 职责

Gameplay Ability 描述角色能够执行的动作，例如攻击、法术和冲刺。该领域只拥有技能定义、
已授予规格、激活上下文、技能链、活跃实例和任务数据；激活、Commit 与清理由
`ability_system` 编排，跨类型请求顺序由 `gameplay_execution` 维护。

## 源码布局

```text
src/gas/
├── gameplay_abilities.rs
└── gameplay_abilities/
    ├── gameplay_ability.rs
    ├── gameplay_ability_spec.rs
    ├── active_gameplay_ability.rs
    ├── active_gameplay_ability/
    │   ├── chain.rs
    │   ├── context.rs
    │   └── state.rs
    ├── ability_task.rs
    └── ability_task/
        ├── definition.rs
        ├── state.rs
        ├── completion.rs
        └── ticking.rs
```

| 文件 | 职责 |
| ---- | ---- |
| `gameplay_ability.rs` | 不可变技能定义与 `AbilityTags` |
| `gameplay_ability_spec.rs` | 某个 ASC 已授予技能的等级、输入状态和活跃计数 |
| `active_gameplay_ability/chain.rs` | 链 ID、深度限制和重复 Handle 检查 |
| `active_gameplay_ability/context.rs` | Instigator、Causer、来源快照、Target Data 和激活原因 |
| `active_gameplay_ability/state.rs` | 活跃实例 Component、Handle 别名和状态 |
| `ability_task/` | startup 定义与跨 tick 运行时任务，详见 [08 — 技能任务](./08-ability-tasks.md) |

模块文件使用显式重导出；调用方应从 `gameplay_abilities`、`gas::prelude` 或 crate 根导入公共
类型，不依赖私有子模块路径。

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

通过 `GameplayAbility::new(...)` 创建定义，并通过 Getter 只读访问。定义通常放入 `Arc`，同一
份定义可被多个 `GameplayAbilitySpec` 共享。

### `AbilityTags`

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
    input_id: Option<u16>,
    input_pressed: bool,
    active_count: u32,
}
```

`input_pressed` 默认为 `false`，只记录外部输入系统写入的瞬时状态，不会自动激活技能。
`active_count` 用于多实例检查；仍有活跃实例时，ASC 不允许清除对应规格。

## 激活上下文与技能链

### `AbilityActivationContext`

```rust
pub struct AbilityActivationContext {
    chain: Option<AbilityChainContext>,
    instigator: Entity,
    causer: Option<Entity>,
    source_snapshot: Option<AttributeSetSnapshot>,
    target_data: Option<AbilityTargetData>,
    reason: AbilityActivationReason,
}
```

公共构造与变换 API：

| API | 作用 |
| --- | ---- |
| `AbilityActivationContext::direct(source, chain)` | 创建 `Direct` 上下文，默认 Instigator 为 `source` |
| `with_instigator(entity)` | 指定实际发起者 |
| `with_causer(Option<Entity>)` | 指定直接造成行为的物理实体 |
| `with_source_snapshot(snapshot)` | 固定来源属性快照 |
| `with_target_data(data)` | 附加确定有序的目标集合 |
| `child_for_chained_ability(parent, handle)` | 继承上下文并推进技能链，原因改为 `Chained` |

Cost、Cooldown、activation effects 和 Ability Task 完成动作都通过同一个 crate 内部转换
函数构造 `EffectPayload`。`source` 和 `level` 由当前执行路径传入，`instigator`、`causer`
与来源快照在存在 `AbilityActivationContext` 时从中继承；独立 `commit_ability()` 没有激活
上下文，因此使用默认 instigator 且不带 causer/快照。该函数是内部一致性边界，不是
公共 API。
消耗与冷却仍应用到技能 `source`。Target Data 存在时，`activation_effects` 会按“效果定义
顺序，再按 Target Data 实体顺序”逐个应用；旧 `target: Entity` 继续表示首要目标。

`TargetingContinuation::ActivateAbility` 会把旧 `target` 设为 `primary_entity()`。直接调用
`GameplayExecutionQueue::push_activation()` 不验证二者一致性，调用方需要自行保持一致。

当前公共构造路径产生 `Direct`，链式 API 产生 `Chained`；`Input`、`TaskEvent` 和
`GameplayEffect` 原因目前没有公共 Setter 或专用构造器。

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
execution_queue.push_activation(source, target, handle, context);
```

请求会在当前 `FixedUpdate` 的 `GameplayResolve` 阶段与效果应用请求按跨类型 FIFO 结算。详细
阶段契约见 [16 — Gameplay 执行模块](./16-gameplay-execution.md)。

### 独立同步入口

```rust
pub fn try_activate_ability_by_handle(
    source: Entity,
    target: Entity,
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
    source: Entity,
    spec_handle: AbilitySpecHandle,
    target: Entity,
    status: AbilityActivationStatus,
    activation_context: AbilityActivationContext,
}
```

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
