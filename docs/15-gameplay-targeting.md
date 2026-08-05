# 15 — Gameplay 目标抓取

## 概述

`gameplay_targeting` 将“寻找目标”与技能和效果执行解耦。目标定义是一组经过验证、
按顺序执行的操作；运行时通过同步 API 或 FIFO `TargetingRequestQueue` 生成
`AbilityTargetData`。

目标抓取只负责产生候选结果。`GameplayEffect` 仍然一次作用于一个实体：技能定义中的
`activation_effects` 会在该技能激活请求内部按目标顺序直接应用；
`ApplyGameplayEffectToTargets` task 则按相同顺序向统一 Gameplay FIFO 写入多个效果请求。

## 源码结构

```text
src/gas/
├── gameplay_targeting.rs                 # 领域门面与显式公共重导出
└── gameplay_targeting/
    ├── ability_target_data.rs            # 有序目标数据与命中记录
    ├── targeting_definition.rs           # 已验证的操作定义、Targetable 与配置错误
    ├── acquisition.rs                    # 同步抓取算法、候选查询与运行时错误
    ├── targeting_queue.rs                # 队列子领域门面
    └── targeting_queue/
        ├── request.rs                    # 请求输入、ID、continuation 与结果事件
        ├── queue.rs                      # FIFO Resource 与请求 ID 分配
        └── processing.rs                 # drain、同步抓取、continuation 和事件派发
```

同步抓取职责集中在 `acquisition.rs`。抓取算法与队列调度分离：同步调用只依赖
acquisition；排队调用则由 processing 复用同一抓取入口。`TargetingRequest` 保持为队列内部
类型，外部只接触请求输入、稳定 ID、continuation、结果事件和队列 Resource。

## 实体要求

范围、锥形和显式实体选择只会接受带有 `Targetable` Component 的实体。这样可以避免将
相机、UI、特效等普通 Transform 实体误选为 Gameplay 目标。

所有参与空间查询的实体需要 `GlobalTransform`：

```rust
commands.spawn((
    Targetable,
    Transform::from_xyz(3.0, 0.0, 0.0),
    GlobalTransform::default(),
    GameplayTagContainer::default(),
));
```

`SelectSelf` 是例外：来源不需要 `Targetable`，但仍需要 `GlobalTransform`。

## 目标数据

```rust
pub struct AbilityTargetData {
    origin: Vec3,
    hits: Vec<AbilityTargetHit>,
}

pub struct AbilityTargetHit {
    entity: Entity,
    position: Vec3,
    normal: Option<Vec3>,
}
```

- `origin`：此次抓取使用的世界坐标原点。
- `entity`：目标实体。
- `position`：抓取时记录的目标世界坐标。
- `normal`：可选表面法线；当前实体/范围查询返回 `None`，预留给射线和物理命中适配。

`primary_entity()` 返回排序后的第一个实体，用于兼容现有单目标激活 API；`entities()`
按照确定顺序遍历全部目标。

## 有序操作管线

每个 `TargetingDefinition` 必须以恰好一个 Selection 操作开头，之后只能执行过滤、排序
和截断：

| 类别 | 操作 | 行为 |
| ---- | ---- | ---- |
| Selection | `SelectSelf` | 选择来源 |
| Selection | `SelectExplicitEntity` | 选择请求中显式提供的实体 |
| Selection | `SelectSphere` | 选择球形范围内的 `Targetable` 实体 |
| Selection | `SelectCone` | 选择指定方向锥形范围内的实体 |
| Filter | `FilterSource` | 排除来源 |
| Filter | `FilterTags` | 使用 `TagRequirements` 过滤 |
| Filter | `RequireAttributeSet` | 要求目标具有 `AttributeSet` |
| Filter | `FilterDistance` | 要求目标不超过指定距离，适合显式目标复核 |
| Sorting | `SortByDistance` | 按距原点的距离平方排序 |
| Limit | `Limit` | 保留前 N 个目标 |

构造函数会拒绝空管线、Selection 不在首位、多个 Selection、非法半径/角度和零上限，
不会在运行时主动 panic。

### 最近三个存活敌人

```rust
let definition = Arc::new(TargetingDefinition::new(vec![
    TargetingOperation::SelectSphere { radius: 12.0 },
    TargetingOperation::FilterSource,
    TargetingOperation::FilterTags {
        requirements: TagRequirements::new(
            vec![enemy_tag, alive_tag],
            vec![untargetable_tag],
        )?,
    },
    TargetingOperation::RequireAttributeSet,
    TargetingOperation::SortByDistance {
        order: TargetingSortOrder::Ascending,
    },
    TargetingOperation::Limit { count: 3 },
])?);
```

操作顺序具有语义。例如先 `Limit` 再排序只会排序已截断的候选；通常应当先过滤、再排序、
最后限制数量。

## 同步抓取

自定义 System 可以直接调用 `acquire_targets()`。它接收：

- 来源实体；
- 捕获好的 `TargetingInput`；
- 已验证的 `TargetingDefinition`；
- 目标查询。

