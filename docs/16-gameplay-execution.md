# 16 — Gameplay 执行模块

## 模块定位

`gameplay_execution` 是技能激活与效果应用的统一结算入口。它把两类 Gameplay mutation
放入同一个跨类型 FIFO，并在 `FixedUpdate` 的 `GameplayResolve` 阶段完整消费。

该模块解决三个问题：

- 技能和效果之间存在唯一、可观察的先后顺序；
- 同一批次较早请求产生的 Active Effect、Tag 和技能状态对后续请求可见；
- 请求量不会改变 Gameplay 结算 tick，不会因为队列上限把伤害或 Tag 隐式推迟。

`TargetingRequestQueue` 仍是独立的目标抓取队列。它先在 `Targeting` 阶段生成目标数据，
需要继续激活技能时再向 `GameplayExecutionQueue` 写入激活请求。

## 源码布局

```text
src/gas/
├── gameplay_execution.rs
└── gameplay_execution/
    ├── request.rs
    ├── queue.rs
    └── resolver.rs
```

- `gameplay_execution.rs`：私有子模块声明和公共重导出。
- `request.rs`：两种具体请求、统一请求枚举及类型转换。
- `queue.rs`：FIFO Resource、入队 API 和链 ID 分配。
- `resolver.rs`：完整 drain、逐请求收敛和 Bevy System 包装。

两种具体请求现在都由 Execution 领域拥有。Ability System 与 Gameplay Effects 的领域门面直接
兼容重导出这两个类型，因此现有领域导入路径仍可使用，而不再保留重复的请求文件。

## 请求类型

### `AbilityActivationRequest`

捕获一次技能激活所需的稳定输入：

```rust
pub struct AbilityActivationRequest {
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
    context: AbilityActivationContext,
}
```

`source` 是技能所有者，`target` 是兼容单目标 API 的旧字段；完整多目标结果位于
`AbilityActivationContext::get_target_data()`。Targeting continuation 会把它设为
`primary_entity()`；直接入队时不做一致性验证，调用方必须自行保持两者一致。

### `GameplayEffectApplicationRequest`

捕获效果定义、目标和应用上下文：

```rust
pub struct GameplayEffectApplicationRequest {
    target: Entity,
    effect: Arc<GameplayEffect>,
    payload: EffectPayload,
}
```

`EffectPayload` 在入队时固定 source、instigator、causer、等级和可选来源属性快照，避免消费
阶段重新推断调用上下文。

### `GameplayExecutionRequest`

```rust
pub enum GameplayExecutionRequest {
    ActivateAbility(AbilityActivationRequest),
    ApplyGameplayEffect(GameplayEffectApplicationRequest),
}
```

两个具体请求都实现了到统一枚举的 `From` 转换，因此既可使用专用入队方法，也可调用
`GameplayExecutionQueue::push(request)`。

## `GameplayExecutionQueue`

```rust
#[derive(Resource)]
pub struct GameplayExecutionQueue {
    requests: VecDeque<GameplayExecutionRequest>,
    next_chain_id: u64,
}
```

主要 API：

| 方法 | 作用 |
| ---- | ---- |
| `push(request)` | 追加任意统一请求 |
| `push_activation(source, target, handle, context)` | 追加技能激活 |
| `push_chained_activation(...) -> Result` | 从父上下文派生并追加链式激活 |
| `new_root_chain(handle)` | 分配队列局部 chain ID 并创建根链上下文 |
| `push_application(target, effect, payload)` | 追加效果应用 |
| `pop()` | 取出最早请求 |
| `len()` / `is_empty()` | 查询待处理数量或空状态 |
| `clear()` | 丢弃所有待处理请求 |

Gameplay 系统通常只应生产请求。`pop()` 和 `clear()` 主要用于受控工具、测试或自定义调度；
运行时存在多个消费者会破坏统一顺序。

## FixedUpdate 阶段契约

运行时插件配置的顺序为：

```text
EffectTicks
  ├─ Duration
  ├─ Requirement
  ├─ Period
  └─ Requirement
      ↓
AbilityTasks
      ↓
RequestProducers
      ↓
Targeting
      ↓
PreGameplayConvergence
      ↓
GameplayResolve
      ↓
UpdateEffectTagRequirements
      ↓
Cleanup
      ↓
RecalculateAttributes
```

需要当前 fixed tick 生效的自定义 Gameplay 请求生产系统必须位于
`GameplayAbilitySystemSet::RequestProducers`，或以显式
`.before(GameplayAbilitySystemSet::GameplayResolve)` 的方式提供等价顺序。推荐使用公共
`RequestProducers` 集合，避免依赖内部系统名称。

