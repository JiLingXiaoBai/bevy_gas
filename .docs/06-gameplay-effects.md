# 06 — Gameplay 效果

## 职责

Gameplay Effects 定义并执行 Buff、Debuff、即时数值变化、周期效果、标签授予、免疫、移除条件
和堆叠。定义通过 `Arc<GameplayEffect>` 共享；有限和无限效果存放在目标实体的
`ActiveGameplayEffects` 中。

Effects 的修改入口使用 `EffectSystemParams`，不要求调用者提供 ASC 或 Active Ability 查询。
只读预检使用 `EffectReadOnlyParams`，只借用 Tag/Attribute registry 和三类只读 Component 查询，
不获取随机源、执行队列或组件写访问。能力侧通过 `effects` 字段组合对应参数。

## 源码布局

```text
src/gas/
├── gameplay_effects.rs
└── gameplay_effects/
    ├── gameplay_effect.rs
    ├── gameplay_effect/
    │   ├── context.rs          # EffectPayload and EffectContext
    │   ├── definition.rs       # GameplayEffect
    │   ├── timing.rs           # Duration and period definitions
    │   ├── effect_tags.rs      # Tags, requirements, and immunity
    │   └── stacking.rs         # Stacking policies
    ├── gameplay_effect_spec.rs # Evaluated modifiers/timing with definition reference
    ├── effect_system_params.rs # Effect-only ECS access
    ├── active_gameplay_effect.rs
    └── active_gameplay_effect/
        ├── planning.rs         # Validation, plan, and public errors
        ├── application.rs      # Synchronous entry and stack lookup
        ├── execution.rs        # Plan execution and rollback
        ├── modifiers.rs        # Shared instant/duration modifier mutations
        ├── lifecycle.rs        # Shared contributions and component lifecycle hooks
        ├── diagnostics.rs      # Optional convergence measurements
        ├── state.rs            # Target-owned active storage
        ├── requirements.rs     # Ongoing/removal fixed point
        ├── removal.rs          # Cleanup and public removal/query API
        └── ticking.rs          # Duration and period systems
```

排队请求类型位于 `src/gas/gameplay_execution/request.rs`，不是 Effects 目录的一部分。

`active_gameplay_effect/modifiers.rs` 是私有实现模块，集中即时修改、持续修饰器安装，以及层数
变化时的移除/重建操作。叠层与到期减层共用刷新函数，条件恢复也使用同一持续修饰器安装逻辑；
`lifecycle.rs` 进一步集中 retained modifier 与 granted tag 的安装、撤销和失败清理；新建、抑制、
恢复、到期和手动移除共用这些操作。周期 pulse 仍属于执行行为，不参与 retained contribution 的
安装。安装会先验证完整属性集合，再授予标签；失败不会留下部分持有效果。

## 公共 API

### 定义、时间与标签

```rust
impl GameplayEffect {
    pub fn new(
        modifiers: Vec<Modifier>,
        duration: EffectDurationTicks,
        period: Option<EffectPeriodTicks>,
        probability_to_apply: f32,
        stacking_policy: StackingPolicy,
        tags: EffectTags,
    ) -> Self;
    pub fn make_spec(self: &Arc<Self>, context: &EffectContext) -> GameplayEffectSpec;
    pub fn get_tags(&self) -> &EffectTags;
    pub fn get_stacking_policy(&self) -> StackingPolicy;
    pub fn has_only_add_modifiers(&self) -> bool;
    pub fn get_probability_to_apply(&self) -> f32;
}

pub enum EffectDurationTicks {
    Instant,
    DurationTicks(ModifierMagnitude),
    Infinite,
}

impl EffectPeriodTicks {
    pub fn new(period: ModifierMagnitude, execute_on_applied: bool) -> Self;
}
```

`EffectTags::new()` 只接收效果资产标签与授予标签；应用、持续、移除条件以及免疫和清理
规则都通过链式 builder 显式配置：

