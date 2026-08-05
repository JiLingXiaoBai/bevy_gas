# 08 — 技能任务

## 概述

技能任务在活跃技能内部编排基于时间的行为。它们在技能激活时生成，并在 `AbilityTasks` 系统集中每个 `FixedUpdate` tick 一次。

## 任务定义 (`AbilityTaskDef`)

```rust
pub enum AbilityTaskDef {
    Instant {
        on_finished: AbilityTaskOnFinishedDef,
    },
    WaitTicks {
        ticks: u32,
        on_finished: AbilityTaskOnFinishedDef,
    },
}
```

| 变体        | 行为                  |
| ----------- | --------------------- |
| `Instant`   | 同一 tick 内立即完成  |
| `WaitTicks` | 等待 N 个 tick 后完成 |

### `AbilityTaskOnFinishedDef`

定义任务完成时的行为：

```rust
pub enum AbilityTaskOnFinishedDef {
    None,
    EndAbility,
    EmitEvent { event_id: UniqueName },
    ApplyGameplayEffectToTarget { effect: Arc<GameplayEffect> },
    ApplyGameplayEffectToTargets { effect: Arc<GameplayEffect> },
    ActivateAbility { handle: AbilitySpecHandle },
}
```

| 变体                          | 完成时的效果                          |
| ----------------------------- | ------------------------------------- |
| `None`                        | 无操作                                |
| `EndAbility`                  | 将父技能状态设为 `Ending`             |
| `EmitEvent`                   | 触发 `AbilityTaskEvent`（Bevy Event） |
| `ApplyGameplayEffectToTarget` | 向目标队列一个 Gameplay 效果          |
| `ApplyGameplayEffectToTargets`| 向 Target Data 中每个实体队列效果      |
| `ActivateAbility`             | 队列一个链式技能激活                  |

## 运行时任务 (`AbilityTask`)

作为实体生成的 `Component`：

```rust
#[derive(Component, Clone)]
pub struct AbilityTask {
    active_ability: ActiveAbilityHandle,
    kind: AbilityTaskKind,
    on_finished: AbilityTaskOnFinished,
}

pub enum AbilityTaskKind {
    Instant,
    WaitTicks { remaining_ticks: u32 },
}
```

### `AbilityTaskOnFinished`（运行时）

```rust
pub enum AbilityTaskOnFinished {
    None,
    EndAbility,
    EmitEvent { source, target, spec_handle, event_id, level },
    ActivateAbility { source, target, handle },
    ApplyGameplayEffect { source, target, effect, level },
}
```

## 任务 Tick 系统

`tick_ability_tasks_system` 在每个 `FixedUpdate` 运行：

1. 遍历所有 `AbilityTask` 实体
2. 跳过父技能不处于 `Active` 状态的任务
3. 调用 `task.tick()` — 完成时返回 `true`
4. 完成时执行 `on_finished` 动作
5. 销毁任务实体

### `AbilityTaskEvent`

```rust
#[derive(Event, Clone)]
pub struct AbilityTaskEvent {
    source: Entity,
    target: Entity,
    active_ability: ActiveAbilityHandle,
    spec_handle: AbilitySpecHandle,
    event_id: UniqueName,
    level: u32,
}
```

当 `EmitEvent` 任务完成时发出。游戏代码可监听这些事件，在技能时间线的特定节点触发自定义逻辑。

## 使用示例

```rust
let fireball = Arc::new(GameplayAbility::new(
    ability_tags,
    vec![
        // 任务 1：等待 5 tick（前摇），然后应用伤害效果
        AbilityTaskDef::wait_ticks(5, AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
            effect: damage_effect.clone(),
        }),
        // 任务 2：等待 10 tick，然后结束技能
        AbilityTaskDef::wait_ticks(10, AbilityTaskOnFinishedDef::EndAbility),
    ],
    cooldown,
    cost,
    activation_effects,
    false,  // end_on_activation
    false,  // allow_multiple_instances
));
```

## 任务生命周期

```
技能激活
       │
       ▼
spawn_startup_ability_tasks()
  └── 对 startup_tasks 中的每个 AbilityTaskDef：
        生成 AbilityTask 实体
       │
       ▼
每个 FixedUpdate：tick_ability_tasks_system()
  ├── Instant 任务 → 立即完成
  └── WaitTicks 任务 → 递减 remaining_ticks
        └── 当 remaining_ticks == 0 → 完成
       │
       ▼
完成时：
  ├── EndAbility → 父技能状态 = Ending
  ├── EmitEvent → 触发 AbilityTaskEvent
  ├── ApplyGameplayEffect → 队列效果应用
  └── ActivateAbility → 队列链式激活
       │
       ▼
任务实体销毁
```
