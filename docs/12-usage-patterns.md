# 12 — 使用模式与示例

## 职责

本文只组合已经公开的 GAS API，展示常见的能力与效果写法。类型的完整语义分别见
[Gameplay Effects](./06-gameplay-effects.md)、[Gameplay Abilities](./07-gameplay-abilities.md)
和 [Gameplay 执行模块](./16-gameplay-execution.md)。示例使用精简 prelude，并从 owning domain
显式导入不属于 prelude 的任务、周期、堆叠和错误类型：

```rust
use bevy::prelude::*;
use bevy_tools::prelude::*;
use bevy_tools::gas::gameplay_abilities::{
    AbilityTaskDef,
    AbilityTaskEvent,
    AbilityTaskOnFinishedDef,
};
use bevy_tools::gas::gameplay_effects::{
    EffectPeriodTicks,
    GameplayEffectImmunityQuery,
    StackingType,
};
use bevy_tools::gas::gameplay_tags::GameplayTagError;
use bevy_tools::UniqueName;
use std::sync::Arc;
```

## 源码入口

| 用途 | 源码 |
|---|---|
| 完整角色组件组合 | `src/gas/ability_system/component.rs` |
| 固定 tick 阶段与插件 | `src/gas/runtime_plugin.rs` |
| 能力定义、激活上下文 | `src/gas/gameplay_abilities/` |
| Task 定义与完成动作 | `src/gas/gameplay_abilities/ability_task/` |
| 效果定义、时长、标签与堆叠 | `src/gas/gameplay_effects/gameplay_effect/` |
| 统一请求队列与请求值 | `src/gas/gameplay_execution/` |

## 使用前提

完整 GAS 角色应显式安装插件并生成 `GameplayAbilitySystemBundle`：

```rust
app.add_plugins(GameplayAbilitySystemPlugin);

let actor = commands
    .spawn(GameplayAbilitySystemBundle::default())
    .id();
```

`GameplayTag` 与 `AttributeId` 必须先通过各自的注册接口取得；需要使用的属性还必须在角色的
`AttributeSet` 上初始化。只使用标签或属性的实体可以单独安装对应组件，不需要
`ActiveGameplayEffects`。

以下辅助函数可用于不需要任何标签行为的效果：

```rust
fn empty_effect_tags() -> EffectTags {
    EffectTags::new(
        vec![],
        vec![],
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        vec![],
        vec![],
    )
}
```

## 公共请求入口与执行时序

外部系统不应直接调用内部 resolver。应在
`GameplayAbilitySystemSet::RequestProducers` 或更早的阶段向
`GameplayExecutionQueue` 写入请求：

```rust
fn enqueue_direct_activation(
    queue: &mut GameplayExecutionQueue,
    source_attributes: &mut AttributeSet,
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
) {
    let snapshot = source_attributes.make_snapshot(source);
    let chain = queue.new_root_chain(handle);
    let context = AbilityActivationContext::direct(source, chain)
        .with_source_snapshot(snapshot);

    queue.push_activation(source, target, handle, context);
}
```

