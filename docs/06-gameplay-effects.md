# 06 — Gameplay 效果

## 概述

Gameplay 效果是核心的 Buff/Debuff 系统。它们对属性应用修饰器、授予标签，可以是即时、持续或无限的。

## 持续时间类型

| 类型                 | 行为                                   |
| -------------------- | -------------------------------------- |
| `Instant`            | 一次性修饰器应用（修改 base 值）       |
| `DurationTicks(u32)` | 持续 N 个 fixed-update tick 后自动移除 |
| `Infinite`           | 持久存在，直到显式移除                 |

## 效果定义 (`GameplayEffect`)

```rust
pub struct GameplayEffect {
    modifiers: Vec<Modifier>,
    duration: EffectDurationTicks,
    period: Option<EffectPeriodTicks>,
    probability_to_apply: f32,       // 0.0–1.0
    stacking_policy: StackingPolicy,
    tags: EffectTags,
}
```

### `EffectDurationTicks`

```rust
pub enum EffectDurationTicks {
    Instant,
    DurationTicks(ModifierMagnitude),  // 支持 Calculated 幅度
    Infinite,
}
```

### `EffectPeriodTicks`

启用周期性修饰器执行（例如每 3 tick 造成一次伤害）。

```rust
pub struct EffectPeriodTicks {
    period_ticks: ModifierMagnitude,
    execute_on_applied: bool,  // 若为 true，应用时立即执行一次
}
```

## 效果标签 (`EffectTags`)

效果的完整标签配置：

```rust
pub struct EffectTags {
    asset_tags: Vec<GameplayTag>,                        // 此效果的身份标签
    granted_tags: Vec<GameplayTag>,                      // 激活期间授予的标签
    source_application_tags: TagRequirements,            // 来源必须满足才能应用
    target_application_tags: TagRequirements,            // 目标必须满足才能应用
    source_ongoing_tags: TagRequirements,                // 来源必须维持（抑制检查）
    target_ongoing_tags: TagRequirements,                // 目标必须维持（抑制检查）
    source_removal_tags: TagRequirements,                // 来源满足则触发移除
    target_removal_tags: TagRequirements,                // 目标满足则触发移除
    granted_application_immunity: Vec<GameplayEffectImmunityQuery>,  // 免疫授予
    remove_effects_with_tags: Vec<GameplayTag>,          // 先移除带有这些标签的效果
}
```

### `TagRequirements`

```rust
pub struct TagRequirements {
    require_all: Vec<GameplayTag>,       // 必须全部存在
    ignore_any: Vec<GameplayTag>,        // 任一存在则阻止
    require_all_bits: GameplayTagBits,   // 预缓存位集
    ignore_any_bits: GameplayTagBits,    // 预缓存位集
}

impl TagRequirements {
    pub fn new(...) -> Result<Self, GameplayTagError>;
    pub fn passes(&self, tags: Option<&GameplayTagContainer>) -> bool;
    pub fn passes_tag_slice(&self, tags: &[GameplayTag], manager: &Res<GameplayTagManager>) -> Result<bool, GameplayTagError>;
    pub fn passes_tag_bits(&self, tag_bits: &GameplayTagBits) -> bool;
    pub fn is_empty(&self) -> bool;
}
```

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
        tag_manager: &Res<GameplayTagManager>,
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

### 子策略

| 策略                    | 选项                                        | 说明                      |
| ----------------------- | ------------------------------------------- | ------------------------- |
| `StackMagnitudePolicy`  | `None`, `Linear`                            | 幅度如何随堆叠数缩放      |
| `StackDurationPolicy`   | `KeepExisting`, `RefreshOnSuccessfulStack`  | 新堆叠时是否刷新持续时间  |
| `StackPeriodPolicy`     | `KeepCurrentTick`, `ResetOnSuccessfulStack` | 新堆叠时是否重置周期 tick |
| `StackOverflowPolicy`   | `RejectApplication`, `RefreshDuration`      | 达到堆叠上限时的行为      |
| `StackExpirationPolicy` | `RemoveAllStacks`, `RemoveSingleStack`      | 单层过期时的行为          |

### 便捷构造函数

```rust
StackingPolicy::non_stacking()                                          // 不堆叠
StackingPolicy::linear_refreshing(StackingType::AggregateBySource, 5)   // 线性、刷新
```

