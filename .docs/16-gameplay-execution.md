# 16 — Gameplay 执行模块

## 模块定位

`gameplay_execution` 是技能激活与效果应用的统一结算入口。它把两类 Gameplay mutation
放入同一个跨类型 FIFO，并在 `FixedUpdate` 的 `GameplayResolve` 阶段完整消费。技能请求内部
的 startup Instant 效果与链式子技能 startup 同步完成后，resolver 才消费下一个队列请求。

该模块解决四个问题：

- 技能和效果之间存在唯一、可观察的先后顺序；
- 同一批次较早请求产生的 Active Effect、Tag 和技能状态对后续请求可见；
- 请求量不会改变 Gameplay 结算 tick，不会因为队列上限把伤害或 Tag 隐式推迟；
- 请求 ID 与结构化结果连接提交方和结算方，让 UI、AI 能观察实际成功或失败。

`TargetingRequestQueue` 仍是独立的目标抓取队列。它先在 `Targeting` 阶段生成目标数据，
需要继续激活技能时再向 `GameplayExecutionQueue` 写入激活请求。

## 源码布局

```text
src/gas/
├── gameplay_execution.rs
└── gameplay_execution/
    ├── request.rs
    ├── queue.rs
    ├── resolver.rs
    └── result.rs
```

- `gameplay_execution.rs`：私有子模块声明和公共重导出。
- `request.rs`：两种具体请求、统一请求枚举及类型转换。
- `queue.rs`：FIFO Resource、入队 API、请求 ID 和链 ID 分配。
- `resolver.rs`：完整 drain、逐请求收敛、结果发布和 Bevy System 包装。
- `result.rs`：成功、玩法拒绝、运行错误，以及带请求 ID 的结果 Message。

两种具体请求由 Execution 领域拥有；Ability System 与 Gameplay Effects 的领域门面显式
重导出对应类型。

## 请求类型

### 共享的 `AbilityActivationData`

Ability 领域先用一个不可变值组合一次激活的完整公共数据：

```rust
pub struct AbilityActivationData {
    source: Entity,
    targets: AbilityActivationTargets,
    context: AbilityActivationContext,
}
```

`source` 是技能所有者；`targets` 是本次激活唯一的目标值；Context 只负责技能链、Instigator、
Causer、来源快照和激活原因。该边界让请求态与活跃态共享同一种语义，而不各自重复三组字段。
`AbilityActivationData::new(source, targets, context)` 接收
`impl Into<AbilityActivationTargets>`，并通过 `get_source()`、`get_targets()`、`get_target()` 和
`get_context()` 只读访问。它由 Ability 领域、GAS 聚合门面与 crate root 公开，不进入精简
prelude。

### `AbilityActivationRequest`

捕获一次技能激活所需的稳定输入：

```rust
pub struct AbilityActivationRequest {
    handle: AbilitySpecHandle,
    activation_data: AbilityActivationData,
}
```

其中的 targets 可由 `single(entity)` 创建，也可由 `acquired(target_data)` 创建。后者以
`Result` 返回并用
`AbilityActivationTargetsError::EmptyTargetData` 拒绝空 Target Data，因此运行时不会同时保存一个
独立 Entity 和一份可能不一致的 Target Data。

`AbilityActivationRequest::from_data(handle, activation_data)` 在已有完整值时直接转移所有权；
`get_activation_data()` 返回该值。兼容的 `AbilityActivationRequest::new()` 与入队/同步激活入口接收
`impl Into<AbilityActivationTargets>`，所以单实体调用方可以继续直接传 `Entity`；Target Data 因为
转换可能失败，必须先调用 `acquired()` 或 `TryFrom`。`get_targets()` 返回完整选择，兼容
`get_source()`、`get_target()` 和 `get_context()` 都委托给 activation data，不会重新保存字段。

