# 12 — 使用模式与示例

## 模式 1：直接伤害技能

一个火球术，经过短暂前摇后造成即时伤害。

```rust
// 1. Define the instant damage effect.
let damage_effect = Arc::new(GameplayEffect::new(
    vec![Modifier::new(
        health_id,
        ModifierOperation::Add,
        ModifierMagnitude::Calculated(Box::new(FireballDamageCalc { attack_id })),
    )],
    EffectDurationTicks::Instant,
    None,       // 无周期
    1.0,        // 100% 概率
    StackingPolicy::non_stacking(),
    EffectTags::new(
        vec![damage_tag],           // asset_tags
        vec![],                     // granted_tags
        TagRequirements::default(), // source_application
        TagRequirements::default(), // target_application
        TagRequirements::default(), // source_ongoing
        TagRequirements::default(), // target_ongoing
        TagRequirements::default(), // source_removal
        TagRequirements::default(), // target_removal
        vec![],                     // immunity
        vec![],                     // remove_effects_with_tags
    ),
));

// 2. Define the fireball ability.
let fireball = Arc::new(GameplayAbility::new(
    AbilityTags::new(
        vec![fireball_tag],
        vec![],
        vec![],
        vec![],
        vec![stun_tag],  // 眩晕时阻止
    ),
    vec![
        // Apply damage after a 5-tick wind-up.
        AbilityTaskDef::wait_ticks(5,
            AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                effect: damage_effect.clone(),
            },
        ),
        // Startup tasks are sibling timelines, so end at the next absolute offset.
        AbilityTaskDef::wait_ticks(6, AbilityTaskOnFinishedDef::EndAbility),
    ],
    Some(cooldown_effect),  // 30 tick 冷却
    Some(cost_effect),      // 25 法力消耗
    vec![],                 // 无激活效果
    false,                  // 激活时不结束
    false,                  // 仅单实例
));

// 3. Grant the definition through an ASC obtained by game setup code.
fn grant_fireball(
    asc: &mut AbilitySystemComponent,
    fireball: Arc<GameplayAbility>,
) -> AbilitySpecHandle {
    asc.give_ability(fireball, 1, Some(0))
}

struct FireballCast {
    player: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
}

#[derive(Resource)]
struct FireballBinding {
    player: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
}

#[derive(Resource, Default)]
struct PendingFireballCasts {
    requests: std::collections::VecDeque<FireballCast>,
}

fn collect_fireball_input(
    input: Res<ButtonInput<KeyCode>>,
    binding: Res<FireballBinding>,
    mut casts: ResMut<PendingFireballCasts>,
    mut was_pressed: Local<bool>,
) {
    let pressed = input.pressed(KeyCode::KeyQ);
    if pressed && !*was_pressed {
        casts.requests.push_back(FireballCast {
            player: binding.player,
            target: binding.target,
            handle: binding.handle,
        });
    }
    *was_pressed = pressed;
}

fn cast_fireball(
    mut queue: ResMut<GameplayExecutionQueue>,
    mut attributes: Query<&mut AttributeSet>,
    mut casts: ResMut<PendingFireballCasts>,
) {
    while let Some(cast) = casts.requests.pop_front() {
        let Ok(mut source_attributes) = attributes.get_mut(cast.player) else {
            error!("fireball source has no AttributeSet");
            continue;
        };
        let snapshot = source_attributes.make_snapshot(cast.player);
        let chain = queue.new_root_chain(cast.handle);
        let context = AbilityActivationContext::direct(cast.player, chain)
            .with_source_snapshot(snapshot);
        queue.push_activation(cast.player, cast.target, cast.handle, context);
    }
}

app.init_resource::<PendingFireballCasts>()
    .insert_resource(FireballBinding {
        player,
        target,
        handle,
    })
    .add_systems(
        FixedUpdate,
        (collect_fireball_input, cast_fireball)
            .chain()
            .in_set(GameplayAbilitySystemSet::RequestProducers),
    );
```

输入系统只在按键边沿向 `PendingFireballCasts::requests` 追加一项；producer 会取走每项请求，
因此不会因 Resource 持续存在而在每个 fixed tick 重复施法。`.chain()` 同时保证先生产再消费，
避免同一 set 中未排序的两个可变借用系统偶发把请求留到下一 tick。

内置 task 只有一个完成动作，因此上例用 5/6 两个相对激活时刻的绝对等待点表达“伤害后
结束”。如果必须在同一个 tick 同时应用伤害并结束技能，应增加组合完成动作，或让
`WaitTicks::EmitEvent` 的 Observer 按明确顺序执行两项操作。

## 模式 2：持续伤害 (DoT)

一个中毒效果，每 3 tick 造成 10 点伤害，持续 15 tick，并在应用时立即执行一次。

```rust
let poison_effect = Arc::new(GameplayEffect::new(
    vec![Modifier::new(
        health_id,
        ModifierOperation::Add,
        ModifierMagnitude::Flat(-10.0),  // 10 damage per period
    )],
    EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(15.0)),
    Some(EffectPeriodTicks::new(
        ModifierMagnitude::Flat(3.0),
        true,  // 应用时立即执行
    )),
    1.0,
    StackingPolicy::linear_refreshing(StackingType::AggregateByTarget, 3),
    // ... tags
));
```

Duration 在 Period 之前递减并清理。所以上例会在应用时以及之后的第 3、6、9、12 个 fixed
tick 造成伤害；第 15 tick 先到期移除，不会再执行一次周期伤害。

## 模式 3：带持续标签要求的 Buff

一个速度 Buff，仅在来源存活且未眩晕时生效。

