# 15 — Gameplay 目标抓取

## 职责与源码布局

`gameplay_targeting` 负责从 ECS 状态生成确定有序的 `AbilityTargetData`。它不直接执行技能或
效果；它还拥有激活全过程使用的唯一目标值 `AbilityActivationTargets`。
`ActivateAbility` continuation 只把激活请求写入统一 Gameplay FIFO。

完整源码树统一维护在 [17 — 当前源码树](./17-source-layout-and-maintenance.md#当前源码树)。

| 文件 | 职责 |
| ---- | ---- |
| `error.rs` | 空激活目标、管线定义验证和运行时抓取错误 |
| `ability_target_data.rs` | 有序命中与目标数据 |
| `activation_targets.rs` | 单目标/抓取目标的统一激活值及非空约束 |
| `targeting_definition.rs` | 操作定义、管线验证和 `Targetable` |
| `acquisition.rs` | 公共 SystemParam、候选 Query 和共享选择/过滤/排序入口 |
| `spatial_index.rs` | 空间索引门面，显式重导出 Resource 与同步系统 |
| `spatial_index/index.rs` | 派生空间 Resource、增量同步与组件生命周期处理 |
| `spatial_index/tree.rs` | 私有动态 AABB 树与 Fat AABB 代理 |
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
| `primary_entity()` | 返回第一个命中实体 |
| `entities()` | 按 Hit 顺序遍历实体 |
| `len()` / `is_empty()` | 查询目标数量 |

同步 acquisition 成功时保证数据非空；游戏层通过 `AbilityTargetData::new()` 手工创建的数据可以
为空，但空值不能转换为 Acquired 激活目标。

### `AbilityActivationTargets`

```rust
pub struct AbilityActivationTargets {
    primary_target: Entity,
    target_data: Option<AbilityTargetData>,
}
```

`AbilityActivationTargets` 由 `AbilityActivationData` 持有，是 Request、Active Ability、Ability
Task 完成动作和链式激活共同使用的唯一目标语义值。字段保持私有，调用方不能绕过构造器制造
“缓存主目标与首个 Hit 不一致”的状态：

| API | 作用 |
| --- | ---- |
| `single(entity)` | 创建一个单实体目标 |
| `acquired(target_data) -> Result<Self, AbilityActivationTargetsError>` | 创建抓取目标；空数据返回 `EmptyTargetData` |
| `get_primary_target()` | 返回缓存且已验证的主目标 |
| `get_target_data()` | 单目标时返回 `None`，抓取目标时返回完整 Target Data |
| `entities()` | 按确定顺序遍历一个单目标实体或全部抓取实体 |

`From<Entity>` 委托给 `single()`；`TryFrom<AbilityTargetData>` 委托给 `acquired()`。因此接收
`impl Into<AbilityActivationTargets>` 的激活 API 仍可直接传入 Entity，而手工 Target Data 必须
先完成可失败转换。

正常 acquisition 已保证非空，因此 Targeting continuation 可以安全转换为抓取目标；游戏层手工
组装数据时必须处理 `acquired()` 的错误。该类型由 `gameplay_targeting` 门面、GAS 聚合门面、
crate root 和精简 prelude 显式重导出。`AbilityActivationTargetsError` 由领域门面、GAS 聚合门面
和 crate root 公开，但作为低频错误类型不进入 prelude。

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

候选 Query 要求实体有 `LogicalPosition`。Sphere、Cone 和 Explicit selection 还要求
`Targetable`；`SelectSelf` 只要求来源有 `LogicalPosition`。`FilterTags` 配置非空时，没有
`GameplayTagContainer` 的候选不会通过；空 requirements 对有无容器都通过。

## 逻辑位置与可选表现

`gameplay_position` 领域的 `LogicalPosition(pub Vec3)` 保存权威世界坐标。移动、索敌范围、
距离排序和 Hit 位置只读取这一组件；索敌不读取 `Transform` / `GlobalTransform`，也不会在
缺少逻辑位置时回退到表现坐标。`TargetingInput::origin` 仍由调用方显式传入，施法者原点应从
逻辑位置取得，避免主动把插值后的表现位置传回 Gameplay。

`LogicalPosition` 不 Required `Transform`，也不依赖 TransformPlugin。服务器或无表现对象
只需显式组合逻辑位置和 `Targetable`；完整 GAS Actor 另外组合 `GameplayAbilitySystemBundle`。
逻辑位置始终是世界坐标，不继承 `ChildOf` 父实体的变换，移动父节点不会隐式改变子节点的逻辑位置。

需要显示的实体可增加 `PositionPresentation`，它 Required `Transform`，在每帧 PostUpdate
只更新 `Transform.translation`，保留 rotation 和 scale。默认和 `interpolated()` 使用插值；
`direct()` 直接采用当前逻辑位置。`is_interpolation_enabled()` 查询模式；
`set_interpolation_enabled(bool)` 在模式变化时清除历史，下一次表现使用当前位置。

```rust
use bevy::prelude::*;
use bevy_gas::prelude::*;
use bevy_gas::gas::gameplay_targeting::Targetable;

// A simulation-only target needs no transform components.
commands.spawn((LogicalPosition(Vec3::new(5.0, 0.0, 0.0)), Targetable));

// A visual root interpolates its translation from authoritative logical positions.
commands.spawn((
    LogicalPosition(Vec3::new(5.0, 0.0, 0.0)),
    Targetable,
    PositionPresentation::interpolated(),
));
```

`LogicalPosition`、`PositionPresentation` 和 `PositionPresentationPlugin` 可从 prelude 导入。
表现系统由独立 `PositionPresentationPlugin` 安装；默认和配置 GAS 插件组已包含它。
单独安装 `GameplayAbilitySystemRuntimePlugin` 的应用需另外安装表现插件。服务器或自有表现系统
可对完整组调用 `.build().disable::<PositionPresentationPlugin>()`，接入示例见
[02 — 逻辑位置与表现调度](./02-plugins-and-lifecycle.md#逻辑位置与表现调度)。
表现根必须没有 `ChildOf`；Mesh、Sprite 等可以作为其视觉子节点，
由 Bevy 官方变换传播系统处理。表现应用需安装 `DefaultPlugins` 或 `TransformPlugin`，以便
更新 `GlobalTransform`；纯逻辑应用无此要求。当前不支持把逻辑世界坐标直接写入有父级实体的
局部 `Transform`，也没有内建独立逻辑实体到另一个表现实体的映射。

### 插值时序与传送

表现插件在每次 `FixedFirst` 运行 `capture_logical_position_history_system`，记录本
fixed tick 开始时的位置；Gameplay 移动在 `FixedUpdate` 的 `Movement` 阶段修改逻辑位置。每帧 `PostUpdate`
运行 `sync_position_presentation_system`，并排在 `TransformSystems::Propagate` 之前：

```text
FixedFirst：保存 previous
→ FixedUpdate：Movement → AbilityTasks → RequestProducers → 动态树同步 → 索敌与 Gameplay
→ PostUpdate：lerp(previous, current, Time<Fixed>.overstep_fraction())
→ Bevy Transform 传播：更新表现根与视觉子节点
```

常规插值的表现比最新模拟状态延迟一个 fixed tick，这是在两个已知状态间平滑过渡的预期行为。
一帧零个 fixed tick 时保留历史，只推进插值比例；一帧多个 tick 时使用最后相邻的两个逻辑状态。
首次生成或没有历史时直接同步当前逻辑位置；没有 `Time<Fixed>` Resource 时也直接同步。

传送时同时修改逻辑位置并清除表现历史，下一次表现同步直接落在新位置，避免穿过中间路径：

```rust
position.0 = destination;
presentation.reset();
```

`position` 与 `presentation` 分别是目标的可变 `LogicalPosition` 和 `PositionPresentation`。
若希望传送影响当前 tick 的索敌，应在 `Movement` 阶段完成逻辑位置修改，再由后续请求生产
系统采样 origin；不要在空间同步与范围抓取之间改位置。

常规连续移动通过 `Mut<LogicalPosition>` 修改 `.0`。表现插件注册的私有 Discard Observer
在移除或 `insert` 替换 `LogicalPosition` 时自动清除表现历史，组件替换视为不连续移动，
同一帧移除再添加也不会沿用旧位置插值。通过 Mut 修改坐标不会触发 Discard，因此这种方式的
传送仍需显式调用 `presentation.reset()`。

`PositionPresentation::last_error()` 返回最近历史采集或表现同步检测到的可选
`PositionPresentationError`，成功采集或同步后清除错误。`reset()` 只清历史，不修改位置、
Transform 或已有错误。
缺失逻辑位置、缺失 Transform、逻辑位置非有限或实体带有父级时，分别记录
`MissingLogicalPosition`、`MissingTransform`、`NonFiniteLogicalPosition`、`ParentedEntity`，
并清除历史、保持原 Transform。修复组件或坐标后可在后续同步恢复；表现错误不会修改权威逻辑位置。

读取本帧表现根 `Transform` 的跟随相机等系统应安排在 `PostUpdate`，显式
`.after(sync_position_presentation_system).before(TransformSystems::Propagate)`。
若读取的是传播后的子节点 `GlobalTransform`，则需排在传播之后，不能把尚未传播的世界变换
当作本帧的表现结果。

## 动态 AABB 树

Sphere/Cone 的候选来源统一为 `TargetingSpatialIndex` 动态 AABB 树，不保留全量扫描后端或
扫描回退。每次范围查询先用 `origin ± radius` 的 AABB 粗筛，再从 ECS 读取候选的当前位置，
完成原有球形距离、锥形角度和后续操作。Self/Explicit 已知实体，继续直接通过 Query 查询。

索引只保存 Entity 与派生空间数据；标签、属性和 Gameplay 状态仍由 ECS Component 持有。
目标的真实形状仍是 `LogicalPosition.0` 表示的位置点，Fat AABB 只是适当扩大的
树代理，当前每轴外扩 0.5 世界单位，不是单位碰撞体，也不扩大技能命中范围。目标在代理盒内移动时无需重新插入叶子，
超出后才更新树结构；精筛始终读取当前 ECS 位置。

`sync_targeting_spatial_index_system` 首次建立索引，随后通过组件变化检测增量同步
`Targetable` / `LogicalPosition`。Runtime Plugin 还注册两种组件的 Discard Observer，在组件
移除、替换或实体销毁时清理旧代理，不依赖 RemovedComponents 消息跨 fixed tick 保留。
同步系统也处理可读到的 RemovedComponents，支持手动调度；常规接入应安装 Runtime Plugin
获得完整生命周期处理。无目标请求时仍运行同步。非有限位置不进入树，恢复为有限位置后会重新
加入。树遍历顺序不参与 Gameplay 语义，候选仍先按 Entity bits 排序，再执行管线。

首次同步会遍历所有合格实体建立索引；后续同步只修改变化实体的树代理，但 ECS 的 Changed
过滤仍需检查组件 change tick。消除的是每次范围请求的全场景扫描，不是索引维护的全部遍历。

`TargetingSpatialIndex::len()` / `is_empty()` 反映当前代理数量；`is_ready()` 只表示首次同步已
完成，不验证游戏层移动与索敌的调度顺序，也不保证同步后没有发生空间修改。

## 同步抓取 API

```rust
pub fn acquire_targets(
    source: Entity,
    input: TargetingInput,
    definition: &TargetingDefinition,
    params: &TargetingSystemParams,
) -> Result<AbilityTargetData, TargetingError>;
```

`TargetingInput::new(origin, direction)` 捕获请求提交时的空间输入，
`with_explicit_target(entity)` 附加显式目标。Origin 总会检查有限值；Direction 仅在 Cone selection
需要时检查有限且平方长度大于 `f32::EPSILON`。缺失实体/Component、无效输入和最终空集合都会返回具体
`TargetingError`，不会 panic。

`TargetingSystemParams` 同时读取 `TargetingSpatialIndex` 和候选 ECS Query；同步调用与队列
处理使用同一入口。调用方必须先执行空间同步，再抓取，且两者之间不再修改目标空间状态。
未完成首次同步时，Sphere/Cone 返回 `TargetingError::SpatialIndexNotReady`，不会扫描 ECS
作为补偿。Self/Explicit 不依赖索引就绪状态，但位置非有限时返回 `InvalidTargetPosition`。

这是一次公共参数迁移：原先注入 `TargetingCandidateQuery` 的系统改为注入
`TargetingSystemParams`，并将 `&params` 传给 `acquire_targets()`。低层候选 Query 别名仍可用，
但不再单独构成同步抓取的访问参数。

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
continuation 刻意只保存 handle 与 Context：请求创建时尚未完成 target acquisition，因此不能
提前构造要求 targets 已存在的 `AbilityActivationData`。

processor 完整 drain 当前 Targeting FIFO。每个请求严格执行：

1. 调用同步 `acquire_targets()`。
2. 成功且 continuation 为 `ActivateAbility` 时，通过 `AbilityActivationTargets::acquired()`
   把完整 Target Data 转换为唯一目标值，再把 Targeting Request 的 source、该 targets 与
   continuation Context 组装为 `AbilityActivationData`，据此写入 `GameplayExecutionQueue`。
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
Movement
    ↓
AbilityTasks
    ↓
RequestProducers
    ↓
TargetingSpatialSync
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

Runtime 调用 Targeting 领域内部的安装入口，统一初始化索引/FIFO、注册空间生命周期 Observer，
并在 `TargetingSpatialSync` 无条件运行同步系统。游戏层将本 tick 的 `LogicalPosition` 移动和
相关 deferred 命令放入 `Movement`，在 `RequestProducers` 中读取已移动的位置提交请求。
阶段顺序保证这些命令在后续任务、请求采样及空间同步中可见；同集合内的业务顺序仍需游戏显式约束。
索敌不再依赖 FixedUpdate 内的世界变换传播。表现变换在 PostUpdate 更新，不会改变空间索引或抓取结果。
自定义同步抓取系统应显式排在 `TargetingSpatialSync` 之后；需要进入本 tick 统一 Gameplay
FIFO 的结果还须排在 `GameplayResolve` 之前。

空间索引不是任意时刻实时更新的全局视图。同步后发生的移动、增删组件和实体生成，需等下一次
同步后参与范围抓取；不要在同步与范围抓取之间修改这些状态。标签和属性过滤仍读取抓取时的 ECS
状态，无需等待空间同步。

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

- startup `Instant` 中的 `ApplyGameplayEffectToTargets` 在激活请求内部按动作定义顺序、再按
  `AbilityActivationTargets::entities()` 顺序直接应用。
- 运行时任务的同名动作按同一个目标值的顺序向统一 Gameplay FIFO 追加独立请求；single
  产生一个请求，acquired 按 Target Data 顺序产生多个请求。
- `ApplyGameplayEffectToTarget` 使用同一目标值的 `get_primary_target()`，主目标没有第二个存储源。
- 空 Target Data 不能创建 acquired 激活目标，因此运行时不存在 Context 与 Request 各自携带
  不同目标的分裂状态。
- 每个目标独立进行要求、免疫、概率和堆叠检查；一个目标拒绝不会中断后续目标的处理。

## 确定性与当前边界

- Sphere/Cone selection 先按 `Entity::to_bits()` 建立基础顺序。
- 距离排序使用 `f64::total_cmp()`，相同距离用 Entity bits 打破平局。
- 请求捕获 Origin、Direction 和显式目标，不在消费时重新读取输入设备。
- 非有限目标位置不进入范围候选；Self/Explicit 对无效位置返回具体错误。
- ECS 坐标与请求仍使用 `f32`；边界与距离计算使用 `f64` 中间值，避免有限 `f32` 坐标运算
  溢出。这不构成跨平台位级锁步保证。
- 当前没有物理射线/形状投射、遮挡检查、单位体积相交、纯世界位置目标、玩家确认任务或网络预测/复验。

动态树减少空间范围外的候选访问，但维护索引本身也有成本。性能评估应统计索引同步、范围查询、
精筛和排序的总成本，并改变实体数、移动比例、请求数与覆盖范围；不能仅凭使用树就承诺加速。

## 测试导航

| 测试文件 | 覆盖范围 |
| -------- | -------- |
| `tests/gas_test/gameplay_position_test.rs` | 独立表现插件、完整组启用/禁用、逻辑/表现分离、插值、传送与组件替换、非法状态及视觉子节点传播 |
| `tests/gas_test/gameplay_targeting_test.rs` | 定义与几何边界、逻辑位置索引生命周期/有限值、同 tick 同步、稳定排序、完整 drain 与多目标激活 |
| `tests/gas_test/runtime_paths_test.rs` | 默认 FixedUpdate 阶段与 Gameplay 同 tick 边界 |
| `tests/gas_test/queues_test.rs` | Task 派生 Gameplay 请求的 FIFO 行为 |

### 手动工作负载测量

以下 ignored 测试可显式运行；此命令使用默认 **debug 构建**：

```bash
cargo test --test gas_test measure_dynamic_aabb_targeting_workloads -- --ignored --nocapture
```

测试在规则网格上放置 64 / 4096 个目标，组合半径 12 / 512、移动比例 0% / 10% / 100%，
以及每 tick 1 / 32 次球形抓取并按距离排序。每组先预热一次，再取 4 tick 的平均耗时。
移动沿 Z 轴在 ±0.25 世界单位之间切换，留在当前 Fat AABB 内；因此这组测量主要覆盖
变化检测与代理盒内更新，不代表持续跨盒移动、频繁树重插入或生成销毁的成本。

输出单位为毫秒 / tick：`fixed_update_ms` 包括完整测试 App 的 FixedUpdate（含索引同步），
`query_sort_ms` 包括该 tick 的全部同步范围查询、精筛和排序，`total_ms` 为两者之和。
初始建树、目标位置写入以及请求队列/结果 Observer 派发不在这两段计时内。

这是动态树单实现的工作负载耗时，没有旧扫描实现作为对照，不能据此推导加速比。
少量 debug 采样也不等同于优化构建下的生产场景性能；比较时须固定机器、构建配置和场景，
另行覆盖实际游戏的移动模式与查询分布。

继续阅读：[07 — Gameplay 技能](./07-gameplay-abilities.md)、
[16 — Gameplay 执行模块](./16-gameplay-execution.md)。
