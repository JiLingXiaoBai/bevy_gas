# 09 — 技能系统组件（ASC）

## 职责

`AbilitySystemComponent`（ASC）是单个 Gameplay Actor 的技能目录和技能互斥状态。它拥有已授予
规格、Handle 索引和活跃技能写入的阻止标签，但不拥有属性、实体 Gameplay Tags 或 Active
Effects。激活、Commit、结束与清理由同一领域中的函数和系统编排。

## 源码布局

```text
src/gas/
├── ability_system.rs
└── ability_system/
    ├── component.rs
    ├── params.rs
    ├── activation.rs
    ├── activation/
    │   ├── error.rs
    │   ├── validation.rs
    │   ├── startup.rs
    │   └── execution.rs
    ├── commit.rs
    └── lifecycle.rs
```

| 文件 | 职责 |
| ---- | ---- |
| `component.rs` | ASC 公共存储 API 与显式 Actor Bundle |
| `params.rs` | Ability SystemParam 与 deferred Active Ability overlay |
| `activation/validation.rs` | 快速预检和完整激活要求检查 |
| `activation/startup.rs` | 创建活跃实例和启动 startup tasks |
| `activation/execution.rs` | 同步入口与 batch 内激活顺序 |
| `commit.rs` | Cost/Cooldown Plan 的准备、验证和执行 |
| `lifecycle.rs` | End、Cancel、标签取消和 Cleanup |

这些文件的私有函数是实现细节；公共入口由 `ability_system.rs` 显式重导出。

## Actor 与 World 的两层组合

### `GameplayAbilitySystemBundle`

ASC、Tags、Attributes 和 Active Effects 之间没有 `#[require(...)]` 反向依赖。完整 Gameplay
Actor 应显式生成：

```rust
#[derive(Bundle, Default)]
pub struct GameplayAbilitySystemBundle {
    pub ability_system: AbilitySystemComponent,
    pub attributes: AttributeSet,
    pub tags: GameplayTagContainer,
    pub active_effects: ActiveGameplayEffects,
}
```

```rust
commands.spawn(GameplayAbilitySystemBundle::default());
```

只需要标签或属性的实体可以单独附加对应 Component，不会自动获得 Active Effects。只附加 ASC
也合法，适合不使用标签条件、Cost、Cooldown 或 Effects 的简单技能；一旦技能依赖这些领域，
调用方必须组合相应 Component。

### `GameplayAbilitySystemPlugin`

Bundle 只组合单个实体上的 Component，不安装全局资源或 FixedUpdate 系统。应用还需要添加
`GameplayAbilitySystemPlugin`。该 Plugin Group 安装名称池、Tag Manager、确定性随机资源、
Attribute ID Manager、两个请求队列、Requirement/Pending 运行时资源以及完整调度管线。

```rust
app.add_plugins(GameplayAbilitySystemPlugin);
```

因此，“完整 GAS”需要同时满足：World 安装 Plugin，参与完整运行时的 Actor 使用 Bundle。

## `AbilitySystemComponent` 公共 API

```rust
pub fn give_ability(
    &mut self,
    ability: Arc<GameplayAbility>,
    level: u32,
    input_id: Option<u16>,
) -> AbilitySpecHandle;

pub fn clear_ability(&mut self, handle: AbilitySpecHandle) -> bool;
pub fn get_ability_specs(&self) -> &[GameplayAbilitySpec];
pub fn find_ability_spec(&self, handle: AbilitySpecHandle)
    -> Option<&GameplayAbilitySpec>;
pub fn get_blocked_ability_tags(&self) -> &GameplayTagContainer;
```

- Handle 从 ASC 局部的递增 `u32` 分配。
- `clear_ability()` 在 Handle 不存在或规格仍有活跃实例时返回 `false`。
- `get_blocked_ability_tags()` 返回技能互斥容器，不是实体自己的 Gameplay Tag 容器。
- 可变规格查找、阻止标签写入和索引重建均为领域内部实现，不是公共扩展点。

