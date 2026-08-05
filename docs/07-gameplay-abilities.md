# 07 — Gameplay 技能

## 概述

Gameplay 技能代表角色可执行的动作——法术、攻击、冲刺等。它们支持冷却、消耗、激活效果、启动任务和链式激活。

## 技能定义 (`GameplayAbility`)

```rust
pub struct GameplayAbility {
    ability_tags: AbilityTags,
    startup_tasks: Vec<AbilityTaskDef>,
    cooldown: Option<Arc<GameplayEffect>>,
    cost: Option<Arc<GameplayEffect>>,
    activation_effects: Vec<Arc<GameplayEffect>>,
    end_on_activation: bool,
    allow_multiple_instances: bool,
}
```

### `AbilityTags`

```rust
pub struct AbilityTags {
    ability_asset_tags: Vec<GameplayTag>,        // 此技能的身份标签
    cancel_abilities_with_tags: Vec<GameplayTag>, // 取消带有这些标签的其他技能
    block_abilities_with_tags: Vec<GameplayTag>,  // 阻止带有这些标签的其他技能
    activation_required_tags: Vec<GameplayTag>,   // 有来源标签容器时必须全部拥有
    activation_blocked_tags: Vec<GameplayTag>,    // 有来源标签容器时必须全部不拥有
}
```

### `GameplayAbilitySpec`

已授予技能的运行时实例：

```rust
pub struct GameplayAbilitySpec {
    handle: AbilitySpecHandle,
    ability: Arc<GameplayAbility>,
    level: u32,
    input_id: Option<u16>,     // 可选的输入绑定
    input_pressed: bool,       // 绑定输入当前是否处于按下状态
    active_count: u32,         // 当前活跃实例数
}
```

当前 `AbilitySystemComponent` 不会自动附带 `GameplayTagContainer`。来源缺少标签容器时，
required、blocked 和 cooldown granted-tag 检查会被跳过；任何使用这些能力的实体都应显式
附加 `GameplayTagContainer`。

`input_pressed` 默认为 `false`。输入处理系统应在绑定输入按下、松开时分别通过
`set_input_pressed(true)` 和 `set_input_pressed(false)` 更新它；可使用
`is_input_pressed()` 查询当前状态。该字段只记录瞬时输入状态，不负责自动激活技能。

### `AbilitySpecHandle`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbilitySpecHandle(u32);
```

## 激活流程

### `try_activate_ability_by_handle()`

独立的同步技能激活调用路径：

```rust
pub fn try_activate_ability_by_handle(
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
    activation_context: AbilityActivationContext,
    params: &mut AbilitySystemParams,
) -> Result<(), AbilityActivationError>;
```

该函数会先收敛 Active Effect 的 Tag 条件，并在返回前完整消费本次激活产生的 startup
Instant 后续请求。它使用独立的局部 batch，不会插队消费全局 `GameplayExecutionQueue`
中已经存在的请求。因此运行时生产系统应使用 `GameplayExecutionQueue::push_activation()`；
不要在同一逻辑阶段混用全局排队请求和该同步入口。同步入口适合初始化、测试或调用者明确
需要一个独立即时结算边界的场景。这里的同步不表示 `Commands` 已 flush，也不承诺失败时
回滚此前全部 Gameplay 副作用。

**激活序列：**

```
1. 检查活跃计数（多实例控制）
2. passes_ability_activation_requirements()
   ├── 阻止标签检查（来源 ASC 的 blocked_ability_tags）
   ├── 激活阻止标签检查（来源标签）
   ├── 激活要求标签检查（来源标签）
   └── 冷却标签检查（来源是否有冷却标签？）
3. prepare_ability_commit_plans()
   ├── 准备消耗计划
   └── 准备冷却计划
4. 取消匹配 cancel_abilities_with_tags 的活跃技能
5. start_ability()
   ├── 在来源 ASC 上设置 block_abilities_with_tags
   ├── 递增 active_count
   └── 生成 ActiveGameplayAbility 实体
6. execute_ability_commit_plans()
   ├── 依次执行消耗与冷却效果
   └── 失败时回滚已启动技能的 bookkeeping；不提供全部 Gameplay mutation 回滚
7. 应用 activation_effects（尽力而为）
8. start_startup_ability_tasks()
   ├── Instant：立即派发到当前 Gameplay batch
   └── WaitTicks：生成运行时任务实体
9. 若 end_on_activation → 将状态设为 Ending
```

### `AbilityActivationError`

```rust
pub enum AbilityActivationError {
    InvalidChain(AbilityChainError),
    MissingAbilitySystemComponent { source: Entity },
    AbilityNotFound { source: Entity, handle: AbilitySpecHandle },
    MultipleInstancesNotAllowed { source: Entity, handle: AbilitySpecHandle },
    ActivationRequirementsNotMet { source: Entity, handle: AbilitySpecHandle },
    CommitPreparationFailed { source: Entity, handle: AbilitySpecHandle, error: AbilityCommitError },
    StartFailed { source: Entity, handle: AbilitySpecHandle, error: GameplayTagError },
    CancellationFailed { source: Entity, handle: AbilitySpecHandle, error: GameplayTagError },
    CommitExecutionFailed { source: Entity, handle: AbilitySpecHandle, error: AbilityCommitError },
}
```

`AbilityCommitError` 进一步区分 Cost 配置、Cost/Cooldown 准备、支付能力以及执行错误。
`AbilityActivationError::is_rejection()` 用于区分正常的激活拒绝与结构性错误；队列在边界
分别使用 debug 和 error 级别记录。验证或 Commit 准备阶段失败发生在取消旧技能之前；一旦
进入取消阶段，后续 start 或 Commit 执行失败不会恢复已经取消的旧技能。

### `AbilityActivationContext`

```rust
pub struct AbilityActivationContext {
    chain: Option<AbilityChainContext>,
    instigator: Entity,
    causer: Option<Entity>,
    source_snapshot: Option<AttributeSetSnapshot>,
    target_data: Option<AbilityTargetData>,
    reason: AbilityActivationReason,
}

