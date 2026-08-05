# 08 — 技能任务

## 概述

技能任务在活跃技能内部编排行为。startup `Instant` 在技能激活流程中直接派发；只有需要
跨 tick 保存状态的任务（当前为 `WaitTicks`）才生成实体，并由 `AbilityTasks` 系统推进。

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
| `Instant`   | 作为 startup definition 时在激活调用内完成 |
| `WaitTicks` | 创建运行时任务，之后等待 N 个 `AbilityTasks` tick |

startup tasks 是同时启动的 sibling，不是依次等待的序列。多个 `WaitTicks` 使用相对激活时刻的
绝对等待点；前一个任务完成不会自动触发后一个任务开始。

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
| `EmitEvent`                   | 通过 `Commands::trigger` 触发 Observer Event |
| `ApplyGameplayEffectToTarget`  | 向统一 Gameplay FIFO 加入一个效果请求        |
| `ApplyGameplayEffectToTargets` | 按 Target Data 顺序加入多个效果请求          |
| `ActivateAbility`              | 向统一 Gameplay FIFO 加入链式技能激活        |

## 运行时任务 (`AbilityTask`)

需要跨 tick 的任务作为实体 `Component` 存在。`AbilityTask::instant(...)` 仍可供游戏层手动
生成，但 startup `AbilityTaskDef::Instant` 不会生成短命实体。

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
    ApplyGameplayEffectToTargets { source, fallback_target, effect, level },
}
```

手动 spawn 的 `AbilityTask::instant(...)` 仍是运行时任务，必须等实体对下一次
`AbilityTasks` 阶段可见后才会完成；只有 startup `AbilityTaskDef::Instant` 是 activation-inline。
startup `WaitTicks` 也通过 deferred spawn 创建，`WaitTicks(0)` 最早在后续 `FixedUpdate` 的
`AbilityTasks` 阶段完成。

## 任务 Tick 系统

`tick_ability_tasks_system` 在每个 `FixedUpdate` 运行：

1. 按任务实体的 `Entity::to_bits()` 稳定顺序遍历 `AbilityTask`
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

当 `EmitEvent` 任务完成时通过 `Commands::trigger()` 发出。游戏代码应注册
`On<AbilityTaskEvent>` Observer，在技能时间线的特定节点触发自定义逻辑；它不是由
`EventReader` 消费的 buffered Message。

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
start_startup_ability_tasks()
  └── 对 startup_tasks 中的每个 AbilityTaskDef：
        ├── Instant → 激活流程内直接派发完成动作
        └── WaitTicks → 生成 AbilityTask 实体
       │
       ▼
每个 FixedUpdate：tick_ability_tasks_system()
  └── WaitTicks 任务 → 递减 remaining_ticks
        └── 当 remaining_ticks == 0 → 完成
       │
       ▼
完成时：
  ├── EndAbility → 父技能状态 = Ending
  ├── EmitEvent → 触发 AbilityTaskEvent
  ├── ApplyGameplayEffect → 写入 GameplayExecutionQueue
  ├── ApplyGameplayEffectToTargets → 按目标顺序写入多个请求
  └── ActivateAbility → 写入 GameplayExecutionQueue
       │
       ▼
任务实体销毁
```

startup Instant 产生的效果或链式激活会追加到当前 `GameplayResolve` 正在 drain 的统一 FIFO，
因此在技能激活所在 tick 内执行。若 startup Instant 在全局 resolver 内 `EmitEvent`，Observer
只能在 resolver 返回后的 deferred sync point 运行，此时回写 Gameplay FIFO 的请求属于下一
tick。由 `AbilityTasks` 完成的 `WaitTicks::EmitEvent` 位于 resolver 之前，Observer 默认仍可在
当前 tick 入队。需要严格同 batch 顺序的 startup 玩法逻辑应直接建模为 Gameplay 请求，而
不是依赖通知事件回写队列。

startup tasks 按定义顺序派发；遇到 `Instant EndAbility` 后停止启动后续 sibling task。此前已经
spawn 的 `WaitTicks` 也会随结束中的父技能清理，不会形成“等待完成后再结束”的序列。
公共同步入口 `try_activate_ability_by_handle()` 会使用局部 batch 完整处理 Instant 派生请求，
而通过全局队列激活时则由当前 `GameplayResolve` drain 完成同样语义。