## SystemParam 组合

### `EffectSystemParams`

Effect 领域使用更窄的 SystemParam：

| 字段 | 类型 | 用途 |
| ---- | ---- | ---- |
| `tag_manager` | `Res<GameplayTagManager>` | Tag 注册与层级位集 |
| `random_gen` | `ResMut<Random>` | 概率效果的确定性随机源 |
| `attribute_id_manager` | `Res<AttributeIdManager>` | Attribute ID 到存储位置的映射 |
| `attr_set_query` | `Query<&mut AttributeSet>` | 目标属性 |
| `tag_container_query` | `Query<&mut GameplayTagContainer>` | 来源/目标 Tags |
| `active_effect_query` | `Query<&mut ActiveGameplayEffects>` | Active Effect 存储 |
| 内部 Requirement sync | `ResMut<ActiveEffectRequirementSync>` | dirty 与固定点收敛状态 |

它不查询 ASC 或 Active Ability，也不持有 `Commands` 或全局 Gameplay FIFO。

### `AbilitySystemParams`

```rust
#[derive(SystemParam)]
pub struct AbilitySystemParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub effects: EffectSystemParams<'w, 's>,
    pub asc_query: Query<'w, 's, &'static mut AbilitySystemComponent>,
    pub attr_set_snapshot_query: Query<'w, 's, &'static AttributeSetSnapshot>,
    pub active_ability_query:
        Query<'w, 's, (Entity, &'static mut ActiveGameplayAbility)>,
    pub(crate) pending_active_abilities:
        ResMut<'w, PendingActiveGameplayAbilities>,
}
```

`AbilitySystemParams` 对 `EffectSystemParams` 实现 `Deref` / `DerefMut`，所以 Ability 编排可将
`&mut AbilitySystemParams` 自动借用为 Effect API 所需的 `&mut EffectSystemParams`。Effect
模块因此不需要反向依赖 ASC。

`PendingActiveGameplayAbilities` 虽因公共系统签名而可见，但带有 `#[doc(hidden)]`，属于运行时
管道，不应由游戏代码直接读写。它补足 `Commands::spawn()` 尚未 flush 时同一 Gameplay drain
中的 Active Ability 可见性。

`AbilitySystemParams` 不持有 `GameplayExecutionQueue`。生产请求的系统应单独声明
`ResMut<GameplayExecutionQueue>`；这也避免只调用 Effect 查询的系统被 ASC 访问无谓串行化。

## 公共编排 API

### 激活

```rust
pub fn try_activate_ability_by_handle(
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
    activation_context: AbilityActivationContext,
    params: &mut AbilitySystemParams,
) -> Result<(), AbilityActivationError>;
```