队列是 FIFO，并在 `GameplayResolve` 中完整 drain；resolver 在处理请求时派生的新请求也会在
同一次 drain 中继续处理。`GameplayResolve` 之后才产生的请求会留到下一 fixed tick。完整阶段
顺序见 [Gameplay 执行模块](./16-gameplay-execution.md#同-tick-与下一-tick-边界)。

## 模式 1：带快照计算的直接伤害技能

计算器只依赖独立的 `ModifierEvaluationContext`，因此 modifier 不需要依赖 Effect 或 ASC：

```rust
struct FireballDamage {
    attack_id: AttributeId,
}

impl ModifierMagnitudeCalculation for FireballDamage {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32 {
        let attack = context
            .source_snapshot()
            .and_then(|snapshot| {
                snapshot
                    .get_current_value(context.attribute_id_manager(), self.attack_id)
                    .ok()
                    .flatten()
            })
            .unwrap_or(0.0);

        -(50.0 + attack * 1.5 + context.level() as f32 * 10.0)
    }
}

fn make_fireball_damage(
    health_id: AttributeId,
    attack_id: AttributeId,
    damage_tag: GameplayTag,
) -> Arc<GameplayEffect> {
    Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            health_id,
            ModifierOperation::Add,
            ModifierMagnitude::Calculated(Box::new(FireballDamage { attack_id })),
        )],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            vec![damage_tag],
            vec![],
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            vec![],
            vec![],
        ),
    ))
}
```

能力的 startup tasks 是从激活时刻开始的并列时间线，不是依次执行的 continuation：

```rust
fn make_fireball_ability(
    damage_effect: Arc<GameplayEffect>,
    cooldown_effect: Arc<GameplayEffect>,
    cost_effect: Arc<GameplayEffect>,
    fireball_tag: GameplayTag,
    stun_tag: GameplayTag,
) -> Arc<GameplayAbility> {
    Arc::new(GameplayAbility::new(
        AbilityTags::new(
            vec![fireball_tag],
            vec![],
            vec![],
            vec![],
            vec![stun_tag],
        ),
        vec![
            AbilityTaskDef::wait_ticks(
                5,
                AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                    effect: damage_effect,
                },
            ),
            AbilityTaskDef::wait_ticks(6, AbilityTaskOnFinishedDef::EndAbility),
        ],
        Some(cooldown_effect),
        Some(cost_effect),
        vec![],
        false,
        false,
    ))
}

fn grant_fireball(
    ability_system: &mut AbilitySystemComponent,
    ability: Arc<GameplayAbility>,
) -> AbilitySpecHandle {
    ability_system.give_ability(ability, 1, Some(0))
}
```

`AbilityActivationContext::direct()` 不会自动捕获属性；要让上面的计算器读取施法时刻攻击力，
producer 必须像本节的队列示例一样附加 `AttributeSetSnapshot`。若没有快照，示例计算器显式回退
到 `0.0`。作为 ability cost 的效果必须是 Instant 且只包含 `Add` modifier。当前支付检查按
Modifier 独立比较检查开始时的当前值，因此同一个 Cost 对同一属性应只配置一个扣减 Modifier；
多个扣减项不会先合并，合计值可能越过零。

## 模式 2：周期伤害（DoT）

下面的中毒每 3 tick 造成 10 点伤害，持续 15 tick，并在应用时立即执行一次：

```rust
fn make_poison(health_id: AttributeId) -> Arc<GameplayEffect> {
    Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            health_id,
            ModifierOperation::Add,
            ModifierMagnitude::Flat(-10.0),
        )],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(15.0)),
        Some(EffectPeriodTicks::new(
            ModifierMagnitude::Flat(3.0),
            true,
        )),
        1.0,
        StackingPolicy::linear_refreshing(StackingType::AggregateByTarget, 3),
        empty_effect_tags(),
    ))
}
```

运行时先递减和清理 duration，再处理 period。因此它会在应用时以及之后的第 3、6、9、12 个
fixed tick 执行；第 15 tick 先到期，不会再执行一次。正 period 使用即时修改 base value 的
脉冲；period 求值为 `0` 时则退化为持续 modifier，`execute_on_applied` 不再产生额外脉冲。

## 模式 3：带 ongoing 条件的 Buff

`source_ongoing_tags` 和 `target_ongoing_tags` 控制 active effect 是否处于启用状态：

```rust
fn make_speed_buff(
    speed_id: AttributeId,
    buff_tag: GameplayTag,
    granted_tag: GameplayTag,
    alive_tag: GameplayTag,
    stun_tag: GameplayTag,
) -> Result<Arc<GameplayEffect>, GameplayTagError> {
    let source_ongoing = TagRequirements::new(vec![alive_tag], vec![stun_tag])?;
    let tags = EffectTags::new(
        vec![buff_tag],
        vec![granted_tag],
        TagRequirements::default(),
        TagRequirements::default(),
        source_ongoing,
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        vec![],
        vec![],
    );

    Ok(Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            speed_id,
            ModifierOperation::PercentAdd,
            ModifierMagnitude::Flat(0.3),
        )],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags,
    )))
}
```

条件不满足时，modifier、授予标签和免疫都会暂时移除；duration 仍继续倒计时，period 暂停。
条件重新满足后它们会恢复。条件循环不能收敛时，参与循环的效果会 fail-closed 移除。

## 模式 4：链式激活

每个 `WaitTicks` 都以当前能力的激活时刻为起点。三连击的前两段可分别使用下面的 startup
task 列表：

```rust
let hit_one_tasks = vec![
    AbilityTaskDef::wait_ticks(
        3,
        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
            effect: hit_one_damage,
        },
    ),
    AbilityTaskDef::wait_ticks(
        4,
        AbilityTaskOnFinishedDef::ActivateAbility {
            handle: hit_two_handle,
        },
    ),
    AbilityTaskDef::wait_ticks(5, AbilityTaskOnFinishedDef::EndAbility),
];

let hit_two_tasks = vec![
    AbilityTaskDef::wait_ticks(
        3,
        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
            effect: hit_two_damage,
        },
    ),
    AbilityTaskDef::wait_ticks(
        4,
        AbilityTaskOnFinishedDef::ActivateAbility {
            handle: hit_three_handle,
        },
    ),
    AbilityTaskDef::wait_ticks(5, AbilityTaskOnFinishedDef::EndAbility),
];
```

若把 `ActivateAbility` 放在 sibling `Instant` task 中，下一段会在当前激活的队列 drain 中立即
入队。单个 `AbilityTaskOnFinishedDef` 只能表达一种完成动作；一个完成点需要触发多个项目级动作
时，可使用 `EmitEvent` 交给 Observer 按明确顺序处理，或新增项目级组合 task。Observer 派生
请求是否仍在同一 tick 消费取决于事件产生阶段，见下一节。

## 模式 5：事件驱动逻辑

`AbilityTaskEvent` 通过 Bevy Observer 发送，不使用 `EventReader`：

```rust
#[derive(Resource)]
struct AbilityEventIds {
    release_projectile: UniqueName,
}

fn observe_ability_task(
    event: On<AbilityTaskEvent>,
    event_ids: Res<AbilityEventIds>,
) {
    if event.get_event_id() == event_ids.release_projectile {
        // Produce project-specific work here.
    }
}

app.insert_resource(AbilityEventIds {
    release_projectile: release_projectile_event,
})
.add_observer(observe_ability_task);

let release_task = AbilityTaskDef::wait_ticks(
    10,
    AbilityTaskOnFinishedDef::EmitEvent {
        event_id: release_projectile_event,
    },
);
```

Observer 若再写入 gameplay 请求，应根据它实际运行的阶段判断是同 tick 还是下一 tick；不能仅凭
“由 task 触发”推断执行时机。

## 模式 6：授予效果免疫

下面的 active effect 会阻止带 `stun_tag` asset tag 的新效果：

```rust
fn make_stun_immunity(
    immunity_tag: GameplayTag,
    stun_tag: GameplayTag,
) -> Result<Arc<GameplayEffect>, GameplayTagError> {
    let stun_effects = TagRequirements::new(vec![stun_tag], vec![])?;
    let query = GameplayEffectImmunityQuery::new(
        TagRequirements::default(),
        stun_effects,
    );
    let tags = EffectTags::new(
        vec![immunity_tag],
        vec![],
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        vec![query],
        vec![],
    );

    Ok(Arc::new(GameplayEffect::new(
        vec![],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(60.0)),
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags,
    )))
}
```

免疫查询的 `source_tags` 匹配新效果的来源实体标签，`effect_tags` 匹配新效果的 asset tags；
它不会匹配新效果将要授予目标的标签。只有已启用的 active effect 才提供免疫。

## 模式 7：按标签移除效果

`target_removal_tags` 使 active effect 在目标满足条件时移除自身；
`remove_effects_with_tags` 则在本效果成功应用时，先清除目标上匹配 asset tag 的旧效果：

```rust
fn removal_effect_tags(
    effect_tag: GameplayTag,
    dead_tag: GameplayTag,
    dispellable_tag: GameplayTag,
) -> Result<EffectTags, GameplayTagError> {
    let remove_when_dead = TagRequirements::new(vec![dead_tag], vec![])?;

    Ok(EffectTags::new(
        vec![effect_tag],
        vec![],
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        remove_when_dead,
        vec![],
        vec![dispellable_tag],
    ))
}
```

移除匹配使用继承后的 tag bits；父标签可匹配子标签。由于双方都采用继承集合，拥有共同祖先的
兄弟标签也可能匹配，设计标签层级时应把这一点纳入规则。

## 边界与维护建议

- 所有 duration、period、task wait 都以 `FixedUpdate` tick 为单位，不是秒。
- Instant 效果不创建 active entity；它忽略 period，也不会长期授予标签或免疫。
- 有 modifier 的效果要求目标存在 `AttributeSet` 且目标属性已初始化；持续或无限效果还要求
  `ActiveGameplayEffects`；授予标签的效果要求 `GameplayTagContainer`。
- 先注册 tag/attribute，再构造长期复用的 `Arc<GameplayEffect>` 与
  `Arc<GameplayAbility>`，避免把注册和热路径执行混在一起。
- 同一效果定义是否可堆叠通过 `Arc::ptr_eq` 判断；需要共享堆叠身份时必须复用同一个 `Arc`。
- 直接效果应用使用 `GameplayExecutionQueue::push_application()` 和 `EffectPayload`；能力激活使用
  `new_root_chain()`、`AbilityActivationContext` 与 `push_activation()`。