这是整个激活流程的唯一完整输入类型：队列 resolver 直接把请求移动到 Ability System，
独立同步入口也会先把参数归一为同一请求。校验、commit 和 ASC startup 共享其中的数据，
不会再复制成字段相同的内部 Start Context；创建 `ActiveGameplayAbility` 时才克隆需要长期
保存的整份 `AbilityActivationData`。

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
    requests: VecDeque<(GameplayExecutionRequestId, GameplayExecutionRequest)>,
    next_request_id: u64,
    next_chain_id: u64,
}
```

主要 API：

| 方法 | 作用 |
| ---- | ---- |
| `push(request)` | 追加任意统一请求，返回 `Result<RequestId, QueueError>` |
| `push_activation(source, targets: impl Into<AbilityActivationTargets>, handle, context)` | 追加携带唯一目标值的技能激活 |
| `push_chained_activation(source, targets: impl Into<...>, ...) -> Result` | 推进父链并携带同一目标值追加链式激活 |
| `new_root_chain(handle)` | 分配队列局部 chain ID 并创建根链上下文 |
| `push_application(target, effect, payload)` | 追加效果应用 |
| `pop()` | 取出最早请求的 `(request_id, request)` |
| `len()` / `is_empty()` | 查询待处理数量或空状态 |
| `clear()` | 丢弃待处理请求，不发布结果，不复用这些请求的 ID |

所有入队方法都返回 `Result<GameplayExecutionRequestId, GameplayExecutionQueueError>`。ID 在队列
局部按接受顺序单调递增；ID 空间耗尽返回 `RequestIdExhausted`，不会回绕或接收请求。链式入队
还可能返回 `InvalidChain(AbilityChainError)`。入队错误不会产生结算结果，因为请求尚未被接受。

已有 `AbilityActivationData` 时，使用
`push(AbilityActivationRequest::from_data(handle, activation_data))` 转移所有权；
`push_activation(...)` 会将独立输入参数组装成同样的数据值，并返回请求 ID 或入队错误。

Gameplay 系统通常只应生产请求。`pop()` 和 `clear()` 主要用于受控工具、测试或自定义调度；
运行时存在多个消费者会破坏统一顺序。

默认消费者签名为：

```rust
pub fn process_gameplay_execution_queue_system(
    mut execution_queue: ResMut<GameplayExecutionQueue>,
    mut params: AbilitySystemParams,
    mut results: MessageWriter<GameplayExecutionResult>,
) {
    /* Drains the shared gameplay FIFO. */
}
```

内部 drain 是私有实现，不是游戏层应直接调用的公共 API。默认入口使用 `()` Provider，
配置 `GameplayAbilitySystemPlugin::with_additional_costs::<P>()` 后，运行时安装
`process_gameplay_execution_queue_with_costs_system::<P>`，以相同 FIFO、启动动作顺序与结果协议
同步支付游戏资源。不要同时注册两个 resolver；业务系统应相对
`GameplayAbilitySystemSet::GameplayResolve` 排序，避免依赖某个 resolver 函数的身份。直接激活和链式子技能使用传入的同一
`AbilitySystemParams<P>`；外部扣费对后续请求立即可见。具体适配协议见
[14 — 扩展系统](./14-extending-the-system.md#接入背包等额外消耗)。

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
    let targets = AbilityActivationTargets::single(source.target);
    if let Err(error) = queue.push_activation(source.entity, targets, source.ability, context) {
        error!("failed to queue attack: {error}");
    }
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

1. 从 pending overlay 移除已经由 `Commands` 应用、可通过 Query 访问的 Active Ability；
2. 收敛此前标记为 dirty 的 Active Effect Requirement；
3. 从 FIFO 头部取出一个请求；
4. 执行技能激活或效果应用；技能激活包括其同步 startup Instant 与链式子技能 startup；
5. 收敛该请求产生的 Tag Requirement 变化；
6. 发布该请求的 `GameplayExecutionResult`；
7. 回到步骤 3，直到队列为空。

这里的“逐请求收敛”不是逐请求执行 `Commands` flush，也不是把 resolver 改成
Exclusive World system。Active Effect、Tag 与属性修改直接写入现有 Component；尚未 flush 的
Active Ability 则由 pending overlay 补足同批次可见性。deferred ECS command 仍由 Bevy 在正常的
调度边界统一应用。

执行请求时通过队列 API 新追加的请求仍位于 FIFO 尾部，并由同一次 drain 继续消费。
startup `Instant` 的效果与链式激活属于当前激活请求的内部步骤，不再向全局 FIFO 追加请求。
它们按动作顺序同步结算，链式 startup 使用迭代执行栈按深度优先顺序完成；`WaitTicks` 则通过
Commands 创建任务实体，由后续 `AbilityTasks` 阶段推进。

例如全局队列为 `[Activate A, Activate B]`，A 的 startup 包含 `[Apply X, Activate C, End A]`，
顺序为 A 提交 → X 结算 → C 的提交及 startup → A 结束 → B 激活。A 与 B 的入队顺序不变，
C 是 A 的内部动作，不是插入公共队列的新请求。若 C 取消 A，A 的剩余 startup 不再执行。
这项同步保证只覆盖 startup Instant；运行时任务产生的效果与激活仍遵循公共 FIFO。

Gameplay 拒绝（例如免疫、条件不满足）记录为 debug 日志；配置或运行时状态错误记录为
error。单个请求失败不会中止后续 FIFO 请求。

## 请求结果与消费时机

默认 Runtime Plugin 注册 `GameplayExecutionResult` Message。全局 resolver 为每个已消费的请求
按 FIFO 顺序发布一个结果：

```rust
pub struct GameplayExecutionResult {
    pub request_id: GameplayExecutionRequestId,
    pub source: Entity,
    pub target: Entity,
    pub outcome: GameplayExecutionOutcome,
}