这是独立同步入口，会使用局部 Gameplay 队列完成 startup `Instant` 派生请求，但不与全局 FIFO
排序。常规运行时使用 `GameplayExecutionQueue::push_activation()`；完整流程见
[07 — Gameplay 技能](./07-gameplay-abilities.md#激活关键流程)。

### 快速预检

```rust
pub fn can_activate_ability(
    source: Entity,
    target: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams,
) -> bool;
```

该函数只检查标签/Cooldown 与数值 Cost，不检查技能是否已授予、Handle、技能链、多实例、取消
和 startup tasks，因此不是最终授权。Cost 计算上下文使用传入 `target`，但实际 Commit 把 Cost
应用到 `source`。来源缺少 `GameplayTagContainer` 时，required、blocked 和 cooldown tag
检查会跳过。

### 独立 Commit

```rust
pub fn commit_ability(
    source: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams,
) -> Result<(), AbilityCommitError>;
```

它准备并预验证 Cost/Cooldown Plan，然后先执行 Cost、再执行 Cooldown，不创建活跃技能实例。
Cost 必须是只含 Add Modifier 的 Instant Effect。当前支付检查会把每个已准备的 Modifier
分别与检查开始时的属性当前值比较；因此同一 Cost 不应为同一个属性配置多个扣减 Modifier，
否则各项可能分别通过检查，但合计扣减后仍把属性降到零以下。这是当前实现边界，不是组合扣减保证。

独立 `commit_ability()` 不调用 Requirement 固定点。调用方若必须在返回后立即观察 Cooldown
granted tags 或 Effect 抑制状态，应显式调用 `resolve_active_effect_tag_requirements()`；正常激活
路径和 Gameplay resolver 会在规定收敛点处理。

### End 与 Cancel

```rust
pub fn end_ability(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    params: &mut AbilitySystemParams,
) -> bool;

pub fn cancel_ability(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    params: &mut AbilitySystemParams,
) -> bool;
```

只有实例存在且来源匹配时返回 `true`。两个函数只把 live query 或 pending overlay 中的实例改为
`Ending` / `Cancelled`；默认插件稍后的 Cleanup 完成 bookkeeping 与销毁。

## 生命周期与 Cleanup

启动技能时，ASC 写入阻止标签、递增规格 `active_count`，并通过 `Commands` 生成
`ActiveGameplayAbility`。结束路径需要对称移除阻止标签和递减计数。

`cleanup_finished_abilities_system` 位于 `GameplayAbilitySystemSet::Cleanup`：

1. 清空上一次 drain 留下的 pending overlay。
2. 遍历状态为 `Ending` 或 `Cancelled` 的活跃实例。
3. 来源 ASC 存在时移除阻止标签、递减 `active_count`。
4. 递归销毁活跃实例及其任务子实体。
5. 来源 ASC 已不存在时仍销毁孤立活跃实例，但无法再修改原 ASC bookkeeping。

标签驱动的“激活新技能并取消旧技能”使用 batch 内部快速路径，会立即更新旧技能 bookkeeping
并排队销毁；普通 `cancel_ability()` 则等待 Cleanup。

## 默认 FixedUpdate 阶段

```text
EffectTicks
  Duration → Requirement → Period → Requirement
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

自定义输入、AI 或 Gameplay 请求生产系统应放入 `RequestProducers`。同一集合内多个 producer
如果需要稳定业务顺序，必须显式 `.chain()`、`.before()` 或 `.after()`；共享可变队列只会让
Bevy 串行执行，不会自动定义确定顺序。

## 典型用法

```rust
fn queue_cast(
    mut queue: ResMut<GameplayExecutionQueue>,
    cast: Res<PendingCast>,
) {
    let chain = queue.new_root_chain(cast.handle);
    let context = AbilityActivationContext::direct(cast.source, chain);
    queue.push_activation(cast.source, cast.target, cast.handle, context);
}

app.add_systems(
    FixedUpdate,
    queue_cast.in_set(GameplayAbilitySystemSet::RequestProducers),
);
```

真实输入缓冲应是一消费即移除的队列或带 submitted 状态的请求，避免 Resource 持续存在而每
tick 重复激活。

## 测试导航

| 测试文件 | 覆盖范围 |
| -------- | -------- |
| `tests/gas_tests/runtime_paths_test.rs` | Bundle 显式组合、插件阶段和同/下一 tick 边界 |
| `tests/gas_tests/abilities/activation.rs` | 激活要求、多实例和阻止标签 |
| `tests/gas_tests/abilities/commit.rs` | Cost/Cooldown 准备与执行 |
| `tests/gas_tests/abilities/lifecycle.rs` | Cancel、Cleanup、活跃计数与规格清除 |
| `tests/gas_tests/abilities/chaining.rs` | pending overlay 和 deferred 父技能取消 |

继续阅读：[07 — Gameplay 技能](./07-gameplay-abilities.md)、
[16 — Gameplay 执行模块](./16-gameplay-execution.md)。