```rust
impl EffectTags {
    pub fn new(
        asset_tags: Vec<GameplayTag>,
        granted_tags: Vec<GameplayTag>,
    ) -> Self;

    pub fn with_application_requirements(
        self,
        source: TagRequirements,
        target: TagRequirements,
    ) -> Self;

    pub fn with_ongoing_requirements(
        self,
        source: TagRequirements,
        target: TagRequirements,
    ) -> Self;

    pub fn with_removal_requirements(
        self,
        source: TagRequirements,
        target: TagRequirements,
    ) -> Self;

    pub fn with_granted_application_immunity(
        self,
        immunity_queries: Vec<GameplayEffectImmunityQuery>,
    ) -> Self;

    pub fn with_remove_effects_with_tags(
        self,
        effect_tags: Vec<GameplayTag>,
    ) -> Self;
}
```

`new()` 会把所有可选规则初始化为空。三组 source/target requirement 在内部通过私有
`SourceTargetTagRequirements` 成对保存，避免定义继续扩展成大量并行字段。所有配置都通过
`get_*` 方法读取；`get_required_tags()` 与 `get_blocked_tags()` 是目标应用条件的兼容快捷
getter。

```rust
impl GameplayEffectImmunityQuery {
    pub fn new(source: TagRequirements, effect: TagRequirements) -> Self;
    pub fn matches(
        &self,
        source_tags: Option<&GameplayTagContainer>,
        effect_asset_tags: &[GameplayTag],
        tag_manager: &Res<GameplayTagManager>,
    ) -> Result<bool, GameplayTagError>;
    pub fn matches_tag_bits(
        &self,
        source_tags: Option<&GameplayTagContainer>,
        effect_asset_bits: Option<&GameplayTagBits>,
    ) -> bool;
}
```

### 堆叠策略

```rust
let stacking_policy = StackingPolicy::new(
    stacking_type,
    stack_limit,
    magnitude_policy,
    duration_policy,
    period_policy,
    overflow_policy,
    expiration_policy,
);
```

| 类型 | 选项 |
| --- | --- |
| `StackingType` | `None`, `AggregateBySource`, `AggregateByTarget` |
| `StackMagnitudePolicy` | `None`, `Linear` |
| `StackDurationPolicy` | `KeepExisting`, `RefreshOnSuccessfulStack` |
| `StackPeriodPolicy` | `KeepCurrentTick`, `ResetOnSuccessfulStack` |
| `StackOverflowPolicy` | `RejectApplication`, `RefreshDuration` |
| `StackExpirationPolicy` | `RemoveAllStacks`, `RemoveSingleStack` |

`non_stacking()` 使用全部“保持/不缩放”策略；`linear_refreshing(type, limit)` 使用线性幅度、
成功堆叠时刷新 duration、重置 period、溢出拒绝、到期移除全部层数。

`StackingPolicy` 只由不可变的 `GameplayEffect` 定义持有。`GameplayEffectSpec` 保留定义
`Arc` 并捕获 modifier、duration 与 period 的求值结果，不再复制堆叠策略；Spec 的
`get_stacking_policy()` 直接委托定义。

### Payload 与计算上下文

```rust
impl EffectPayload {
    pub fn new(source: Entity, causer: Option<Entity>, level: u32) -> Self;
    pub fn with_instigator(self, instigator: Entity) -> Self;
    pub fn with_source_snapshot(self, snapshot: AttributeSetSnapshot) -> Self;
    pub fn get_source(&self) -> Entity;
    pub fn get_instigator(&self) -> Entity;
    pub fn get_causer(&self) -> Option<Entity>;
    pub fn get_level(&self) -> u32;
    pub fn get_source_snapshot(&self) -> Option<&AttributeSetSnapshot>;
}
```

`new()` 默认令 `instigator == source`。`EffectContext` 实现
`ModifierEvaluationContext`；自定义 calculator 应依赖该 trait，而不是读取 Context 的 Query
字段。

### 应用、计划与移除

