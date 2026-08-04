# 12 — 使用模式与示例

## 模式 1：直接伤害技能

一个火球术，经过短暂前摇后造成即时伤害。

```rust
// 1. 定义伤害效果 (Instant)
let damage_effect = Arc::new(GameplayEffect::new(
    vec![Modifier::new(
        health_id,
        ModifierOperation::Add,
        ModifierMagnitude::Calculated(Box::new(FireballDamageCalc)),
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

// 2. 定义火球技能
let fireball = Arc::new(GameplayAbility::new(
    AbilityTags::new(
        vec![fireball_tag],
        vec![],
        vec![],
        vec![],
        vec![stun_tag],  // 眩晕时阻止
    ),
    vec![
        // 前摇：等待 5 tick，然后应用伤害
        AbilityTaskDef::wait_ticks(5,
            AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                effect: damage_effect.clone(),
            },
        ),
        // 伤害后结束
        AbilityTaskDef::instant(AbilityTaskOnFinishedDef::EndAbility),
    ],
    Some(cooldown_effect),  // 30 tick 冷却
    Some(cost_effect),      // 25 法力消耗
    vec![],                 // 无激活效果
    false,                  // 激活时不结束
    false,                  // 仅单实例
));

// 3. 授予并激活
fn setup_fireball(mut params: AbilitySystemParams, player: Entity) {
    if let Ok(mut asc) = params.asc_query.get_mut(player) {
        asc.give_ability(fireball.clone(), 1, Some(0));
    }
}

fn cast_fireball(
    mut params: AbilitySystemParams,
    mut queue: ResMut<AbilityActivationQueue>,
    player: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
) {
    let chain = queue.new_root_chain(handle);
    let context = AbilityActivationContext::direct(player, chain);
    queue.push_activation(player, target, handle, context);
}
```

## 模式 2：持续伤害 (DoT)

一个中毒效果，每 3 tick 造成 10 点伤害，持续 15 tick。

```rust
let poison_effect = Arc::new(GameplayEffect::new(
    vec![Modifier::new(
        health_id,
        ModifierOperation::Add,
        ModifierMagnitude::Flat(-10.0),  // 每 tick 10 伤害
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
        AbilityTaskDef::instant(
            AbilityTaskOnFinishedDef::ActivateAbility {
                handle: hit2_spec_handle,
            },
        ),
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
        AbilityTaskDef::instant(
            AbilityTaskOnFinishedDef::ActivateAbility {
                handle: hit3_spec_handle,
            },
        ),
    ],
    /* ... */
));
```

## 模式 5：事件驱动技能逻辑

使用 `AbilityTaskEvent` 在技能时间线的特定节点触发自定义游戏逻辑。

```rust
fn observe_ability_events(
    mut reader: EventReader<AbilityTaskEvent>,
) {
    for event in reader.read() {
        match event.get_event_id() {
            // 特定任务完成时的自定义逻辑
            id if id == my_custom_event_id => {
                // 生成 VFX、播放音效等
            }
            _ => {}
        }
    }
}

// 在技能定义中：
AbilityTaskDef::wait_ticks(10,
    AbilityTaskOnFinishedDef::EmitEvent {
        event_id: my_custom_event_id,
    },
)
```

## 模式 6：免疫授予

一个效果，授予对特定来源眩晕效果的免疫。

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
struct FireballDamageCalc;

impl ModifierMagnitudeCalculation for FireballDamageCalc {
    fn calculate(&self, context: &EffectContext) -> f32 {
        let snapshot = context.source_snapshot()
            .expect("火球术需要来源快照");

        let attack = snapshot
            .get_current_value(context.attribute_id_manager(), attack_id)
            .ok()
            .flatten()
            .unwrap_or(0.0);
        let level = context.level() as f32;

        // 基础 50 + 150% 攻击力 + 每级 10
        -(50.0 + attack * 1.5 + level * 10.0)
    }
}
```

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
