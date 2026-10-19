# 15 — Gameplay 目标结果契约

## 职责与源码布局

`bevy_gas::gas::gameplay_targeting` 只拥有目标结果数据和激活目标值。GAS 使用 Entity 与有序
目标数据完成技能和效果结算，不决定目标如何获取，不要求目标挂载位置或渲染组件，也不安装
空间索引、索敌队列或表现系统。

空间查询、锁定规则与攻击命中检测由游戏决定。配套模板保留原有动态 AABB 树、逻辑位置、
选择/过滤/排序和目标请求队列，详见
[模板 — 空间、索敌与 GAS 接入](../../bevy_gas_template/.docs/05-spatial-targeting.md)。

| 文件 | 职责 |
| ---- | ---- |
| `ability_target_data.rs` | 有序命中与目标数据 |
| `activation_targets.rs` | 单目标/抓取目标的统一激活值及非空约束 |
| `error.rs` | 空激活目标错误 |

完整源码树见 [17 — 当前源码树](./17-source-layout-and-maintenance.md#当前源码树)。

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

`AbilityTargetData::new()` 保留调用方给出的顺序，允许空数据，不执行空间查询、去重、排序、
实体有效性或有限坐标校验。调用方负责这些语义；空数据不能转换为 Acquired 激活目标。
`position` 与 `normal` 是游戏提供的结果字段，不证明已经完成碰撞检测，也不自动参与效果幅度计算。
位置可以是锁定目标的逻辑位置，也可以是游戏命中检测提供的接触点，调用方应定义一致的使用约定。

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

游戏层组装数据时必须处理 `acquired()` 的错误。该类型由 `gameplay_targeting` 门面、GAS 聚合门面、
crate root 和精简 prelude 显式重导出。`AbilityActivationTargetsError` 由领域门面、GAS 聚合门面
和 crate root 公开，但作为低频错误类型不进入 prelude。

## 游戏查询或命中检测的接入

游戏可以从 AI 选择、鼠标指定、范围查询或攻击命中检测获得目标，再创建相同的结果契约：

```rust
use bevy_gas::gas::gameplay_targeting::{
    AbilityActivationTargets, AbilityTargetData, AbilityTargetHit,
};

let data = AbilityTargetData::new(
    origin,
    vec![AbilityTargetHit::new(target, position, normal)],
);
let targets = AbilityActivationTargets::acquired(data)?;
```

这里的 `origin`、`target`、`position`、`normal` 来自游戏查询。将已验证的 `targets` 与 source、
`AbilityActivationContext` 组成 `AbilityActivationData`，通过 `GameplayExecutionQueue` 提交激活请求。
只需要一个已知实体时可直接使用 `AbilityActivationTargets::single(entity)`，无需构造空间数据。

对于已经启动的挥砍或弹体，接触目标时通常应创建效果应用请求，而非再次激活技能，以免重复
支付成本或触发冷却。当前效果请求不会自动读取 Hit 的接触位置、法线或部位；有此需求时由游戏
明确保存和传递上下文，不能把数据字段的存在等同于完整命中机制。

游戏负责受击体到 GAS Actor 的归属、多受击体去重、稳定结果排序、攻击窗口及重复命中规则。
GAS 负责现有技能校验和效果应用规则。`AbilityActivationTargets::acquired()` 只验证非空，
不会删除重复实体；重复项会按给定顺序参与多目标动作。

## 调度边界

GAS 保留 `AbilityTasks`、`RequestProducers`、`PreGameplayConvergence`、`GameplayResolve` 等
公开阶段，不再拥有 Movement、空间同步或 Targeting 阶段。
游戏将移动、查询和命中系统明确排序在需要的 GAS 阶段之间；希望当前 tick 结算的请求应在
`GameplayResolve` 前入队，若还修改外部标签，宜在 `PreGameplayConvergence` 前完成。
同一集合内的请求生产顺序仍需显式 `.chain()` / `.before()` / `.after()`。

模板的 `GameplaySchedulePlugin` 负责把游戏 `GameplaySet::Movement` 排在 EffectTicks 与 AbilityTasks
之间，把 `SpatialSync` / `Targeting` 排在 RequestProducers 与 PreGameplayConvergence 之间。
表现插值由模板的独立插件在 PostUpdate 完成，不反向改变权威查询位置。

## 多目标效果

- startup `Instant` 中的 `ApplyGameplayEffectToTargets` 在激活请求内部按动作定义顺序、再按
  `AbilityActivationTargets::entities()` 顺序直接应用。
- 运行时任务的同名动作按同一目标值的顺序向统一 Gameplay FIFO 追加独立请求。
- `ApplyGameplayEffectToTarget` 使用 `get_primary_target()`，主目标没有第二个存储源。
- 每个目标独立进行要求、免疫、概率和堆叠检查；一个目标拒绝不会中断后续目标。
- 目标数据是激活时捕获的结果；等待任务不会自动重新索敌，也不持续跟踪其空间变化。

## 从旧版库内索敌迁移

| 旧入口 | 当前所有者 |
| --- | --- |
| `AbilityTargetHit`、`AbilityTargetData`、`AbilityActivationTargets`、对应构造错误 | 保留在 GAS 原领域路径 |
| `LogicalPosition`、`PositionPresentation`、`PositionPresentationPlugin` | 模板 `gameplay::position` |
| `Targetable`、`TargetingDefinition`、`TargetingSystemParams`、索引和请求队列 | 模板 `gameplay::targeting` |
| `GameplayAbilitySystemSet::Movement` / `TargetingSpatialSync` / `Targeting` | 模板 `GameplaySet::Movement` / `SpatialSync` / `Targeting` |

调用方需调整已迁移 API 的导入，并显式安装模板 `TargetingPlugin`；需要表现时再安装
`PositionPresentationPlugin`。GAS 的默认和自定义成本插件组均只安装 GAS 基础设施。
本次迁移不增加第三方依赖，模板保留动态树单实现，既有点索敌语义不变，尚无实际体积碰撞检测。

## 测试导航

`tests/gas_test/gameplay_targeting_test.rs` 验证结果构造、空目标拒绝、主目标一致性和多目标
技能/任务结算。测试直接构造目标数据，不依赖任何模板空间实现或表现组件。
模板 `tests/gameplay_test/` 覆盖位置表现、动态树生命周期、索敌管线、队列和跨库调度。

继续阅读：[07 — Gameplay 技能](./07-gameplay-abilities.md)、
[16 — Gameplay 执行模块](./16-gameplay-execution.md)。
