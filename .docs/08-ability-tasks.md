# 08 — 技能任务

## 职责

Ability Task 编排活跃技能中的定时行为。startup `Instant` 的效果与链式激活在激活阶段同步
结算，不创建任务实体。`WaitTicks` 以 `AbilityTask` Component 保存等待状态；游戏层也可手动
创建运行时 Instant 任务，由后续任务系统处理。所有时间单位都是 `FixedUpdate` tick。

## 源码布局

```text
src/gas/gameplay_abilities/
├── ability_task.rs
└── ability_task/
    ├── context.rs
    ├── definition.rs
    ├── state.rs
    ├── completion.rs
    └── ticking.rs
```

| 文件 | 职责 |
| ---- | ---- |
| `ability_task.rs` | 门面与显式重导出 |
| `context.rs` | 公开的任务共享执行上下文 |
| `definition.rs` | 资产定义 `AbilityTaskDef`、`AbilityTaskOnFinishedDef` 及实例化 |
| `state.rs` | 跨 tick 的 Component、任务种类和仅保存动作数据的完成枚举 |
| `completion.rs` | 完成分派、`AbilityTaskEvent` 和 Gameplay 请求生产 |
| `ticking.rs` | 按稳定实体顺序推进运行时任务 |

startup `Instant` 由 `ability_system/activation` 的迭代执行栈处理；运行时任务由
`completion.rs` 生产 Gameplay 请求。两条路径共享动作定义、目标和 payload 语义，但执行时机
不同。调用方不应依赖私有模块或内部执行状态。

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
    Batch { actions: Vec<AbilityTaskOnFinishedDef> },
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
| `Batch` | 按定义顺序派发子动作，遇到第一个 `EndAbility` 停止 |
| `None` | 不产生副作用 |
| `EndAbility` | 请求把父技能状态设为 `Ending` |
| `EmitEvent` | 通过 `Commands::trigger()` 触发 `AbilityTaskEvent` Observer Event |
| `ApplyGameplayEffectToTarget` | 对主目标应用效果：startup Instant 同步结算，运行时任务追加 FIFO 请求 |
| `ApplyGameplayEffectToTargets` | 按捕获目标顺序应用效果：startup Instant 逐个同步结算，运行时任务逐个入队 |
| `ActivateAbility` | 从父上下文派生技能链：startup Instant 先完成子技能 startup，运行时任务追加 FIFO 请求 |

任务定义实例化时，source、spec handle 和 level 只存入一个 `AbilityTaskExecutionContext`。
`AbilityTaskOnFinished` 仅保存动作专属数据，例如 event ID、Ability handle 或 Effect `Arc`。
Task 实例不持有完整 `AbilityActivationTargets`：startup Instant 完成动作从 Request 的
`AbilityActivationData` 借用，跨 tick task 完成动作从父 `ActiveGameplayAbility` 的同类数据
借用，避免每个 task 再保存一份可能包含 `Vec` 的抓取数据。两条路径的效果 payload 都从其中的
激活上下文继承 Instigator、Causer 与来源属性快照。

## 运行时状态

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityTaskExecutionContext {
    source: Entity,
    spec_handle: AbilitySpecHandle,
    level: u32,
}

#[derive(Component, Clone)]
pub struct AbilityTask {
    active_ability: ActiveAbilityHandle,
    context: AbilityTaskExecutionContext,
    kind: AbilityTaskKind,
    on_finished: AbilityTaskOnFinished,
}

