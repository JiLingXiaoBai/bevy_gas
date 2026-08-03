# 07 — Gameplay 技能

## 概述

Gameplay 技能代表角色可执行的动作——法术、攻击、冲刺等。它们支持冷却、消耗、激活效果、启动任务和链式激活。

## 技能定义 (`GameplayAbility`)

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

### `AbilityTags`

```rust
pub struct AbilityTags {
    ability_asset_tags: Vec<GameplayTag>,        // 此技能的身份标签
    cancel_abilities_with_tags: Vec<GameplayTag>, // 取消带有这些标签的其他技能
    block_abilities_with_tags: Vec<GameplayTag>,  // 阻止带有这些标签的其他技能
    activation_required_tags: Vec<GameplayTag>,   // 来源必须拥有才能激活
    activation_blocked_tags: Vec<GameplayTag>,    // 来源必须不拥有才能激活
}
```

### `GameplayAbilitySpec`

已授予技能的运行时实例：

```rust
pub struct GameplayAbilitySpec {
    handle: AbilitySpecHandle,
    ability: Arc<GameplayAbility>,
    level: u32,
    input_id: Option<u16>,     // 可选的输入绑定
    input_pressed: bool,       // 绑定输入当前是否处于按下状态
    active_count: u32,         // 当前活跃实例数
}
```

`input_pressed` 默认为 `false`。输入处理系统应在绑定输入按下、松开时分别通过
`set_input_pressed(true)` 和 `set_input_pressed(false)` 更新它；可使用
`is_input_pressed()` 查询当前状态。该字段只记录瞬时输入状态，不负责自动激活技能。

### `AbilitySpecHandle`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbilitySpecHandle(u32);
```

## 激活流程

### `try_activate_ability_by_handle()`

技能激活的主入口：

```rust
pub fn try_activate_ability_by_handle(
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
    context: AbilityActivationContext,
    params: &mut AbilitySystemParams,
) -> Result<ActiveAbilityHandle, AbilityActivationError>;
```

**激活序列：**

```
1. 取消匹配 cancel_abilities_with_tags 的活跃技能
2. 检查活跃计数（多实例控制）
3. passes_ability_activation_requirements()
   ├── 阻止标签检查（来源 ASC 的 blocked_ability_tags）
   ├── 激活阻止标签检查（来源标签）
   ├── 激活要求标签检查（来源标签）
   └── 冷却标签检查（来源是否有冷却标签？）
4. prepare_ability_commit_plans()
   ├── 准备消耗计划
   └── 准备冷却计划
5. start_ability()
   ├── 在来源 ASC 上设置 block_abilities_with_tags
   ├── 递增 active_count
   └── 生成 ActiveGameplayAbility 实体
6. execute_ability_commit_plans()
   ├── 执行消耗效果 → 失败则回滚
   └── 执行冷却效果 → 失败则回滚
7. 应用 activation_effects（尽力而为）
8. spawn_startup_ability_tasks()
9. 若 end_on_activation → 将状态设为 Ending
```

### `AbilityActivationError`

```rust
pub enum AbilityActivationError {
    InvalidChain(AbilityChainError),
    MissingAbilitySystemComponent { source: Entity },
    AbilityNotFound { source: Entity, handle: AbilitySpecHandle },
    MultipleInstancesNotAllowed { source: Entity, handle: AbilitySpecHandle },
    ActivationRequirementsNotMet { source: Entity, handle: AbilitySpecHandle },
    CommitPreparationFailed { source: Entity, handle: AbilitySpecHandle },
    StartFailed { source: Entity, handle: AbilitySpecHandle },
    CommitExecutionFailed { source: Entity, handle: AbilitySpecHandle },
}
```

### `AbilityActivationContext`

```rust
pub struct AbilityActivationContext {
    chain: Option<AbilityChainContext>,
    instigator: Entity,
    causer: Option<Entity>,
    source_snapshot: Option<AttributeSetSnapshot>,
    reason: AbilityActivationReason,
}

pub enum AbilityActivationReason {
    Direct,
    Input { input_id: u16 },
    Chained { parent_ability: ActiveAbilityHandle },
    TaskEvent { event_id: UniqueName },
    GameplayEffect,
}
```

`instigator` 默认等于技能来源实体，可通过 `with_instigator()` 指定实际发起者。
`causer` 表示直接造成技能行为的可选物理实体。两者都会沿链式技能激活继承，并传播到
技能产生的 `EffectPayload`；消耗、冷却、来源属性和来源标签仍始终从技能的 `source`
读取。

### `AbilityChainContext`

追踪链式技能激活，防止无限循环：

```rust
pub struct AbilityChainContext {
    chain_id: u64,
    depth: u8,                              // 最大：ABILITY_CHAIN_MAX_DEPTH (8)
    visited: Vec<AbilitySpecHandle>,        // 循环检测
}

impl AbilityChainContext {
    pub fn root(handle: AbilitySpecHandle, chain_id: u64) -> Self;
    pub fn next(&self, handle: AbilitySpecHandle) -> Result<Self, AbilityChainError>;
}
```

## 活跃技能生命周期

### `ActiveGameplayAbility`

技能激活时生成的 `Component`：

```rust
#[derive(Component, Clone)]
pub struct ActiveGameplayAbility {
    source: Entity,
    spec_handle: AbilitySpecHandle,
    target: Entity,
    status: AbilityActivationStatus,
    activation_context: AbilityActivationContext,
}
```

### `AbilityActivationStatus`

```
Active ──► Ending ──► (销毁)
   │
   └──► Cancelled ──► (销毁)
```

| 状态         | 说明                         |
| ------------ | ---------------------------- |
| `Active`     | 运行中；任务正在 tick        |
| `Ending`     | 正常关闭；任务停止，清理开始 |
| `Cancelled`  | 强制关闭；任务停止，清理开始 |

### 生命周期方法

| 函数                                                   | 说明                   |
| ------------------------------------------------------ | ---------------------- |
| `end_ability(handle, params)`                          | 将状态设为 `Ending`    |
| `cancel_ability(handle, params)`                       | 将状态设为 `Cancelled` |
| `can_activate_ability(source, handle, params) -> bool` | 检查而不实际激活       |
| `commit_ability(source, handle, params) -> Result`     | 仅执行消耗 + 冷却      |

`cleanup_finished_abilities_system` 销毁处于 `Ending` 或 `Cancelled` 状态的技能，递减活跃计数，移除阻止标签。

## 技能激活队列

```rust
#[derive(Resource)]
pub struct AbilityActivationQueue {
    requests: VecDeque<AbilityActivationRequest>,
    max_activations_per_tick: usize,  // 默认：64
    next_chain_id: u64,
}
```

将激活加入延迟处理队列：

```rust
activation_queue.push_activation(source, target, handle, context);
```

链式激活（带循环/深度保护）：

```rust
activation_queue.push_chained_activation(
    source, target, handle, parent_ability, &parent_context,
)?;
```

## 技能任务

详见 [08 — 技能任务](./08-ability-tasks.md)。
