# 06 — Gameplay 效果

## 概述

Gameplay 效果是核心的 Buff/Debuff 系统。它们对属性应用修饰器、授予标签，可以是即时、持续或无限的。

源码分为两组门面：`gameplay_effect.rs + gameplay_effect/` 负责定义、上下文、时间、标签和
堆叠策略；`active_gameplay_effect.rs + active_gameplay_effect/` 负责 planning、应用事务、活跃
状态、Requirement 收敛、移除和 tick 系统；`effect_system_params.rs` 定义 Effect 专用的 ECS
访问边界。完整所有权边界见
[17 — 源码布局与维护边界](./17-source-layout-and-maintenance.md)。

## 持续时间类型

| 类型                 | 行为                                   |
| -------------------- | -------------------------------------- |
| `Instant`            | 一次性修饰器应用（修改 base 值）       |
| `DurationTicks(ModifierMagnitude)` | 幅度解析为 N 个 fixed-update tick，之后自动移除 |
| `Infinite`           | 持久存在，直到显式移除                 |

## 效果定义 (`GameplayEffect`)

`GameplayEffect` 是通过 `GameplayEffect::new()` 构造的不可变定义，组合 Modifier、持续时间、
可选周期、应用概率、堆叠策略和 Effect 标签。字段布局属于实现细节；精确构造签名以 rustdoc
为准。

### `EffectDurationTicks`

```rust
pub enum EffectDurationTicks {
    Instant,
    DurationTicks(ModifierMagnitude),  // 支持 Calculated 幅度
    Infinite,
}
```

持续时间和周期幅度解析为 tick 时，正的非整数向上取整，大于等于 `u32::MAX` 的值饱和到
`u32::MAX`，非有限值和小于等于 0 的值解析为 0。`DurationTicks(0)` 会以
`InvalidDuration` 拒绝；对持续/无限效果，period 为 0 不会创建周期计数器，而是按普通持续
modifier 应用，`execute_on_applied` 也不会额外执行。需要周期语义时应显式保证 period 大于 0。

### `EffectPeriodTicks`

为 `DurationTicks` 或 `Infinite` 效果启用周期性修饰器执行（例如每 3 tick 造成一次伤害）。
`Instant` 效果会忽略 period 配置。

使用 `EffectPeriodTicks::new(period_ticks, execute_on_applied)` 构造周期规则；第一个参数支持
Flat 或 Calculated 幅度，第二个参数控制新建正周期 Active Effect 时是否立即 pulse。

`execute_on_applied` 只作用于新建的正周期 Active Effect。重应用并堆叠到已有周期效果时不会
立即 pulse，只会按 stack duration/period policy 更新其运行时状态。

## 效果标签 (`EffectTags`)

`EffectTags` 组合效果身份标签、激活期间授予的标签、来源/目标的 application、ongoing、
removal 条件、免疫查询以及应用前需要移除的效果标签。字段布局属于实现细节；公开 getter 和
构造签名以 rustdoc 为准。

### `TagRequirements`

