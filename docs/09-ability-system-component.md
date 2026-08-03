# 09 — 技能系统组件 (ASC)

## 概述

`AbilitySystemComponent` (ASC) 是挂载到可使用技能的实体上的主要 `Component`。它管理已授予的技能、追踪阻止标签，并提供核心激活逻辑。

## `AbilitySystemComponent`

```rust
#[derive(Component)]
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
| `clear_ability(handle)`                                     | 移除已授予的技能                     |
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
    pub active_effect_target_index: ResMut<'w, ActiveGameplayEffectTargetIndex>,
    pub active_effect_query: Query<'w, 's, (
        Entity,
        &'static mut ActiveGameplayEffect,
        Option<&'static ActiveEffectDurationTicks>,
        Option<&'static ActiveEffectPeriodTicks>,
    )>,
    pub active_ability_query: Query<'w, 's, &'static mut ActiveGameplayAbility>,
}
```

这是向 GAS 函数传递上下文的**标准方式**。大多数公开 API 函数接收 `&mut AbilitySystemParams` 而非单独的查询。

## 激活函数

### `try_activate_ability_by_handle`

主激活入口。完整流程见 [07 — Gameplay 技能](./07-gameplay-abilities.md#激活流程)。

### `can_activate_ability`

检查技能是否可激活而不实际执行：

```rust
pub fn can_activate_ability(
    source: Entity,
    handle: AbilitySpecHandle,
    params: &mut AbilitySystemParams,
) -> bool;
```

### `commit_ability`

对已激活的技能执行消耗 + 冷却：

```rust
pub fn commit_ability(
    source: Entity,
    handle: AbilitySpecHandle,
    params: &mut AbilitySystemParams,
) -> Result<(), AbilityActivationError>;
```

### `end_ability` / `cancel_ability`

```rust
pub fn end_ability(handle: ActiveAbilityHandle, params: &mut AbilitySystemParams);
pub fn cancel_ability(handle: ActiveAbilityHandle, params: &mut AbilitySystemParams);
```

## 清理系统

`cleanup_finished_abilities_system` 在 `Cleanup` 集合中运行：

1. 查询状态为 `Ending` 或 `Cancelled` 的 `ActiveGameplayAbility` 实体
2. 在来源 ASC 上调用 `finish_ability_with_status()`
3. 销毁 `ActiveGameplayAbility` 实体

## 典型用法

```rust
fn grant_fireball_to_player(
    mut params: AbilitySystemParams,
    player: Entity,
    fireball_def: Arc<GameplayAbility>,
) {
    // 1. 获取或插入玩家上的 ASC
    // 2. 授予技能
    if let Ok(mut asc) = params.asc_query.get_mut(player) {
        let handle = asc.give_ability(fireball_def, 1, Some(0)); // 等级 1，输入 0
    }
}

fn try_cast_fireball(
    mut params: AbilitySystemParams,
    player: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
    mut activation_queue: ResMut<AbilityActivationQueue>,
) {
    let chain = activation_queue.new_root_chain(handle);
    let context = AbilityActivationContext::direct(player, chain);

    activation_queue.push_activation(player, target, handle, context);
}
```