```rust
pub fn apply_gameplay_effect(
    target: Entity,
    effect: &Arc<GameplayEffect>,
    params: &mut EffectSystemParams,
    payload: &EffectPayload,
) -> Result<(), GameplayEffectApplicationError>;

pub fn prepare_gameplay_effect(
    target: Entity,
    effect: &Arc<GameplayEffect>,
    params: &mut EffectSystemParams,
    payload: &EffectPayload,
) -> Result<GameplayEffectApplicationPlan, GameplayEffectApplicationError>;

pub fn execute_gameplay_effect_plan(
    plan: GameplayEffectApplicationPlan,
    params: &mut EffectSystemParams,
) -> Result<(), GameplayEffectApplicationError>;

pub fn remove_active_effect(
    handle: ActiveEffectHandle,
    params: &mut EffectSystemParams,
) -> Result<bool, GameplayEffectApplicationError>;

pub fn remove_active_effects_with_tags(
    target: Entity,
    tags: &[GameplayTag],
    params: &mut EffectSystemParams,
) -> Result<usize, GameplayEffectApplicationError>;
```

`GameplayEffectApplicationPlan` 是不透明、应立即执行的计划。它只公开
`get_modifier_specs()` 和 `is_instant()`；内部 application kind 不是公共 API。

### 活跃效果只读访问

`ActiveGameplayEffects` 公开 `len()`、`is_empty()`、`get(handle)` 和
`handles(target)`。`ActiveGameplayEffect` 公开 spec、source、target、stack count、inhibited、
duration 和 period getter。单条效果通过 Effects API 移除。整体容器通过 ECS `remove` 或
`insert` 替换时，`on_discard` hook 在旧组件仍存在时同步撤销修饰器和标签，之后才安装替代组件。
已抑制效果不会重复扣除标签引用。`despawn` 同样允许，已不存在的组件会安全跳过。

每次 ECS 安装容器都会获得新的 `storage_id`，移动取出的容器不会迁移活跃效果。
`ActiveEffectHandle::new(target, storage_id, slot, generation)` 及对应 getter 暴露句柄信息。
替换前后的相同 slot/generation 具有不同 storage_id，旧句柄不会指向新效果。
效果转换出的 ModifierSourceId 属于运行时域；调用方通过 ModifierSourceId::new 创建的来源
位于独立域，数值字段相同也不会在移除或替换效果时被误删。
整体替换 Tags 与 ActiveEffects 时，清理只影响旧标签值，不会扣除新标签组件自己的引用。正常代码应保存 API 返回或遍历得到的 handle，而不是猜测槽位。

应先安装 GAS runtime，再创建效果容器；无法分配身份的容器返回
`UninitializedActiveEffectStorage`。禁止通过可变 Query 直接赋值、`mem::replace` 或 `mem::take`
替换整个容器，这些普通 Rust 操作不会触发 ECS hooks。

### 错误

```rust
pub enum GameplayEffectApplicationError {
    InvalidProbability { probability: f32 },
    ProbabilityRejected,
    ApplicationRequirementsNotMet,
    BlockedByImmunity,
    InvalidDuration,
    MissingActiveGameplayEffects { target: Entity },
    UninitializedActiveEffectStorage { target: Entity },
    ActiveEffectCapacityExceeded { target: Entity },
    MissingAttributeSet { target: Entity },
    MissingAttribute { target: Entity, id: AttributeId },
    MissingTagContainer { target: Entity },
    StackOverflowRejected,
    GameplayTag(GameplayTagError),
    AttributeId(AttributeIdError),
}
```

`is_rejection()` 仅对概率拒绝、应用条件失败、免疫阻止和堆叠溢出拒绝返回 `true`。其余错误
表示配置或 ECS 状态问题。

## 关键语义

### 时间换算与执行模式

`ModifierMagnitude` 转 tick 时使用以下规则：正的非整数向上取整；大值饱和到 `u32::MAX`；
非有限值和小于等于 0 的值变为 0。有限 duration 为 0 会返回 `InvalidDuration`。