pub enum AbilityTaskKind {
    Instant,
    WaitTicks { remaining_ticks: u32 },
}
```

`AbilityTaskExecutionContext::new(source, spec_handle, level)` 建立一份轻量、可复制的执行值；
`get_source()`、`get_spec_handle()` 和 `get_level()` 提供只读访问。目标不属于该 Context；startup
直接借用 Request activation data 的 targets，跨 tick task 则在完成时从父 Active Ability 的
activation data 读取同一 targets。

```rust
pub enum AbilityTaskOnFinished {
    Batch { actions: Vec<AbilityTaskOnFinished> },
    None,
    EndAbility,
    EmitEvent { event_id: UniqueName },
    ActivateAbility { handle: AbilitySpecHandle },
    ApplyGameplayEffect { effect: Arc<GameplayEffect> },
    ApplyGameplayEffectToTargets { effect: Arc<GameplayEffect> },
}
```

运行时枚举是 action-only；不得重复携带 source、targets、spec handle 或 level。

公共构造与查询 API：

| API | 作用 |
| --- | ---- |
| `AbilityTask::instant(active, context, action)` | 创建下次可见的 `AbilityTasks` 阶段即完成的运行时任务 |
| `AbilityTask::wait_ticks(active, context, ticks, action)` | 创建按 tick 递减的运行时任务 |
| `get_active_ability()` | 返回父活跃实例 Entity |
| `get_context()` | 返回共享执行上下文 |
| `get_kind()` | 返回当前任务种类和剩余 tick |
| `get_on_finished()` | 返回动作专属的完成数据 |

`AbilityTaskOnFinished` 与定义枚举分离，因为它保存已实例化的 Effect `Arc` 或其他动作
参数；与所有动作共享的执行数据由 `AbilityTask` 统一持有。

## startup 与运行时任务的差异

| 场景 | 是否创建任务实体 | 首次执行时机 |
| ---- | ---------------- | ------------ |
| startup `Instant` | 否 | 激活阶段逐动作同步结算效果与链式子技能 startup；事件通知仍 deferred |
| startup `WaitTicks` | 是，并设为活跃技能子实体 | 激活发生在 `GameplayResolve` 时，最早下一次 `AbilityTasks` |
| 游戏层手动生成 `AbilityTask::instant` | 是 | 该实体下一次对 `AbilityTasks` 可见时 |
| 游戏层手动生成 `AbilityTask::wait_ticks` | 是 | 该实体下一次对 `AbilityTasks` 可见时开始计数 |

`WaitTicks(0)` 和 `WaitTicks(1)` 都会在第一次被任务系统处理时完成；`WaitTicks(2)` 会在第二次
处理时完成。startup WaitTicks 从同一个激活时刻开始计时，是 sibling，不是前一个等待完成后
才启动下一个的串行时间线；WaitTicks(0) 也不会改为激活阶段同步执行。

startup definitions 按定义顺序处理。每个 Instant 效果结算并收敛标签 Requirement 后，才进入
下一动作；ActivateAbility 先完成子技能及其链式 startup，再恢复父技能。实现使用迭代执行栈，
不递归调用技能激活函数。子技能若取消父技能，父技能恢复时会停止剩余 startup，并保留取消状态。

遇到 `Instant EndAbility` 后停止当前技能的剩余 Batch 动作和后续 sibling；此前已排入
`Commands` 的 `WaitTicks` 会随结束中的父技能在 `Cleanup` 阶段递归清理。子技能的 EndAbility
只结束该子技能，不截断仍处于 Active 的父技能后续动作。

## 同一任务内的顺序动作

同一完成点需要多个有序动作时，将它们放入一个 `Batch`。嵌套 Batch 按深度优先顺序展开；
同一技能内任意子动作的 `EndAbility` 会截断整个外层 Batch，空 Batch 不产生副作用。

- startup Instant 的 `[ApplyEffect, EndAbility]` 先尝试实际应用效果，再结束技能。
- WaitTicks 或手动创建的运行时 Instant 使用相同 Batch 时，ApplyEffect 先入队，EndAbility
  随后将父技能标记为 Ending；效果稍后由 GameplayResolve 结算，已入队请求不会撤回。

两条路径都不提供事务回滚。单个效果或链式子激活失败后，仍可执行后续动作；已经成功的效果
不会回滚。Batch 也不会改变 EmitEvent 的 deferred 时机。

独立 sibling 任务仍按 Entity bits 排序，不能通过创建多个同 tick WaitTicks 表达策划的动作顺序。
游戏的配置编译层应将同一技能、同一完成 tick 的动作按明确顺序合为一个 Batch；
不同 tick 仍相对激活时点并行计时。配置与运行时的衔接见
[19 — 外部配置集成](./19-external-configuration.md)。

## Tick 算法与生命周期

`tick_ability_tasks_system` 位于 `GameplayAbilitySystemSet::AbilityTasks`，每个固定 tick 执行：

1. 收集任务 Entity，并按 `Entity::to_bits()` 排序。
2. 重新取得每个任务；若父活跃技能不存在，排队销毁任务。
3. 若父技能不是 `Active`，排队销毁任务，不执行完成动作。
4. `Instant` 立即完成；`WaitTicks` 在大于零时递减，并在结果为零时完成。
5. 完成后分派 `on_finished`，然后排队销毁任务实体。
6. `EndAbility` 会把父技能设为 `Ending`；默认插件稍后的 `Cleanup` 阶段负责 ASC bookkeeping
   和递归销毁。

空任务列表或全部任务完成都不会自动结束父技能。需要通过 `EndAbility` 动作或生命周期 API
明确结束；在第 5 tick 结束的技能，其第 6 tick 的任务不会再执行完成动作。

任务实体顺序是确定性的 Entity bits 顺序，不是独立 FIFO。完成动作向
`GameplayExecutionQueue` 写入时保留该顺序；队列随后在 `GameplayResolve` 按 FIFO 消费。

## Event 与同 tick 边界

```rust
#[derive(Event, Clone)]
pub struct AbilityTaskEvent {
    context: AbilityTaskExecutionContext,
    targets: AbilityActivationTargets,
    active_ability: ActiveAbilityHandle,
    event_id: UniqueName,
}
```

`get_context()` 返回轻量执行上下文；`get_targets()` 返回事件在触发时从父 Active Ability 的
`AbilityActivationData` 克隆出的完整目标值，`get_target()` 委托给其主目标。source、spec handle
和 level getter 委托给 task context。Event 必须拥有 targets，因为 Observer 运行时原任务或
父技能可能已进入 deferred 清理；它不会为了复用而持有整份激活数据。

`EmitEvent` 使用 `Commands::trigger()`，应通过 `On<AbilityTaskEvent>` Observer 消费，而不是
`EventReader`。Observer 的回写时机取决于事件产生阶段：

- 运行时任务在 `AbilityTasks` 产生事件。默认插件在后续 `GameplayResolve` 前应用 deferred
  commands，因此 Observer 写入 Gameplay FIFO 的请求可在当前 fixed tick 消费。
- startup `Instant` 在 `GameplayResolve` drain 内产生事件。Observer 要等 resolver 返回后才
  运行，此时本 tick 的唯一 Gameplay 消费阶段已经结束；Observer 新写入的请求留到下一 tick。
- startup `ApplyGameplayEffect*` 与 `ActivateAbility` 不依赖 Observer，在下一动作前完成效果
  结算或子技能 startup。运行时任务的对应动作仍写入 Gameplay FIFO，由 resolver 消费。

需要在 startup 下一动作之前看到玩法副作用时，应使用同步效果或链式激活动作，不要依赖
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
    false,
));
```