```rust
#[derive(Resource)]
struct AttackSource {
    entity: Entity,
    target: Entity,
    ability: AbilitySpecHandle,
}

fn queue_attack(
    mut queue: ResMut<GameplayExecutionQueue>,
    source: Res<AttackSource>,
) {
    let chain = queue.new_root_chain(source.ability);
    let context = AbilityActivationContext::direct(source.entity, chain);
    queue.push_activation(source.entity, source.target, source.ability, context);
}

app.add_systems(
    FixedUpdate,
    queue_attack.in_set(GameplayAbilitySystemSet::RequestProducers),
);
```

如果同一集合中有多个 producer，必须用 `.chain()`、`.before()` 或 `.after()` 明确它们的
业务顺序。多个系统都可变借用队列只会迫使 Bevy 串行执行，不会自动定义稳定的语义顺序。

## 完整 drain 语义

`process_gameplay_execution_queue_system` 只在队列非空时运行。内部 resolver 按以下步骤工作：

1. 整理尚未由 `Commands` flush 的 Active Ability overlay；
2. 收敛此前标记为 dirty 的 Active Effect Requirement；
3. 从 FIFO 头部取出一个请求；
4. 执行技能激活或效果应用；
5. 收敛该请求产生的 Tag Requirement 变化；
6. 回到步骤 3，直到队列为空。

这里的“逐请求收敛”不是逐请求执行 `Commands` flush，也不是把 resolver 改成
Exclusive World system。Active Effect、Tag 与属性修改直接写入现有 Component；尚未 flush 的
Active Ability 则由 pending overlay 补足同批次可见性。deferred ECS command 仍由 Bevy 在正常的
调度边界统一应用。

执行请求时新追加的请求位于 FIFO 尾部，并由同一次 drain 继续消费。startup `Instant`
任务产生的效果或链式技能因此可以在当前激活 tick 内完成；`WaitTicks` 则创建任务实体，
由后续 `AbilityTasks` 阶段推进。

Gameplay 拒绝（例如免疫、条件不满足）记录为 debug 日志；配置或运行时状态错误记录为
error。单个请求失败不会中止后续 FIFO 请求。

## 请求之间的可见性

### Active Effect

持续效果直接存储在目标的 `ActiveGameplayEffects` Component 中，不依赖效果实体的 deferred
spawn。较早请求创建的效果会立即参与后续请求的堆叠、免疫和移除检查。

### Active Ability

技能实例仍通过 `Commands` 创建实体。resolver 使用内部 pending overlay 表示尚未 flush 的
`ActiveGameplayAbility`，使同批次链式技能能够看到并取消父技能。第一次取消会先更新状态，
避免同一个 deferred despawn 实例在同一 drain 中被重复清理。

### Tag Requirement

创建或移除 Active Effect 会标记 Requirement dirty。resolver 在每个请求后执行确定性固定点
收敛，因此队列产生的 granted Tag、ongoing requirement、removal requirement 以及跨实体
source Tag 依赖，会在下一个请求执行前达到稳定状态。

Requirement 每一轮先基于同一快照收集决策，再按稳定的实体和槽位顺序提交。非收敛循环会
确定性 fail-closed，移除参与循环的效果，而不是跨 tick 保留不稳定中间态。

### Attribute

属性修饰会设置 `AttributeSet` 的 dirty bit。后续请求如果立即读取同一属性，
`AttributeSet::get_current_value()` 会按需重算该属性；管线末尾的 `RecalculateAttributes` 则统一
清理仍未被读取的 dirty 属性。因此属性值不需要等待下一 tick 才对同批次请求可见。

## 同 tick 与下一 tick 边界

| 请求产生位置 | 消费时机 |
| ------------ | -------- |
| `AbilityTasks`、`RequestProducers` 或 `Targeting` | 当前 `FixedUpdate` |
| `GameplayResolve` drain 内直接追加 | 当前 drain |
| `GameplayResolve` 之后的系统 | 下一次 `FixedUpdate` |
| `GameplayResolve` 内 startup `Instant::EmitEvent` 的 Observer 再入队 | 下一次 `FixedUpdate` |

最后一项是 `GameplayResolve` 场景下的通知边界：`EmitEvent` 使用 `Commands::trigger`，Observer
在当前 resolver 返回、deferred command 应用后才运行，此时本 tick 的唯一消费阶段已经结束。
这不是由 `EventReader` 读取的缓冲消息。若 `WaitTicks` 在 `AbilityTasks` 发出通知，或目标结果在
`Targeting` 发出通知，Observer 在 `GameplayResolve` 前完成入队时仍可赶上当前 tick。若要求
startup 派生效果或技能严格在当前 batch 生效，应使用 `ApplyGameplayEffectToTarget`、
`ApplyGameplayEffectToTargets` 或 `ActivateAbility` 完成动作。