```rust
let speed_buff = Arc::new(GameplayEffect::new(
    vec![Modifier::new(
        speed_id,
        ModifierOperation::PercentAdd,
        ModifierMagnitude::Flat(0.3),  // +30% 速度
    )],
    EffectDurationTicks::Infinite,
    None,
    1.0,
    StackingPolicy::non_stacking(),
    EffectTags::new(
        vec![buff_tag, speed_buff_tag],
        vec![speed_buff_granted_tag],
        TagRequirements::default(),
        TagRequirements::default(),
        // 来源持续条件：必须存活且未眩晕
        TagRequirements::new(
            vec![alive_tag],
            vec![stun_tag],
        )?,
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        vec![],
        vec![],
    ),
));
```

## 模式 4：链式技能（连招）

三连击，每击链式激活下一击。

```rust
// 第 1 击链到第 2 击
let hit1 = Arc::new(GameplayAbility::new(
    /* ... */,
    vec![
        AbilityTaskDef::wait_ticks(3,
            AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                effect: hit1_damage.clone(),
            },
        ),
        // Sibling task at the next absolute offset activates hit 2.
        AbilityTaskDef::wait_ticks(4,
            AbilityTaskOnFinishedDef::ActivateAbility {
                handle: hit2_spec_handle,
            },
        ),
        AbilityTaskDef::wait_ticks(5, AbilityTaskOnFinishedDef::EndAbility),
    ],
    /* ... */
));

// 第 2 击链到第 3 击
let hit2 = Arc::new(GameplayAbility::new(
    /* ... */,
    vec![
        AbilityTaskDef::wait_ticks(3,
            AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                effect: hit2_damage.clone(),
            },
        ),
        // Sibling task at the next absolute offset activates hit 3.
        AbilityTaskDef::wait_ticks(4,
            AbilityTaskOnFinishedDef::ActivateAbility {
                handle: hit3_spec_handle,
            },
        ),
        AbilityTaskDef::wait_ticks(5, AbilityTaskOnFinishedDef::EndAbility),
    ],
    /* ... */
));
```

startup task 列表不是串行 continuation。若把上例的 `ActivateAbility` 写成 sibling `Instant`，
下一击会在当前激活 drain 中立刻入队，而不会等待伤害任务。当前内置完成动作也不能在同一个
Wait 完成点同时“造成伤害 + 激活下一击”；需要同 tick 连招时应扩展组合完成动作，或用
`EmitEvent` Observer 依次写入两个请求。

## 模式 5：事件驱动技能逻辑

使用 `AbilityTaskEvent` 在技能时间线的特定节点触发自定义游戏逻辑。

```rust
#[derive(Resource)]
struct AbilityEventIds {
    custom: UniqueName,
}

fn observe_ability_events(
    event: On<AbilityTaskEvent>,
    event_ids: Res<AbilityEventIds>,
) {
    if event.get_event_id() == event_ids.custom {
        // Spawn VFX or request audio playback here.
    }
}

app.insert_resource(AbilityEventIds {
    custom: my_custom_event_id,
})
.add_observer(observe_ability_events);

// In the ability definition:
AbilityTaskDef::wait_ticks(10,
    AbilityTaskOnFinishedDef::EmitEvent {
        event_id: my_custom_event_id,
    },
)
```

`AbilityTaskEvent` 由 `Commands::trigger()` 发送给 Observer，不使用 `EventReader`。若 Observer
还要生产 Gameplay 请求，其生效 tick 取决于事件所在阶段，详见
[16 — Gameplay 执行模块](./16-gameplay-execution.md#同-tick-与下一-tick-边界)。

## 模式 6：免疫授予

一个效果，授予对任意来源、带 Stun asset tag 的效果的免疫。

```rust
let stun_immunity = Arc::new(GameplayEffect::new(
    vec![],  // 无修饰器——纯粹授予免疫
    EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(60.0)),
    None,
    1.0,
    StackingPolicy::non_stacking(),
    EffectTags::new(
        vec![immunity_tag],
        vec![],
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        vec![GameplayEffectImmunityQuery::new(
            TagRequirements::new(vec![], vec![])?,           // 任意来源
            TagRequirements::new(vec![stun_tag], vec![])?,   // 带 Stun 标签的效果
        )],
        vec![],
    ),
));
```

## 模式 7：基于快照的伤害计算

伤害随施法时刻的施法者攻击力缩放。

```rust
struct FireballDamageCalc {
    attack_id: AttributeId,
}

impl ModifierMagnitudeCalculation for FireballDamageCalc {
    fn calculate(&self, context: &EffectContext) -> f32 {
        let attack = context
            .source_snapshot()
            .and_then(|snapshot| {
                snapshot
                    .get_current_value(context.attribute_id_manager(), self.attack_id)
                    .ok()
                    .flatten()
            })
            .unwrap_or(0.0);
        let level = context.level() as f32;

        // Base 50 + 150% attack + 10 per ability level.
        -(50.0 + attack * 1.5 + level * 10.0)
    }
}
```

`AbilityActivationContext::direct()` 默认不捕获快照。模式 1 的 producer 在入队前调用
`AttributeSet::make_snapshot()` 并通过 `with_source_snapshot()` 附加结果；若省略这一步，上面的
安全 fallback 会把攻击力视为 0，而不是读取施法者当前属性。

## 模式 8：条件效果移除

一个效果在目标获得特定标签时自动移除自身。

```rust
EffectTags::new(
    /* ... */,
    TagRequirements::default(),  // source_removal
    TagRequirements::new(        // target_removal：目标有 Dead 标签时移除
        vec![dead_tag],
        vec![],
    )?,
    /* ... */
)
```