pub enum GameplayExecutionOutcome {
    Succeeded,
    Rejected(GameplayExecutionError),
    Failed(GameplayExecutionError),
}
```

`target` 对效果是应用目标，对技能是捕获的主目标；完整多目标数据仍由提交方或 Active Ability
持有。`GameplayExecutionError` 保留 `AbilityActivation(AbilityActivationError)` 或
`EffectApplication(GameplayEffectApplicationError)`，无需从日志文本解析原因。

- `Succeeded`：该请求的主操作成功。技能激活本身成功不表示每个 startup 效果或链式子激活
  都成功；这些内部步骤失败会记录日志，不改写根激活的成功结果。
  startup 内部步骤没有独立请求 ID 或结果，只有实际进入公共 FIFO 的请求才有各自的 ID 和结果。
- `Rejected`：预期的玩法拒绝，如实例限制、Cost、Cooldown 或免疫。
- `Failed`：配置或运行时状态错误，如目标缺少必需组件。失败不阻断后续请求。

消费系统使用 `MessageReader<GameplayExecutionResult>`，并显式安排在
`.after(GameplayAbilitySystemSet::GameplayResolve)`。本次结果出现时，本 tick 的 drain 已结束；
结果消费者追加的请求在下一次 FixedUpdate 执行。结果是 Bevy 缓冲 Message，须正常推进 reader，
不是持久审计日志。

同步 `try_activate_ability_by_handle()` 不向公共 FIFO 提交请求，因此不发布这个全局 Message。
调用方直接读取同步函数的 `Result`；其 startup 效果与链式子技能 startup 在同步返回前完成。
`clear()` 丢弃的请求和通过公共 `pop()` 交给自定义消费者的请求，不会自动生成 resolver 结果。

## 请求之间的可见性

### Active Effect

持续效果直接存储在目标的 `ActiveGameplayEffects` Component 中，不依赖效果实体的 deferred
spawn。较早请求创建的效果会立即参与后续请求的堆叠、免疫和移除检查。

### Active Ability

技能实例仍通过 `Commands` 创建实体。resolver 使用内部 pending overlay 表示尚未 flush 的
`ActiveGameplayAbility`，使同批次链式技能能够看到并取消父技能。第一次取消会先更新状态，
避免同一个 deferred despawn 实例在同一 drain 中被重复清理。该 overlay 是 `#[doc(hidden)]`
运行时资源，不是游戏层状态容器。

### Tag Requirement

创建或移除 Active Effect 会标记 Requirement dirty。resolver 在每个请求后执行确定性固定点
收敛，因此队列产生的 granted Tag、ongoing requirement、removal requirement 以及跨实体
source Tag 依赖，会在下一个请求执行前达到稳定状态。startup Instant 直接应用效果后也执行
相同的 dirty Requirement 收敛，使下一 startup 动作能看到稳定的标签状态。

Requirement 每一轮先基于同一快照收集决策，再按稳定的实体和槽位顺序提交。非收敛循环会
确定性 fail-closed，移除参与循环的效果，而不是跨 tick 保留不稳定中间态。

### Attribute

属性修饰会设置 `AttributeSet` 的 dirty bit。后续请求如果立即读取同一属性，
`AttributeSet::get_current_value()` 会按需重算该属性；管线末尾的 `RecalculateAttributes` 则统一
清理仍未被读取的 dirty 属性。因此属性值不需要等待下一 tick 才对同批次请求可见。

## 同 tick 与下一 tick 边界

| 请求或状态产生位置 | 默认插件下的消费时机 |
| ------------------ | -------------------- |
| `AbilityTasks`、`RequestProducers` 或 Targeting direct continuation 写入 Gameplay FIFO | 当前 `FixedUpdate` |
| `TargetingResultEvent` Observer 写入 Gameplay FIFO | deferred trigger 在 resolver 前应用，当前 `FixedUpdate` |
| `GameplayResolve` drain 内直接追加 Gameplay 请求 | 当前 drain，按 FIFO 排队 |
| 激活请求内部的 startup Instant 效果或链式激活 | 同步完成效果或子技能 startup 后才进入下一动作及下一个队列请求 |
| `GameplayResolve` 创建 startup `WaitTicks` | `AbilityTasks` 已结束，下一次 `FixedUpdate` 才开始推进 |
| `GameplayResolve` 内 startup `Instant::EmitEvent` 的 Observer 写入 Gameplay FIFO | resolver 已结束，下一次 `FixedUpdate` |
| `GameplayResolve` 之后的系统写入 Gameplay FIFO | 下一次 `FixedUpdate` |
| `GameplayExecutionResult` 消费者写入 Gameplay FIFO | 消费者在 resolver 后运行，下一次 `FixedUpdate` |
| `TargetingResultEvent` Observer 再写 Targeting FIFO | Targeting drain 已结束，下一次 `FixedUpdate` 抓取 |