对于未来的 AI、寻路或异步计算也采用相同规则：结果若在 `GameplayResolve` 前进入
`RequestProducers`，则当前 tick 生效；若结果在该阶段之后才就绪，就明确属于下一 tick，
不要通过运行负载相关的隐式分帧改变边界。

## 与同步 API 的关系

以下 API 提供独立的同步调用路径，不参与全局 FIFO 排序：

- `try_activate_ability_by_handle()`：先收敛 Requirement，使用局部队列执行根激活，并在返回前
  drain 该激活产生的 startup Instant 后续请求；
- `apply_gameplay_effect()`：应用前检查当前 Requirement，并在返回前完成本次效果引起的收敛；
- `execute_gameplay_effect_plan()`：执行已准备计划，并收敛由该计划标记的变化。

这里的“同步”表示返回前完成该调用路径的逻辑结算、Requirement 收敛和本地队列 drain，
不表示数据库式原子事务：由 `Commands` 创建或销毁的实体可能尚未 flush，执行失败也不承诺
回滚此前全部副作用。初始化、测试或明确需要立即结算时可以使用这些 API。运行时如果已经
存在全局排队请求，不要在同一逻辑阶段混用同步 mutation，否则它会越过全局 FIFO 中尚未
消费的请求。

## 容量、延迟与性能

`GameplayExecutionQueue` 不设置每 tick 请求上限，也不会把超额请求推迟到下一 tick。大量请求
会增加当前 tick 的 CPU 时间，这是确定性结算语义的一部分。性能问题应通过 profiling、减少
无意义请求、合并上游玩法操作或优化数据访问解决，而不是改变 Tag、伤害和属性生效 tick。

技能链仍受 `GameplayAbilitySystemSettings::ABILITY_CHAIN_MAX_DEPTH` 和重复 handle 检查保护；
它限制的是非法链式递归，不是普通 FIFO 的吞吐量。

由于 resolver 会完整 drain，未来新增的请求类型如果能无条件再次入队自身，就可能一直占用
当前 tick。`ABILITY_CHAIN_MAX_DEPTH` 只保护技能链，不能保护其他自生产请求；新请求类型必须
证明派生过程有限，或使用针对该玩法语义的深度、重复项或环检测并返回具体错误。不要用吞吐
上限掩盖无法终止的请求图。

旧配置 `ABILITY_ACTIVATION_QUEUE_MAX_PER_TICK`、
`GAMEPLAY_EFFECT_APPLICATION_QUEUE_MAX_PER_TICK` 和 `TARGETING_REQUEST_QUEUE_MAX_PER_TICK` 已删除，
不再是可调整项。`ABILITY_CHAIN_MAX_DEPTH` 是递归安全边界，不是它们的吞吐替代项。

## 扩展新的执行请求

如果后续需要加入第三种 `GameplayExecutionRequest`：

1. 在 `gameplay_execution/request.rs` 定义只捕获稳定输入的具体请求类型；
2. 在同一文件增加统一枚举变体及 `From` 转换；
3. 按需在 `GameplayExecutionQueue` 增加语义清晰的便捷入队方法；
4. 在 resolver 中实现该变体，明确它的错误分类、请求间可见性、收敛点与终止保护；
5. 更新模块重导出、API 索引，并添加同类型 FIFO、跨类型 FIFO、派生请求和阶段边界测试。

## 测试重点

相关集成测试位于：

- `tests/gas_tests/queues_test.rs`：完整 drain、同类型和跨类型 FIFO；
- `tests/gas_tests/runtime_paths_test.rs`：阶段边界、当前 tick 与下一 tick；
- `tests/gas_tests/effects/requirements.rs` 与 `stacking.rs`：请求间 Requirement、免疫、堆叠和跨实体 Tag 可见性；
- `tests/gas_tests/abilities/chaining.rs` 与 `lifecycle.rs`：startup Instant、链式激活和 deferred cancellation。

继续阅读：

- [02 — 插件系统与生命周期](./02-plugins-and-lifecycle.md)
- [06 — Gameplay 效果](./06-gameplay-effects.md)
- [07 — Gameplay 技能](./07-gameplay-abilities.md)
- [08 — 技能任务](./08-ability-tasks.md)
- [14 — 扩展系统](./14-extending-the-system.md)
- [15 — Gameplay 目标抓取](./15-gameplay-targeting.md)