| 定义 | 运行时行为 |
| --- | --- |
| `Instant` | 不创建 Active Effect；modifier 直接修改 base；period 被忽略 |
| Duration/Infinite + `period: None` | modifier 作为持续聚合值 |
| Duration/Infinite + period 解析为 0 | 与无 period 一样作为持续聚合值；`execute_on_applied` 无效 |
| Duration/Infinite + 正 period | 不保留 duration modifier；到点时以 instant 方式修改 base |

新建正周期效果且 `execute_on_applied == true` 时，会在创建 tick 立即 pulse 一次。该 pulse 发生
在首次 ongoing Requirement 收敛之前。

### 目标组件要求

| 配置 | 必需组件 |
| --- | --- |
| 非 Instant | `ActiveGameplayEffects` |
| 至少一个 modifier | `AttributeSet` 且所有目标属性已初始化 |
| 非空 `granted_tags` | `GameplayTagContainer` |

Instant 效果也会验证 `granted_tags`，但不会持久授予它们；因此 Instant 的 granted tags 应为空。
完整 GAS Actor 推荐使用 `GameplayAbilitySystemBundle`，纯即时属性目标可只组合所需组件。

### Prepare 与 Execute

Prepare 的顺序是：

1. 验证概率并掷骰；
2. 检查 source/target application requirements；
3. 检查目标上未抑制 Active Effect 授予的 immunity；
4. 通过 `EffectContext` 求值 spec；
5. 验证 duration、目标组件、属性初始化和 granted tag ID；
6. 收集 `remove_effects_with_tags` 匹配项；
7. 在未计划移除的效果中查找堆叠目标并决定层数。

Execute 会重新验证结构性 ECS 状态和待清理数据，然后先移除计划中的旧效果，再执行 instant、
stack 或 create。它不会重新掷骰，也不会重新检查 application requirement、immunity 或堆叠
决策，并且不提供数据库式回滚。Plan 不应跨帧保存。

`apply_gameplay_effect()` 在调用前后执行 Requirement 收敛；运行时生产系统若需要与技能激活
共享严格 FIFO，应使用 `GameplayExecutionQueue::push_application()`。

### 标签、免疫与移除

- `asset_tags` 标识效果定义；`granted_tags` 只在未抑制 Active Effect 存活期间存在。
- application requirements 在应用前检查一次。
- ongoing requirements 不满足时移除 duration modifier、granted tags 和 immunity 参与资格；
  条件恢复后重新添加。
- 非空 removal requirements 通过时永久移除效果；空 removal requirement 表示未配置。
- `remove_effects_with_tags`、`has_active_effect_with_tags()` 和批量移除都匹配 asset tags。

Asset tag 匹配会同时展开效果标签和查询标签的祖先，再检查任意交集。这意味着共享祖先的
sibling 标签也可能匹配；需要精确类别时，不要让过宽的共同父标签进入移除查询。

Requirement 以 target entity、slot、generation 的稳定顺序迭代到固定点。检测到非收敛循环
时，参与循环的效果会 fail-closed 移除。

### 堆叠

只有共享同一个 `Arc<GameplayEffect>` 实例（`Arc::ptr_eq`）的 spec 才能堆叠。按 source 聚合
还要求 source 相同；按 target 聚合只要求位于同一目标容器。`stack_limit == 0` 表示无限制。

`StackMagnitudePolicy::Linear` 将每个 spec 的原始 value 乘层数。已存在效果保留首次捕获的
spec；后续应用更新层数和策略允许的计时器，不会用新 payload 重新捕获其 modifier 幅度。

`RefreshDuration` 溢出策略保持原层数并继续成功堆叠路径；只有搭配
`RefreshOnSuccessfulStack` 才实际刷新 duration。`RemoveSingleStack` 到期时减一层并把有限
duration 重置为定义值。

### FixedUpdate 顺序

完整插件中的 EffectTicks 顺序为：

```text
duration tick
→ requirement convergence
→ period tick
→ requirement convergence
```

所以到期清理及其标签变化先收敛，周期效果不会在同一 tick 多 pulse 一次。抑制不暂停有限
duration，但会暂停 period current tick。