pub enum AbilityActivationReason {
    Direct,
    Input { input_id: u16 },
    Chained { parent_ability: ActiveAbilityHandle },
    TaskEvent { event_id: UniqueName },
    GameplayEffect,
}
```

当前公共构造路径会生成 `Direct`，链式 API 会生成 `Chained`；`Input`、`TaskEvent` 和
`GameplayEffect` 目前是预留原因，尚无公共 context 构造器或 setter。

`instigator` 默认等于技能来源实体，可通过 `with_instigator()` 指定实际发起者。
`causer` 表示直接造成技能行为的可选物理实体。两者都会沿链式技能激活继承，并传播到
技能产生的 `EffectPayload`；消耗、冷却、来源属性和来源标签仍始终从技能的 `source`
读取。

`target_data` 保存目标抓取模块返回的完整有序目标集合。旧 `target: Entity` 继续表示首要
目标；存在 Target Data 时，激活效果会逐个应用到其中的实体。详见
[15 — Gameplay 目标抓取](./15-gameplay-targeting.md)。

`TargetingContinuation::ActivateAbility` 会保证旧 `target` 等于 `primary_entity()`；直接调用
`GameplayExecutionQueue::push_activation()` 时不会验证这一点，调用方应自行保持两者一致。

### `AbilityChainContext`

追踪链式技能激活，防止无限循环：

```rust
pub struct AbilityChainContext {
    chain_id: u64,
    depth: u8,                              // 最大：ABILITY_CHAIN_MAX_DEPTH (8)
    visited: Vec<AbilitySpecHandle>,        // 循环检测
}

impl AbilityChainContext {
    pub fn root(handle: AbilitySpecHandle, chain_id: u64) -> Self;
    pub fn next(&self, handle: AbilitySpecHandle) -> Result<Self, AbilityChainError>;
}
```

## 活跃技能生命周期

### `ActiveGameplayAbility`

技能激活时生成的 `Component`：

```rust
#[derive(Component, Clone)]
pub struct ActiveGameplayAbility {
    source: Entity,
    spec_handle: AbilitySpecHandle,
    target: Entity,
    status: AbilityActivationStatus,
    activation_context: AbilityActivationContext,
}
```

### `AbilityActivationStatus`

```
Active ──► Ending ──► (销毁)
   │
   └──► Cancelled ──► (销毁)
```

| 状态         | 说明                         |
| ------------ | ---------------------------- |
| `Active`     | 运行中；任务正在 tick        |
| `Ending`     | 正常关闭；任务停止，清理开始 |
| `Cancelled`  | 强制关闭；任务停止，清理开始 |

### 生命周期方法

| 函数 | 说明 |
| ---- | ---- |
| `end_ability(source, active_handle, params) -> bool` | 将匹配来源的实例设为 `Ending` |
| `cancel_ability(source, active_handle, params) -> bool` | 将匹配来源的实例设为 `Cancelled` |
| `can_activate_ability(source, target, &ability, level, params) -> bool` | 标签与数值费用快速预检 |
| `commit_ability(source, &ability, level, params) -> Result` | 独立应用消耗 + 冷却，不启动技能实例 |

`can_activate_ability()` 不检查授予句柄、活跃实例数、技能链、取消或 startup 预检，也不保证
后续 `try_activate_ability_by_handle()` 一定成功。它用传入 `target` 构造计算型费用的预检上下文，
而实际 commit 将费用应用到 `source`；依赖目标的费用计算应避免把它当作最终授权结果。

`cleanup_finished_abilities_system` 销毁处于 `Ending` 或 `Cancelled` 状态的技能，递减活跃计数，移除阻止标签。

## 统一 Gameplay 执行队列

```rust
#[derive(Resource)]
pub struct GameplayExecutionQueue { /* ... */ }
```

技能激活与效果应用共享 `GameplayExecutionQueue`。加入激活请求：

```rust
execution_queue.push_activation(source, target, handle, context);
```

`GameplayResolve` 在当前 `FixedUpdate` 中按跨类型 FIFO 处理完整 drain，不会根据请求数量
把技能隐式推迟到后续 tick。一个激活请求产生的 startup Instant 后续请求会追加到同一
FIFO，并在同一次 drain 内处理。

链式激活（带循环/深度保护）：

```rust
execution_queue.push_chained_activation(
    source, target, handle, parent_ability, &parent_context,
)?;
```

## 技能任务

详见 [08 — 技能任务](./08-ability-tasks.md)。
