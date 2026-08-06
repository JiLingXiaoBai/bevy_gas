# 15 — Gameplay 目标抓取

## 职责与源码布局

`gameplay_targeting` 负责从 ECS 状态生成确定有序的 `AbilityTargetData`。它不直接执行技能或
效果；`ActivateAbility` continuation 只把激活请求写入统一 Gameplay FIFO。

```text
src/gas/
├── gameplay_targeting.rs
└── gameplay_targeting/
    ├── ability_target_data.rs
    ├── targeting_definition.rs
    ├── acquisition.rs
    ├── targeting_queue.rs
    └── targeting_queue/
        ├── request.rs
        ├── queue.rs
        └── processing.rs
```

| 文件 | 职责 |
| ---- | ---- |
| `ability_target_data.rs` | 有序命中与目标数据 |
| `targeting_definition.rs` | 操作定义、验证错误和 `Targetable` |
| `acquisition.rs` | 同步选择/过滤算法、候选 Query 和运行时错误 |
| `targeting_queue/request.rs` | 输入快照、稳定 ID、continuation 与结果 Event |
| `targeting_queue/queue.rs` | 私有请求的 FIFO Resource 与 ID 分配 |
| `targeting_queue/processing.rs` | 完整 drain、continuation 和 Event 派发 |

`TargetingRequest` 是私有实现。公共调用方只使用输入、定义、continuation、请求 ID、结果 Event
和队列 Resource。

## 目标数据

```rust
pub struct AbilityTargetHit {
    entity: Entity,
    position: Vec3,
    normal: Option<Vec3>,
}

pub struct AbilityTargetData {
    origin: Vec3,
    hits: Vec<AbilityTargetHit>,
}
```

`AbilityTargetHit::new()` 和 `AbilityTargetData::new()` 可用于游戏层适配器。常用只读 API：

| API | 作用 |
| --- | ---- |
| `get_origin()` | 返回请求捕获的世界坐标原点 |
| `get_hits()` | 返回确定有序的全部 Hit |
| `primary_entity()` | 返回第一个目标，兼容旧单目标技能 API |
| `entities()` | 按 Hit 顺序遍历实体 |
| `len()` / `is_empty()` | 查询目标数量 |

同步 acquisition 成功时保证数据非空；游戏层通过 `AbilityTargetData::new()` 手工创建的数据可以
为空。

## 定义与实体要求

每个 `TargetingDefinition` 必须以恰好一个 Selection 开头，后续操作按给定顺序细化结果：

| 类别 | 操作 | 行为 |
| ---- | ---- | ---- |
| Selection | `SelectSelf` | 选择来源；不要求 `Targetable` |
| Selection | `SelectExplicitEntity` | 选择输入中的显式实体；要求 `Targetable` |
| Selection | `SelectSphere` | 选择球形范围内的 `Targetable` 实体 |
| Selection | `SelectCone` | 选择方向锥形范围内的 `Targetable` 实体 |
| Filter | `FilterSource` | 排除来源 |
| Filter | `FilterTags` | 使用 `TagRequirements` 过滤 |
| Filter | `RequireAttributeSet` | 要求目标具有 `AttributeSet` |
| Filter | `FilterDistance` | 要求距请求原点不超过上限 |
| Sorting | `SortByDistance` | 按距离平方升序或降序排序 |
| Limit | `Limit` | 截断为前 N 个 |

`TargetingDefinition::new()` 拒绝空操作、Selection 不在首位、多个 Selection、负数或非有限
radius/distance、非有限或超出 `[0, PI]` 的半角，以及 `Limit { count: 0 }`。

操作顺序具有语义。通常应先过滤，再排序，最后 Limit；先 Limit 再排序只会重排已截断集合。

候选 Query 要求实体有 `GlobalTransform`。Sphere、Cone 和 Explicit selection 还要求
`Targetable`；`SelectSelf` 只要求来源有 `GlobalTransform`。`FilterTags` 配置非空时，没有
`GameplayTagContainer` 的候选不会通过；空 requirements 对有无容器都通过。

## 同步抓取 API

```rust
pub fn acquire_targets(
    source: Entity,
    input: TargetingInput,
    definition: &TargetingDefinition,
    query: &TargetingCandidateQuery,
) -> Result<AbilityTargetData, TargetingError>;
```

`TargetingInput::new(origin, direction)` 捕获请求提交时的空间输入，
`with_explicit_target(entity)` 附加显式目标。Origin 总会检查有限值；Direction 仅在 Cone selection
需要时检查有限且非零。缺失实体/Component、无效输入和最终空集合都会返回具体
`TargetingError`，不会 panic。