startup Event 项是 `GameplayResolve` 场景下的通知边界：`EmitEvent` 使用 `Commands::trigger`，Observer
在当前 resolver 返回、deferred command 应用后才运行，此时本 tick 的唯一消费阶段已经结束。
这不是由 `EventReader` 读取的缓冲消息。若 `WaitTicks` 在 `AbilityTasks` 发出通知，或目标结果在
`Targeting` 发出通知，Observer 在 `GameplayResolve` 前完成入队时仍可赶上当前 tick。若要求
startup 派生效果或技能严格在当前 batch 生效，应使用 `ApplyGameplayEffectToTarget`、
`ApplyGameplayEffectToTargets` 或 `ActivateAbility` 完成动作。

`EffectTicks` 位于 resolver 之前。因此本 tick 在 `GameplayResolve` 新建的 Duration/Period Effect
不会倒退补 tick；它第一次参与 duration/period 推进是在下一次 `FixedUpdate`。同理，resolver
新建的 Ability Task 不会倒退到本 tick 已结束的 `AbilityTasks`。

对于未来的 AI、寻路或异步计算也采用相同规则：结果若在 `GameplayResolve` 前进入
`RequestProducers`，则当前 tick 生效；若结果在该阶段之后才就绪，就明确属于下一 tick，
不要通过运行负载相关的隐式分帧改变边界。

## 与同步 API 的关系

以下 API 提供独立的同步调用路径，不参与全局 FIFO 排序：

- `try_activate_ability_by_handle()`：先收敛 Requirement，再通过迭代执行栈完成根激活及其
  startup Instant 效果和链式子技能 startup；
- `apply_gameplay_effect()`：应用前检查当前 Requirement，并在返回前完成本次效果引起的收敛；
- `execute_gameplay_effect_plan()`：执行已准备计划，并收敛由该计划标记的变化。

Effect 同步入口接收 `&mut EffectSystemParams`；Ability 同步入口接收
`&mut AbilitySystemParams`。后者通过 `DerefMut` 可借用为前者，但 Effect 模块本身不查询 ASC：

```rust
pub fn apply_gameplay_effect(
    target: Entity,
    effect_def: &Arc<GameplayEffect>,
    params: &mut EffectSystemParams,
    payload: &EffectPayload,
) -> Result<(), GameplayEffectApplicationError>;

pub fn execute_gameplay_effect_plan(
    plan: GameplayEffectApplicationPlan,
    params: &mut EffectSystemParams,
) -> Result<(), GameplayEffectApplicationError>;
```

这里的“同步”表示返回前完成该调用路径的逻辑结算与 Requirement 收敛；Ability 激活路径还会
完成整条 startup 链，但不会等待子技能的 WaitTicks。它不表示数据库式原子事务：由 `Commands`
创建或销毁的实体可能尚未 flush，执行失败也不承诺回滚此前全部副作用。初始化、测试或明确
需要立即结算时可以使用这些 API。运行时如果已经存在全局排队请求，不要在同一逻辑阶段混用
同步 mutation，否则它会越过
全局 FIFO 中尚未消费的请求。

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

- `tests/gas_test/queues_test.rs`：完整 drain、同类型和跨类型 FIFO、结果 ID/顺序/失败分类、结果驱动请求的下一 tick 边界，以及同步入口不发布全局结果；
- `tests/gas_test/runtime_paths_test.rs`：阶段边界、当前 tick 与下一 tick；
- `tests/gas_test/effects_test/requirements_test.rs` 与 `stacking_test.rs`：请求间 Requirement、免疫、堆叠和跨实体 Tag 可见性；
- `tests/gas_test/abilities_test/chaining_test.rs` 与 `lifecycle_test.rs`：startup Instant、链式激活和 deferred cancellation。
- `tests/gas_test/gameplay_targeting_test.rs`：Targeting FIFO、direct continuation 与多目标请求。

继续阅读：

- [02 — 插件系统与生命周期](./02-plugins-and-lifecycle.md)
- [06 — Gameplay 效果](./06-gameplay-effects.md)
- [07 — Gameplay 技能](./07-gameplay-abilities.md)
- [08 — 技能任务](./08-ability-tasks.md)
- [14 — 扩展系统](./14-extending-the-system.md)
- [15 — Gameplay 目标抓取](./15-gameplay-targeting.md)