两个 `WaitTicks` 同时开始；第 5 次任务处理应用效果，第 10 次任务处理结束技能。

游戏层手动创建运行时任务时，应先组合共享上下文：

```rust
let context = AbilityTaskExecutionContext::new(source, spec_handle, level);

let mut task_commands = commands.spawn(AbilityTask::wait_ticks(
    active_ability,
    context,
    5,
    AbilityTaskOnFinished::ApplyGameplayEffect {
        effect: damage_effect,
    },
));
task_commands.set_parent_in_place(active_ability);
```

## 测试导航

| 测试文件 | 覆盖范围 |
| -------- | -------- |
| `tests/gas_test/abilities_test/tasks_test.rs` | Wait tick 计数、startup 同步动作、`EndAbility` 截断 sibling、嵌套 Batch 与运行时已入队效果保留 |
| `tests/gas_test/abilities_test/lifecycle_test.rs` | 父技能结束时递归清理 startup task |
| `tests/gas_test/queues_test.rs` | 缺失父实例、效果/技能入队、上下文继承和 Event payload |
| `tests/gas_test/runtime_paths_test.rs` | startup `Instant` 同 tick 与 `WaitTicks` 跨 tick |
| `tests/gas_test/gameplay_targeting_test.rs` | Target Data 多目标完成动作 |

继续阅读：[07 — Gameplay 技能](./07-gameplay-abilities.md)、
[16 — Gameplay 执行模块](./16-gameplay-execution.md)。