## 队列 API 与 continuation

```rust
pub fn push_request(
    &mut self,
    source: Entity,
    input: TargetingInput,
    definition: Arc<TargetingDefinition>,
    continuation: TargetingContinuation,
) -> TargetingRequestId;
```

`TargetingRequestQueue` 还公开 `len()`、`is_empty()` 与 `clear()`；出队只属于内部 processor。
请求 ID 从 1 开始，按入队顺序分配，溢出后跳过 0。

```rust
pub enum TargetingContinuation {
    EmitResult,
    ActivateAbility {
        handle: AbilitySpecHandle,
        context: Box<AbilityActivationContext>,
    },
}
```

推荐使用 `TargetingContinuation::activate_ability(handle, context)` 创建激活 continuation。

processor 完整 drain 当前 Targeting FIFO。每个请求严格执行：

1. 调用同步 `acquire_targets()`。
2. 成功且 continuation 为 `ActivateAbility` 时，把完整 Target Data 附加到 Context，以
   `primary_entity()` 作为旧单目标字段，并写入 `GameplayExecutionQueue`。
3. 通过 `Commands::trigger()` 排队触发包含同一 `TargetingRequestId` 的
   `TargetingResultEvent`，无论成功或失败都会触发。
4. 处理下一个 Targeting 请求。

Direct continuation 在结果 Observer 之前已经入队，Observer 不能撤销它。需要审核、玩家确认
或取消时使用 `EmitResult`，再由 `On<TargetingResultEvent>` Observer 决定是否生产 Gameplay
请求。

整批 direct continuations 都在 processor 内完成；deferred result triggers 在系统返回后才应用，
所以 Observer 派生的 Gameplay 请求排在该批所有 direct continuation 之后，不会逐请求交错。

## FixedUpdate 时序与边界

```text
EffectTicks
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

| 产生位置 | 结果 |
| -------- | ---- |
| `AbilityTasks` / `RequestProducers` 或其他明确位于 `Targeting` 前的系统写入 Targeting FIFO | 当前 tick 抓取 |
| Targeting processor 的 direct activation continuation | 当前 tick `GameplayResolve` |
| `TargetingResultEvent` Observer 写入 Gameplay FIFO | 默认插件下仍在 `GameplayResolve` 前，当前 tick |
| `TargetingResultEvent` Observer 再写 Targeting FIFO | processor 已结束，下一 tick 抓取 |
| `Targeting` 阶段之后新写 Targeting FIFO | 下一 tick 抓取 |

队列没有每 tick 数量上限，会完整处理进入本次 drain 的请求。Observer 在 processor 返回后新增
的 Targeting 请求不会被本次已经结束的 drain 重新消费。

## 多目标效果

- 技能 `activation_effects` 在激活请求内部按效果定义顺序、再按 Target Data 顺序直接应用。
- `ApplyGameplayEffectToTargets` task 按 Target Data 顺序向统一 Gameplay FIFO 追加独立请求。
- Task 只有在 Context 没有 Target Data 时才回退到旧单目标；手工提供空 Target Data 会产生零个
  多目标请求。
- 每个目标独立进行要求、免疫、概率和堆叠检查；一个目标拒绝不会中断后续 FIFO 请求。

## 确定性与当前边界

- Sphere/Cone selection 先按 `Entity::to_bits()` 建立基础顺序。
- 距离排序使用 `f32::total_cmp()`，相同距离用 Entity bits 打破平局。
- 请求捕获 Origin、Direction 和显式目标，不在消费时重新读取输入设备。
- 候选 `GlobalTransform::translation()` 当前没有统一复验有限值。
- 空间计算仍使用 `f32`，不保证跨平台位级锁步。
- 当前没有物理射线/形状投射、纯世界位置目标、玩家确认任务、网络预测/复验或空间索引。

范围查询目前扫描候选 Query；只有 Profiling 证明它成为瓶颈后再接入网格、BVH 或物理 broad
phase。

## 测试导航

| 测试文件 | 覆盖范围 |
| -------- | -------- |
| `tests/gas_tests/gameplay_targeting_test.rs` | 定义验证、Sphere/Cone/Explicit、排序、完整 drain、Target Data 激活和多目标效果 |
| `tests/gas_tests/runtime_paths_test.rs` | 默认 FixedUpdate 阶段与 Gameplay 同 tick 边界 |
| `tests/gas_tests/queues_test.rs` | Task 派生 Gameplay 请求的 FIFO 行为 |

继续阅读：[07 — Gameplay 技能](./07-gameplay-abilities.md)、
[16 — Gameplay 执行模块](./16-gameplay-execution.md)。