`TagRequirements` 是 Gameplay Tags 领域的通用条件类型，详细 API 见
[03 — Gameplay 标签](./03-gameplay-tags.md#tagrequirements)。Gameplay Effects 为兼容旧路径
继续重导出该类型。

空的 application/ongoing requirement 会通过检查；空的 removal requirement 被特殊视为
“未配置移除条件”，不会触发移除。`remove_effects_with_tags` 匹配现有效果的 `asset_tags`
而不是该效果授予目标的 `granted_tags`。当前实现会同时展开效果 asset tag 和查询 tag 的父
标签，再检查任意交集；因此两个 sibling tag 只要共享祖先也会匹配。例如查询
`Effect.Buff.Power` 也会命中 `Effect.Damage.Fire`，因为二者都包含 `Effect`。公开
`has_active_effect_with_tags()` 与 `remove_active_effects_with_tags()` 使用相同语义。

### `GameplayEffectImmunityQuery`

```rust
pub struct GameplayEffectImmunityQuery {
    source_tags: TagRequirements,
    effect_tags: TagRequirements,
}

impl GameplayEffectImmunityQuery {
    pub fn matches(
        &self,
        source_tags: Option<&GameplayTagContainer>,
        effect_asset_tags: &[GameplayTag],
        tag_manager: &GameplayTagManager,
    ) -> Result<bool, GameplayTagError>;
}
```

## 堆叠策略 (StackingPolicy)

```rust
pub struct StackingPolicy {
    stacking_type: StackingType,
    stack_limit: u32,
    magnitude_policy: StackMagnitudePolicy,
    duration_policy: StackDurationPolicy,
    period_policy: StackPeriodPolicy,
    overflow_policy: StackOverflowPolicy,
    expiration_policy: StackExpirationPolicy,
}
```

### `StackingType`

| 变体                | 行为                   |
| ------------------- | ---------------------- |
| `None`              | 不堆叠；每次应用独立   |
| `AggregateBySource` | 与同一来源的效果堆叠   |
| `AggregateByTarget` | 与同一目标上的效果堆叠 |

除上表条件外，只有共享同一个 `Arc<GameplayEffect>` 定义实例（`Arc::ptr_eq`）的规格才会互相
堆叠；内容相同但分别构造的两个定义不会合并。`stack_limit == 0` 表示不限制堆叠数。

### 子策略

| 策略                    | 选项                                        | 说明                      |
| ----------------------- | ------------------------------------------- | ------------------------- |
| `StackMagnitudePolicy`  | `None`, `Linear`                            | 幅度如何随堆叠数缩放      |
| `StackDurationPolicy`   | `KeepExisting`, `RefreshOnSuccessfulStack`  | 新堆叠时是否刷新持续时间  |
| `StackPeriodPolicy`     | `KeepCurrentTick`, `ResetOnSuccessfulStack` | 新堆叠时是否重置周期 tick |
| `StackOverflowPolicy`   | `RejectApplication`, `RefreshDuration`      | 达到上限时拒绝，或按原层数重应用 |
| `StackExpirationPolicy` | `RemoveAllStacks`, `RemoveSingleStack`      | 单层过期时的行为          |

### 便捷构造函数

```rust
StackingPolicy::non_stacking()                                          // 不堆叠
StackingPolicy::linear_refreshing(StackingType::AggregateBySource, 5)   // 线性、刷新
```

当前 `RefreshDuration` 在到达上限时只允许以原 `stack_count` 继续执行堆叠路径；是否真的刷新
持续时间仍由 `StackDurationPolicy::RefreshOnSuccessfulStack` 决定，周期计数是否重置则由
`StackPeriodPolicy` 决定。与 `StackDurationPolicy::KeepExisting` 组合时不会刷新持续时间。

## 效果应用流程

### 直接应用

```rust
pub fn apply_gameplay_effect(
    target: Entity,
    effect: &Arc<GameplayEffect>,
    params: &mut EffectSystemParams,
    payload: &EffectPayload,
) -> Result<(), GameplayEffectApplicationError>;
```

该同步入口会在应用前检查当前 Active Effect Requirement 状态，并在返回前完成本次应用
产生的 Tag Requirement 收敛。因此连续同步调用时，后一个效果能稳定看到前一个效果的
堆叠、免疫、移除和抑制结果。它不参与全局 FIFO 排序；运行时生产系统应写入
`GameplayExecutionQueue`，不要在同一逻辑阶段混用排队应用与同步应用。

### 两阶段 API

```rust
// 阶段 1：准备（检查条件、查找可堆叠、收集待移除）
let plan = prepare_gameplay_effect(target, effect, &mut effect_params, &payload)?;

// 阶段 2：执行
execute_gameplay_effect_plan(plan, &mut effect_params)?;
```

`execute_gameplay_effect_plan()` 返回前也会收敛由该计划标记的 Requirement 变化。plan 应在
prepare 后立即、在相同逻辑状态中执行：执行阶段会重验清理条件、目标组件、属性和标签 ID，
但不会重新检查 application requirement、免疫或概率决定。

两阶段 API 不是可长期保存的命令，也不提供数据库式原子事务。执行会先移除 plan 收集的旧
效果，再应用或创建新效果；如果后续因容量或已变化的 ECS 状态失败，不承诺恢复已移除效果。

`GameplayEffectApplicationError` 区分无效概率、概率拒绝、标签条件、免疫、无效持续时间、
缺少目标组件、Active Effect 容量耗尽、目标未初始化属性、堆叠溢出、无效标签和无效属性 ID。
`MissingAttribute { target, id }` 表示目标有 `AttributeSet`，但没有 modifier 所需的属性。
`is_rejection()` 可用于区分正常的 Gameplay 拒绝与配置/状态错误。

FixedUpdate 系统遇到不可恢复的执行错误时会记录一次错误并移除对应 Active Effect，
避免每个 tick 重试同一个永久错误而产生日志风暴。

### `prepare_gameplay_effect` 检查顺序

1. **概率** — 根据 `probability_to_apply` 掷骰
2. **应用条件** — 来源 + 目标标签要求
3. **应用免疫** — 检查目标是否免疫
4. **生成 Spec** — 通过 `EffectContext` 解析幅度
5. **执行条件预检** — 检查目标组件、属性初始化状态、标签和 ID
6. **收集待移除** — 按 `asset_tags` 查找匹配 `remove_effects_with_tags` 的效果
7. **查找可堆叠** — 查找已有的可堆叠活跃效果

### `GameplayEffectApplicationKind`

```rust
enum GameplayEffectApplicationKind {
    Instant,                                          // 应用即完成
    StackExisting { handle: ActiveEffectHandle, new_stack_count: u32 },  // 堆叠到已有
    CreateActive,                                     // 创建新的活跃效果
}
```

## 活跃效果生命周期

### 目标持有的 `ActiveGameplayEffects`

持续/无限效果不再各自生成 Bevy 实体，而是直接存放在目标实体的 Component 中：

```rust
#[derive(Component, Default)]
pub struct ActiveGameplayEffects {
    // 稳定槽位 + 空闲槽位列表
}

pub struct ActiveEffectHandle {
    target: Entity,
    slot: u32,
    generation: u32,
}
```

基础 Component 不再反向要求 `ActiveGameplayEffects`。完整 GAS Actor 应通过
`GameplayAbilitySystemBundle` 显式组合 ASC、Attributes、Tags 和 Active Effects；只使用 Tags
或 Attributes 的实体无需携带效果存储。目标内的槽位顺序稳定；移除后通常递增 generation，
因此旧句柄即使遇到槽位复用也不会误
命中新效果。generation 达到 `u32::MAX` 时该槽位退休、不再复用。句柄携带 target，跨目标
误用同样会返回缺失。

`ActiveGameplayEffects` 的公开接口只提供只读访问；不要直接移除或用 `Default` 替换目标上的
容器。所有 mutation 必须通过效果应用/移除 API，才能同步清理 Modifier 与 Tag 引用计数。

这种存储使同一个 Gameplay FIFO 中较早请求创建的效果，对后续请求的堆叠、免疫和移除
检查立即可见，不依赖 `Commands` 在系统结束时 flush。

### 持续时间 Tick

每个 `FixedUpdate`，`tick_effect_duration_system` 递减剩余 tick。归零时效果被移除（修饰器清理、标签移除）。

抑制不会暂停持续时间：被抑制的有限效果仍会倒计时并可能到期。

Duration 清理后会先执行 Requirement 固定点收敛，再进入 Period Tick。因此某个授予标签
刚好过期时，依赖该标签的周期效果不会额外多执行一次。

### 周期 Tick

`tick_effect_period_system` 追踪每个效果的周期计数器。当计数器达到周期间隔时，以 Instant
方式修改属性 base 值。抑制期间周期计数器暂停；解除抑制后从原计数继续。

`Instant` 效果不会创建 `ActiveGameplayEffect`，其 `granted_tags` 不会被持久授予。它会先清理
`remove_effects_with_tags` 匹配的旧效果，再执行自身 instant modifier。需要在一段时间内授予
标签时应使用 `DurationTicks` 或 `Infinite`。

当前预检仍会验证 Instant 效果中配置的 `granted_tags`，并在它们非空时要求目标具有
`GameplayTagContainer`，否则返回 `MissingTagContainer`，但执行阶段不会真正授予这些标签。
因此 Instant 效果应把 `granted_tags` 保持为空，避免无效且容易误解的配置。

新建正周期效果设置 `execute_on_applied = true` 时，首次 instant pulse 发生在创建后、第一次
ongoing requirement 收敛之前。因此即使 ongoing requirement 初始不满足，也会先执行这一次
pulse，随后效果才进入抑制；不希望该行为时应关闭 `execute_on_applied` 或改用 application
requirement。

### 抑制 (Inhibition)

当持续标签要求不满足时，效果被**抑制**：

- 修饰器从聚合器中暂时移除
- 授予的标签暂时移除
- 该效果授予的 application immunity 暂停参与传入效果检查
- 条件恢复后，修饰器和标签自动恢复

Requirement 解析会按目标 Entity、slot、generation 的稳定顺序运行到固定点。一次队列请求
若创建或移除了 Active Effect，解析器会在下一请求前按需收敛；队列产生的 Tag、抑制和移除
因此能在当前 tick 内被后续请求观察。若配置形成自抑制等非收敛循环，参与循环的效果会按
稳定顺序 fail-closed 移除，避免每个 tick 反复振荡。

### 移除

效果可通过以下方式移除：

- 持续时间到期
- 移除标签要求被满足
- 显式调用 `remove_active_effect()`，或用 `remove_active_effects_with_tags()` 按 `asset_tags` 匹配

## 统一 Gameplay 执行队列

```rust
#[derive(Resource)]
pub struct GameplayExecutionQueue {
    // GameplayExecutionRequest::ApplyGameplayEffect
    // GameplayExecutionRequest::ActivateAbility
}
```

加入效果请求：

```rust
execution_queue.push_application(target, effect, payload);
```

游戏层生产系统应注册到公共生产阶段：

```rust
app.add_systems(
    FixedUpdate,
    queue_effect_requests.in_set(GameplayAbilitySystemSet::RequestProducers),
);
```

队列由 `process_gameplay_execution_queue_system` 在 `GameplayResolve` 阶段消费。技能激活和
效果应用共享一条 FIFO，因此 `效果 → 技能 → 效果` 的跨类型顺序不会被拆成两个批次。
系统在当前 `FixedUpdate` 中处理完整 drain，包括消费期间追加的派生请求；不会按请求数量
隐式分帧。只有在 `GameplayResolve` 之后才生产的请求，按阶段定义进入下一 tick。

## `EffectContext` 与 `EffectPayload`

### `EffectPayload`

携带效果执行元数据：

```rust
pub struct EffectPayload {
    source: Entity,
    instigator: Entity,
    causer: Option<Entity>,
    level: u32,
    source_snapshot: Option<AttributeSetSnapshot>,
}
```

- `source`：提供来源属性和来源标签等 Gameplay 数据的实体。
- `instigator`：发起产生该效果之行为的实体；默认等于 `source`，拥有者与实际发起者
  不同时可通过 `with_instigator()` 覆盖。
- `causer`：直接造成效果的可选物理实体，例如武器、投射物或爆炸区域。

运行时代码不得使用 `instigator` 或 `causer` 代替 `source` 查询消耗、冷却、来源属性
或来源标签。

### `EffectSystemParams`

Effect 的准备、应用、移除和 Requirement 收敛统一接收较窄的 `EffectSystemParams`。它只包含
Tag/Attribute manager、确定性随机资源，以及 Attribute、Tag、Active Effect 查询和内部 dirty
状态，不包含 ASC、Active Ability 或 Commands。`AbilitySystemParams` 内嵌该参数并实现
`DerefMut`，因此 Ability 编排仍可直接调用 Effect API。

### `EffectContext`

包装 `EffectPayload` 并提供世界查询引用，供 `ModifierMagnitudeCalculation` 使用：

```rust
pub struct EffectContext<'w, 's> {
    pub target: Option<Entity>,
    pub payload: &'w EffectPayload,
    pub attribute_id_manager: &'w AttributeIdManager,
    pub attr_set_query: &'w Query<'w, 's, &'static AttributeSet>,
    pub tag_container_query: &'w Query<'w, 's, &'static GameplayTagContainer>,
}

impl EffectContext<'_, '_> {
    pub fn source(&self) -> Entity;
    pub fn instigator(&self) -> Entity;
    pub fn causer(&self) -> Option<Entity>;
    pub fn level(&self) -> u32;
    pub fn source_snapshot(&self) -> Option<&AttributeSetSnapshot>;
    pub fn attribute_id_manager(&self) -> &AttributeIdManager;
}
```

`EffectContext` 实现中立的 `ModifierEvaluationContext`。自定义幅度计算只依赖该 trait，不能再
直接访问 ASC Query 或 Effect 存储；可通过 trait 方法读取 target/source/instigator/causer、
level、来源快照、属性注册表和来源/目标标签。

## 查询活跃效果

从目标实体取得 `ActiveGameplayEffects` 后，可使用 `handles(target)` 按稳定槽位顺序遍历，
并通过 `get(handle)` 读取运行时效果。旧的全局 `ActiveGameplayEffectTargetIndex` 已删除，
不再需要索引对账系统，也不存在效果实体被外部 despawn 后留下的悬空索引。
