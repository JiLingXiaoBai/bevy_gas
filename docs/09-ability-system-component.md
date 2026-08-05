# 09 — 技能系统组件 (ASC)

## 概述

`AbilitySystemComponent` (ASC) 是挂载到可使用技能的实体上的主要 `Component`。它管理已授予的技能、追踪阻止标签，并提供核心激活逻辑。

## `AbilitySystemComponent`

```rust
#[derive(Component, Default)]
#[require(ActiveGameplayEffects)]
pub struct AbilitySystemComponent {
    next_ability_handle: u32,
    abilities: Vec<GameplayAbilitySpec>,
    ability_indices: HashMap<AbilitySpecHandle, usize>,
    blocked_ability_tags: GameplayTagContainer,
}
```

### 主要方法

| 方法                                                        | 说明                                 |
| ----------------------------------------------------------- | ------------------------------------ |
| `give_ability(ability, level, input_id)`                    | 授予新技能；返回 `AbilitySpecHandle` |
| `clear_ability(handle)`                                     | 不存在或仍有活跃实例时返回 `false`；否则移除 |
| `get_ability_specs() -> &[GameplayAbilitySpec]`             | 获取所有已授予的技能规格             |
| `find_ability_spec(handle) -> Option<&GameplayAbilitySpec>` | 按句柄查找规格                       |
| `get_blocked_ability_tags() -> &GameplayTagContainer`       | 获取阻止标签容器                     |

### 内部方法（由激活流程使用）

| 方法                               | 说明                                                     |
| ---------------------------------- | -------------------------------------------------------- |
| `start_ability(...)`               | 设置阻止标签、递增活跃计数、生成 `ActiveGameplayAbility` |
| `finish_active_ability(handle)`    | 递减活跃计数、移除阻止标签                               |
| `rollback_started_ability(handle)` | 回滚后续失败的 `start_ability`                           |

## `AbilitySystemParams`

主要的 `SystemParam` 聚合器——打包了所有 GAS 操作所需的查询和资源：

```rust
pub struct AbilitySystemParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub tag_manager: Res<'w, GameplayTagManager>,
    pub random_gen: ResMut<'w, Random>,
    pub attribute_id_manager: Res<'w, AttributeIdManager>,
    pub attr_set_query: Query<'w, 's, &'static mut AttributeSet>,
    pub tag_container_query: Query<'w, 's, &'static mut GameplayTagContainer>,
    pub asc_query: Query<'w, 's, &'static mut AbilitySystemComponent>,
    pub attr_set_snapshot_query: Query<'w, 's, &'static AttributeSetSnapshot>,
    pub active_effect_query: Query<'w, 's, &'static mut ActiveGameplayEffects>,
    pub active_ability_query:
        Query<'w, 's, (Entity, &'static mut ActiveGameplayAbility)>,
}
```

上面只列出公开字段。实现还包含 Requirement dirty 状态和 pending Active Ability overlay，
用于同一结算 batch 的内部收敛与 deferred 可见性。

这是向 GAS 函数传递上下文的**标准方式**。大多数公开 API 函数接收 `&mut AbilitySystemParams` 而非单独的查询。
Gameplay 请求生产系统应单独声明 `ResMut<GameplayExecutionQueue>`；基础
`AbilitySystemParams` 不锁定全局队列，从而避免无关效果查询降低其他系统并行度。

## 激活函数

### `try_activate_ability_by_handle`

独立同步激活入口。常规运行时系统应写入 `GameplayExecutionQueue`；完整同步流程见
[07 — Gameplay 技能](./07-gameplay-abilities.md#激活流程)。

### `can_activate_ability`

执行标签与数值费用的快速预检，不实际激活：

```rust
pub fn can_activate_ability(
    source: Entity,
    target: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams,
) -> bool;
```

它不检查授予句柄、活跃实例数、技能链、取消或 startup 预检，因此不保证同步激活入口一定成功。
来源缺少 `GameplayTagContainer` 时还会跳过 required、blocked 和 cooldown tag 检查；使用标签
条件的 ASC 实体应显式附加该 Component。

### `commit_ability`

独立应用技能定义的消耗 + 冷却，不要求也不会启动活跃技能实例：

```rust
pub fn commit_ability(
    source: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams,
) -> Result<(), AbilityCommitError>;
```

该函数使用 batch 内部执行路径，本身不调用 Requirement 固定点收敛。若作为独立同步入口使用，
且后续逻辑必须立即观察 cooldown granted tags 或抑制变化，调用方还需执行
`resolve_active_effect_tag_requirements()`；常规技能激活路径会自行完成收敛。

### `end_ability` / `cancel_ability`

```rust
pub fn end_ability(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    params: &mut AbilitySystemParams,
) -> bool;
pub fn cancel_ability(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    params: &mut AbilitySystemParams,
) -> bool;
```

只有句柄存在且实例来源与 `source` 一致时才返回 `true`。

## 清理系统

`cleanup_finished_abilities_system` 在 `Cleanup` 集合中运行：

1. 查询状态为 `Ending` 或 `Cancelled` 的 `ActiveGameplayAbility` 实体
2. 在来源 ASC 上调用内部 `finish_active_ability()`，递减计数并移除阻止标签
3. 销毁 `ActiveGameplayAbility` 实体

## 典型用法

```rust
struct PendingCast {
    player: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
}

#[derive(Resource, Default)]
struct PendingCasts {
    requests: std::collections::VecDeque<PendingCast>,
}

fn grant_and_queue_example(
    asc: &mut AbilitySystemComponent,
    player: Entity,
    target: Entity,
    ability: Arc<GameplayAbility>,
    pending: &mut PendingCasts,
) {
    let handle = asc.give_ability(ability, 1, Some(0));
    pending.requests.push_back(PendingCast {
        player,
        target,
        handle,
    });
}

fn queue_pending_casts(
    mut execution_queue: ResMut<GameplayExecutionQueue>,
    mut casts: ResMut<PendingCasts>,
) {
    while let Some(cast) = casts.requests.pop_front() {
        let chain = execution_queue.new_root_chain(cast.handle);
        let context = AbilityActivationContext::direct(cast.player, chain);

        execution_queue.push_activation(cast.player, cast.target, cast.handle, context);
    }
}

app.init_resource::<PendingCasts>().add_systems(
    FixedUpdate,
    queue_pending_casts.in_set(GameplayAbilitySystemSet::RequestProducers),
);
```

`PendingCasts` 是一次性消费缓冲，不会因 Resource 持续存在而每 tick 重复激活。若输入或 AI
系统也在同一个 `FixedUpdate` 写入该缓冲，必须让写入系统 `.before(queue_pending_casts)`，或将
二者 `.chain()` 后放入 `RequestProducers`，避免消费者先运行而产生一 tick 延迟。