成功时返回非空的 `AbilityTargetData`；没有合法目标时返回
`TargetingError::NoTargetsFound`。

`TargetingInput` 保存请求提交时的 `origin`、`direction` 和可选显式目标。使用捕获值而非
执行过程中重新读取相机方向，可以清楚定义一次请求的输入快照。

## 队列抓取和技能激活

通常通过 `TargetingRequestQueue` 提交工作：

```rust
let request_id = targeting_queue.push_request(
    source,
    TargetingInput::new(origin, aim_direction),
    definition,
    TargetingContinuation::activate_ability(handle, context),
);
```

生产该请求的系统应位于 `Targeting` 之前，通常注册到公共生产阶段：

```rust
app.add_systems(
    FixedUpdate,
    queue_targeting_requests.in_set(GameplayAbilitySystemSet::RequestProducers),
);
```

每个请求无论成功或失败都会通过 `Commands::trigger()` 触发包含同一
`TargetingRequestId` 的 `TargetingResultEvent`。成功且 continuation 为 `ActivateAbility` 时，
系统还会：

1. 把完整 `AbilityTargetData` 附加到 `AbilityActivationContext`；
2. 以 `primary_entity()` 作为旧 API 的单一 `target`；
3. 向统一的 `GameplayExecutionQueue` 写入激活请求；
4. 随后触发成功结果事件。

失败时不会写入技能激活，但 Observer 仍会收到 `Err(TargetingError)`。direct continuation 在
结果事件的 deferred Observer 运行前已经入队，因此 Observer 不能撤销它；需要先审核、确认
或允许取消时，应使用 `EmitResult`，再由 Observer 决定是否生产 Gameplay 请求。

使用 `TargetingContinuation::EmitResult` 时只触发结果事件，不自动激活技能，适合 AI、
UI 或游戏专用逻辑消费。

队列在当前 `FixedUpdate` 中按 FIFO 顺序处理全部待处理请求，不会因为请求数量而隐式推迟
到后续 tick。

对每个出队请求，processing 固定按“执行同步抓取 → 成功时执行 direct continuation → 触发
结果事件”的顺序处理。随后才读取下一个请求；这保证了 continuation 与事件的相对语义没有
因文件拆分而改变。

整批请求的 direct continuation 会先按 Targeting FIFO 写入 Gameplay 队列，系统返回后才应用
deferred result triggers。因此 Observer 派生的请求排在该批所有 direct continuation 之后，
不会与每个 Targeting 请求逐个交错。

## 多目标效果

技能的 `activation_effects` 在存在 Target Data 时，会在当前技能激活请求内部按顺序直接应用给
每个实体。任务也可使用：

```rust
AbilityTaskOnFinishedDef::ApplyGameplayEffectToTargets {
    effect: damage_effect,
}
```

如果激活上下文没有 Target Data，该任务回退到 `ActiveGameplayAbility` 的旧单目标字段。
每个目标独立执行效果的应用要求、免疫、概率和堆叠检查，因此一个目标拒绝效果不会阻止
后续目标。只有 task 路径会为每个目标创建独立的统一 FIFO 请求；`activation_effects` 仍属于
当前技能激活请求的内部执行步骤。

## FixedUpdate 时序

```text
EffectTicks → AbilityTasks → RequestProducers
            │
            ▼
        Targeting
            │
            ▼
 PreGameplayConvergence
            │
            ▼
 GameplayResolve（效果 + 技能统一 FIFO）
            │
            ▼
Requirement → Cleanup → RecalculateAttributes
```

`AbilityTasks` 和 `RequestProducers` 中的系统可为当前 tick 产生目标请求；Targeting 成功后会
在同一 tick 将技能激活写入统一 FIFO。技能的 startup task 位于后续 `GameplayResolve`，此时
新建的 Targeting 请求只能等下一 tick。startup Instant 直接追加到 Gameplay FIFO 的效果或
技能请求仍由当前 drain 消费。

## 确定性

- Selection 完成后先按 `Entity::to_bits()` 建立确定的基础顺序。
- 距离排序使用 `f32::total_cmp()`，距离相同时以 Entity bits 打破平局。
- `Limit` 因此不依赖 Bevy Query 的遍历顺序。
- 多目标效果按照 `AbilityTargetData` 中的顺序入队。
- 请求 origin、锥形 direction 以及定义中的半径、距离和角度会检查有限值；锥形方向不能为
  零向量。候选实体 `GlobalTransform::translation()` 当前不会统一复验有限值。

当前空间计算仍使用 `f32`，不保证不同 CPU/平台上的位级锁步。严格锁步项目应在游戏层
使用量化或定点坐标。

## 当前边界

当前版本尚未提供：

- 射线/形状投射与物理引擎适配；
- 只有世界位置、没有实体的目标数据；
- 等待玩家确认/取消的 `WaitTargetData` 任务；
- 客户端预测、网络序列化和服务端目标复验；
- 空间索引加速。

范围选择目前扫描带 `GlobalTransform` 的查询结果。只有 Profiling 证明它成为瓶颈后，
才应接入网格、BVH 或具体物理引擎的 broad phase。