之后流水线依次运行 AbilityTasks、RequestProducers、Targeting、PreGameplayConvergence、
GameplayResolve、UpdateEffectTagRequirements、Cleanup 和 RecalculateAttributes。

## 示例

推荐的运行时生产方式：

```rust
fn queue_poison(
    mut queue: ResMut<GameplayExecutionQueue>,
    request: Res<PoisonRequest>,
) {
    let payload = EffectPayload::new(request.source, request.causer, request.level);
    if let Err(error) = queue.push_application(request.target, request.effect.clone(), payload) {
        error!("failed to enqueue poison effect: {error}");
    }
}

app.add_systems(
    FixedUpdate,
    queue_poison.in_set(GameplayAbilitySystemSet::RequestProducers),
);
```

队列会在同一 `GameplayResolve` drain 中消费期间追加的派生请求，并保持 Ability/Effect 的跨类型
FIFO。入队成功返回 `GameplayExecutionRequestId`；Resolver 按消费顺序发送
`GameplayExecutionResult` Message，区分 `Succeeded`、`Rejected` 和 `Failed`，失败项保留具体
`GameplayEffectApplicationError`。可在 `GameplayResolve` 之后按 request_id 关联结果。
需要独立同步结果时使用 `apply_gameplay_effect()`。

## Requirement 收敛测量

`EffectRequirementDiagnostics` 由 Runtime Plugin 注册，默认关闭。使用 `set_enabled(true)`
开启后，`metrics()` 返回累计收敛调用次数、决策轮次、签名/决策访问量、完整效果快照数量、
状态转换数量、循环数量及耗时；`reset()` 清空数据，不改变开关。关闭时不读取时钟。
计时仅供观测，不参与 gameplay 判断或确定性顺序。

显式运行代表性测量：

```bash
cargo test --test gas_test effects_test::requirements_test::measure_requirement_convergence -- --ignored --nocapture --test-threads=1
```

2026-09-15 在同一机器、debug/unoptimized 测试 profile 下，各场景进行 64 次无标签变化的收敛，
每个活跃效果携带 4 个 modifier。优化只把决策阶段的完整效果复制延后至确实发生
Remove/Inhibit/Uninhibit 时；全局双扫描、稳定排序、source 跨实体依赖和循环处理保持原算法。

| 活跃效果数 | 签名/决策访问量（各自） | 优化前快照 | 优化后快照 | 优化前耗时 | 优化后耗时 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 | 0 | 0 | 0 | 0.095 ms | 0.072 ms |
| 128 | 8192 | 8192 | 0 | 9.252 ms | 8.599 ms |
| 2048 | 131072 | 131072 | 0 | 162.781 ms | 149.183 ms |

上述时间是单次观测，不代表发布构建吞吐或稳定加速比例。确定的工作量收益是：没有状态转换时，
决策阶段不再复制 effect/spec 及其 modifier Vec；2048 效果场景减少 131072 次完整快照。
当前没有据此引入标签反向依赖索引；后续仍应根据实际项目的标签变化频率和收敛轮数决定是否增量化。

## 边界与注意事项

- 不要依赖 `GameplayEffect`、`EffectTags`、Plan 或 Active Effect 的私有字段布局。
- 整体移除或替换 `ActiveGameplayEffects` 使用 ECS remove/insert；不要通过可变引用直接赋值以绕过 hooks。
- Stale handle 返回 `None` 或 `Ok(false)`；容器重装更换 storage_id，槽位复用增加 generation，达到 `u32::MAX` 后退休。
- `EffectPayload::get_source()` 返回的实体才用于来源属性和标签查询；instigator 与 causer 是元数据，不能代替
  source 支付 cost 或读取来源状态。
- 正周期执行失败、Requirement 转换失败或到期清理失败时，系统记录错误并强制移除效果，避免
  永久重试同一无效状态。
- 两阶段 API 是同一逻辑状态内的短暂优化边界，不是长期命令或事务。