## 效果应用流程

### 直接应用

```rust
pub fn apply_gameplay_effect(
    target: Entity,
    effect: &Arc<GameplayEffect>,
    params: &mut AbilitySystemParams,
    payload: &EffectPayload,
) -> Result<(), GameplayEffectApplicationError>;
```

### 两阶段 API

```rust
// 阶段 1：准备（检查条件、查找可堆叠、收集待移除）
let plan = prepare_gameplay_effect(target, effect, params, &payload)?;

// 阶段 2：执行
execute_gameplay_effect_plan(plan, params)?;
```

准备和执行阶段都会验证目标组件、每个 modifier 对应的属性是否已初始化、Attribute ID、
GameplayTag，以及所有待移除效果的清理条件。只有完整预检通过后才开始修改 World，
避免多 modifier 效果只应用一部分，或新效果执行失败时已经移除了旧效果。

`GameplayEffectApplicationError` 区分概率拒绝、标签条件、免疫、无效持续时间、
缺少目标组件、目标未初始化属性、堆叠溢出、无效标签和无效属性 ID。
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
6. **收集待移除** — 查找匹配 `remove_effects_with_tags` 的效果
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

### `ActiveGameplayEffect`

```rust
pub struct ActiveGameplayEffect {
    spec: GameplayEffectSpec,
    source: Entity,
    target: Entity,
    stack_count: u32,
    inhibited: bool,
}
```

### 持续时间 Tick

每个 `FixedUpdate`，`tick_effect_duration_system` 递减剩余 tick。归零时效果被移除（修饰器清理、标签移除）。

### 周期 Tick

`tick_effect_period_system` 追踪每个效果的周期计数器。当计数器达到周期间隔时，重新应用修饰器。

### 抑制 (Inhibition)

当持续标签要求不满足时，效果被**抑制**：

- 修饰器从聚合器中暂时移除
- 授予的标签暂时移除
- 条件恢复后，修饰器和标签自动恢复

### 移除

效果可通过以下方式移除：

- 持续时间到期
- 移除标签要求被满足
- 显式调用 `remove_active_effect()` / `remove_active_effects_with_tags()`

## 效果应用队列

```rust
#[derive(Resource)]
pub struct GameplayEffectApplicationQueue {
    requests: VecDeque<GameplayEffectApplicationRequest>,
    max_applications_per_tick: usize,  // 默认：256
}
```

将效果加入延迟应用队列：

```rust
effect_queue.push_application(target, effect, payload);
```

队列由 `process_gameplay_effect_application_queue_system` 消费（仅在有工作时运行）。

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

- `source`：提供 ASC、技能规格、来源属性和来源标签的 Gameplay 数据来源。
- `instigator`：发起产生该效果之行为的实体；默认等于 `source`，拥有者与实际发起者
  不同时可通过 `with_instigator()` 覆盖。
- `causer`：直接造成效果的可选物理实体，例如武器、投射物或爆炸区域。

运行时代码不得使用 `instigator` 或 `causer` 代替 `source` 查询消耗、冷却、来源属性
或来源标签。

### `EffectContext`

包装 `EffectPayload` 并提供世界查询引用，供 `ModifierMagnitudeCalculation` 使用：

```rust
pub struct EffectContext<'w, 's> {
    pub target: Option<Entity>,
    pub payload: &'w EffectPayload,
    pub attribute_id_manager: &'w AttributeIdManager,
    pub attr_set_query: &'w Query<'w, 's, &'static AttributeSet>,
    pub tag_container_query: &'w Query<'w, 's, &'static GameplayTagContainer>,
    pub asc_query: &'w Query<'w, 's, &'static AbilitySystemComponent>,
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

## `ActiveGameplayEffectTargetIndex`

提供按目标实体 O(1) 查找活跃效果的 `Resource`：

```rust
pub struct ActiveGameplayEffectTargetIndex {
    by_target: HashMap<Entity, Vec<ActiveEffectHandle>>,
    by_handle: HashMap<ActiveEffectHandle, Entity>,
}

impl ActiveGameplayEffectTargetIndex {
    pub fn handles_for(&self, target: Entity) -> &[ActiveEffectHandle];
}
```
