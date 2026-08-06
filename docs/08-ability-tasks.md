# 08 — 技能任务

## 职责

Ability Task 编排活跃技能中的定时行为。startup `Instant` 在技能激活函数内直接派发；只有需要
跨 tick 保存状态的任务才以 `AbilityTask` Component 存活。目前唯一的跨 tick 类型是
`WaitTicks`，所有时间单位都是 `FixedUpdate` tick。

## 源码布局

```text
src/gas/gameplay_abilities/
├── ability_task.rs
└── ability_task/
    ├── definition.rs
    ├── state.rs
    ├── completion.rs
    └── ticking.rs
```

| 文件 | 职责 |
| ---- | ---- |
| `ability_task.rs` | 门面与显式重导出 |
| `definition.rs` | 资产定义 `AbilityTaskDef`、`AbilityTaskOnFinishedDef` 及实例化 |
| `state.rs` | 跨 tick 的 Component、任务种类和已捕获完成动作 |
| `completion.rs` | 完成分派、`AbilityTaskEvent` 和 Gameplay 请求生产 |
| `ticking.rs` | 按稳定实体顺序推进运行时任务 |

完成分派是 crate 内部入口，startup `Instant` 与运行时任务复用它；调用方不应依赖私有模块或
内部完成枚举。

## 定义层公共 API

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

使用 `AbilityTaskDef::instant(...)` 和 `AbilityTaskDef::wait_ticks(...)` 创建定义。

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

| 完成动作 | 行为 |
| -------- | ---- |
| `None` | 不产生副作用 |
| `EndAbility` | 请求把父技能状态设为 `Ending` |
| `EmitEvent` | 通过 `Commands::trigger()` 触发 `AbilityTaskEvent` Observer Event |
| `ApplyGameplayEffectToTarget` | 向统一 Gameplay FIFO 追加一个效果应用请求 |
| `ApplyGameplayEffectToTargets` | 有 Target Data 时按其顺序追加多个请求，否则回退到旧单目标 |
| `ActivateAbility` | 从父上下文派生技能链并向统一 FIFO 追加激活请求 |

任务定义实例化时会捕获 source、target、spec handle、level 和效果 `Arc`。效果请求还会从父技能
激活上下文继承 Instigator、Causer 与来源属性快照。

## 运行时状态

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

公共构造与查询 API：

| API | 作用 |
| --- | ---- |
| `AbilityTask::instant(active, action)` | 创建下次可见的 `AbilityTasks` 阶段即完成的运行时任务 |
| `AbilityTask::wait_ticks(active, ticks, action)` | 创建按 tick 递减的运行时任务 |
| `get_active_ability()` | 返回父活跃实例 Entity |
| `get_kind()` | 返回当前任务种类和剩余 tick |
| `get_on_finished()` | 返回捕获好的完成动作 |

`AbilityTaskOnFinished` 是实例化后的运行时动作。它与定义枚举分离，因为其中已经捕获具体实体、
等级、技能 Handle 和 Effect `Arc`。

## startup 与运行时任务的差异

| 场景 | 是否创建任务实体 | 首次执行时机 |
| ---- | ---------------- | ------------ |
| startup `Instant` | 否 | 激活函数内立即派发 |
| startup `WaitTicks` | 是，并设为活跃技能子实体 | 激活发生在 `GameplayResolve` 时，最早下一次 `AbilityTasks` |
| 游戏层手动生成 `AbilityTask::instant` | 是 | 该实体下一次对 `AbilityTasks` 可见时 |
| 游戏层手动生成 `AbilityTask::wait_ticks` | 是 | 该实体下一次对 `AbilityTasks` 可见时开始计数 |

`WaitTicks(0)` 和 `WaitTicks(1)` 都会在第一次被任务系统处理时完成；`WaitTicks(2)` 会在第二次
处理时完成。startup 任务从同一个激活时刻启动，是 sibling，不是前一个完成后才启动下一个的
串行时间线。

startup definitions 按定义顺序处理。遇到 `Instant EndAbility` 后停止启动后续 sibling；此前已
排入 `Commands` 的 `WaitTicks` 会随结束中的父技能在 `Cleanup` 阶段递归清理。

## Tick 算法与生命周期

`tick_ability_tasks_system` 位于 `GameplayAbilitySystemSet::AbilityTasks`，每个固定 tick 执行：

1. 收集任务 Entity，并按 `Entity::to_bits()` 排序。
2. 重新取得每个任务；若父活跃技能不存在，排队销毁任务。
3. 若父技能不是 `Active`，排队销毁任务，不执行完成动作。
4. `Instant` 立即完成；`WaitTicks` 在大于零时递减，并在结果为零时完成。
5. 完成后分派 `on_finished`，然后排队销毁任务实体。
6. `EndAbility` 会把父技能设为 `Ending`；默认插件稍后的 `Cleanup` 阶段负责 ASC bookkeeping
   和递归销毁。

任务实体顺序是确定性的 Entity bits 顺序，不是独立 FIFO。完成动作向
`GameplayExecutionQueue` 写入时保留该顺序；队列随后在 `GameplayResolve` 按 FIFO 消费。

## Event 与同 tick 边界

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

`EmitEvent` 使用 `Commands::trigger()`，应通过 `On<AbilityTaskEvent>` Observer 消费，而不是
`EventReader`。Observer 的回写时机取决于事件产生阶段：

- 运行时任务在 `AbilityTasks` 产生事件。默认插件在后续 `GameplayResolve` 前应用 deferred
  commands，因此 Observer 写入 Gameplay FIFO 的请求可在当前 fixed tick 消费。
- startup `Instant` 在 `GameplayResolve` drain 内产生事件。Observer 要等 resolver 返回后才
  运行，此时本 tick 的唯一 Gameplay 消费阶段已经结束；Observer 新写入的请求留到下一 tick。
- `ApplyGameplayEffect*` 与 `ActivateAbility` 不依赖 Observer，它们在完成分派时直接入队。若在
  resolver 内产生，会被同一次 drain 继续消费。

需要严格同 batch 的玩法副作用时，应使用直接生产 Gameplay 请求的完成动作，不要依赖
`EmitEvent` Observer 回写。

## 示例

```rust
let startup_tasks = vec![
    AbilityTaskDef::wait_ticks(
        5,
        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
            effect: damage_effect.clone(),
        },
    ),
    AbilityTaskDef::wait_ticks(10, AbilityTaskOnFinishedDef::EndAbility),
];

let ability = Arc::new(GameplayAbility::new(
    ability_tags,
    startup_tasks,
    cooldown,
    cost,
    activation_effects,
    false,
    false,
));
```

两个 `WaitTicks` 同时开始；第 5 次任务处理应用效果，第 10 次任务处理结束技能。

## 测试导航

| 测试文件 | 覆盖范围 |
| -------- | -------- |
| `tests/gas_tests/abilities/tasks.rs` | Wait tick 计数和 startup `EndAbility` 截断 sibling |
| `tests/gas_tests/abilities/lifecycle.rs` | 父技能结束时递归清理 startup task |
| `tests/gas_tests/queues_test.rs` | 缺失父实例、效果/技能入队、上下文继承和 Event payload |
| `tests/gas_tests/runtime_paths_test.rs` | startup `Instant` 同 tick 与 `WaitTicks` 跨 tick |
| `tests/gas_tests/gameplay_targeting_test.rs` | Target Data 多目标完成动作 |

继续阅读：[07 — Gameplay 技能](./07-gameplay-abilities.md)、
[16 — Gameplay 执行模块](./16-gameplay-execution.md)。
