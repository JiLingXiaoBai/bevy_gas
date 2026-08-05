# 09 — 技能系统组件 (ASC)

## 概述

`AbilitySystemComponent` (ASC) 是挂载到可使用技能的实体上的主要 `Component`。它管理已授予的技能、追踪阻止标签，并提供核心激活逻辑。

实现直接在 `ability_system/` 下组织 Component、SystemParam、activation、commit 和
lifecycle；activation 内部再按 error、validation、startup 和 execution 拆分。完整所有权边界见
[17 — 源码布局与维护边界](./17-source-layout-and-maintenance.md)。

## `AbilitySystemComponent`

ASC 可以独立默认构造。它不再通过 Required Component 隐式安装 Tags、Attributes 或 Active
Effects；其私有存储维护 Ability 规格、Handle 索引、活跃计数和阻止标签，这些字段不是公共契约。

需要完整 GAS 运行时能力的实体应显式生成 `GameplayAbilitySystemBundle`：

```rust
commands.spawn(GameplayAbilitySystemBundle::default());
```

Bundle 统一包含 `AbilitySystemComponent`、`AttributeSet`、`GameplayTagContainer` 和
`ActiveGameplayEffects`。只需要标签或属性的实体仍可单独使用相应 Component，且不会意外获得
Effect 存储。

### 主要方法

| 方法                                                        | 说明                                 |
| ----------------------------------------------------------- | ------------------------------------ |
| `give_ability(ability, level, input_id)`                    | 授予新技能；返回 `AbilitySpecHandle` |
| `clear_ability(handle)`                                     | 不存在或仍有活跃实例时返回 `false`；否则移除 |
| `get_ability_specs() -> &[GameplayAbilitySpec]`             | 获取所有已授予的技能规格             |
| `find_ability_spec(handle) -> Option<&GameplayAbilitySpec>` | 按句柄查找规格                       |
| `get_blocked_ability_tags() -> &GameplayTagContainer`       | 获取阻止标签容器                     |

### 内部生命周期

激活流程负责设置阻止标签、递增活跃计数并生成 `ActiveGameplayAbility`；结束、取消或后续步骤失败
时，生命周期模块对这些变化进行对称清理或回滚。具体私有函数名称不是知识库契约。

## `AbilitySystemParams`

Ability 编排使用的 `SystemParam` 聚合器。Effect 运行时访问被收敛到独立的
`EffectSystemParams` 中：

```rust
pub struct AbilitySystemParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub effects: EffectSystemParams<'w, 's>,
    pub asc_query: Query<'w, 's, &'static mut AbilitySystemComponent>,
    pub attr_set_snapshot_query: Query<'w, 's, &'static AttributeSetSnapshot>,
    pub active_ability_query:
        Query<'w, 's, (Entity, &'static mut ActiveGameplayAbility)>,
}
```

`AbilitySystemParams` 对 `EffectSystemParams` 实现 `Deref` / `DerefMut`，因此 Ability 编排可以
直接调用接收 `&mut EffectSystemParams` 的 Effect API，同时 Effect 模块不再依赖 ASC Query。
实现还包含 pending Active Ability overlay，用于同一结算 batch 的 deferred 可见性。

Ability API 使用 `&mut AbilitySystemParams`；独立 Effect API 使用更窄的
`&mut EffectSystemParams`。Gameplay 请求生产系统应单独声明
`ResMut<GameplayExecutionQueue>`；
`AbilitySystemParams` 不锁定全局队列，从而避免无关效果查询降低其他系统并行度。

## 激活函数

### `try_activate_ability_by_handle`

独立同步激活入口。常规运行时系统应写入 `GameplayExecutionQueue`；完整同步流程见
[07 — Gameplay 技能](./07-gameplay-abilities.md#激活流程)。

### `can_activate_ability`

执行标签与数值费用的快速预检，不实际激活：

```rust
pub fn can_activate_ability(
    source: Entity,
    target: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams,
) -> bool;
```

它不检查授予句柄、活跃实例数、技能链、取消或 startup 预检，因此不保证同步激活入口一定成功。
来源缺少 `GameplayTagContainer` 时还会跳过 required、blocked 和 cooldown tag 检查；使用标签
条件的 ASC 实体应显式附加该 Component。

### `commit_ability`

独立应用技能定义的消耗 + 冷却，不要求也不会启动活跃技能实例：

```rust
pub fn commit_ability(
    source: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams,
) -> Result<(), AbilityCommitError>;
```

该函数使用 batch 内部执行路径，本身不调用 Requirement 固定点收敛。若作为独立同步入口使用，
且后续逻辑必须立即观察 cooldown granted tags 或抑制变化，调用方还需执行
`resolve_active_effect_tag_requirements()`；常规技能激活路径会自行完成收敛。

### `end_ability` / `cancel_ability`

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

只有句柄存在且实例来源与 `source` 一致时才返回 `true`。

## 清理系统

`cleanup_finished_abilities_system` 在 `Cleanup` 集合中运行：

1. 查询状态为 `Ending` 或 `Cancelled` 的 `ActiveGameplayAbility` 实体
2. 在来源 ASC 上调用内部 `finish_active_ability()`，递减计数并移除阻止标签
3. 销毁 `ActiveGameplayAbility` 实体

## 典型用法

```rust
struct PendingCast {
    player: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
}

#[derive(Resource, Default)]
struct PendingCasts {
    requests: std::collections::VecDeque<PendingCast>,
}

fn grant_and_queue_example(
    asc: &mut AbilitySystemComponent,
    player: Entity,
    target: Entity,
    ability: Arc<GameplayAbility>,
    pending: &mut PendingCasts,
) {
    let handle = asc.give_ability(ability, 1, Some(0));
    pending.requests.push_back(PendingCast {
        player,
        target,
        handle,
    });
}

fn queue_pending_casts(
    mut execution_queue: ResMut<GameplayExecutionQueue>,
    mut casts: ResMut<PendingCasts>,
) {
    while let Some(cast) = casts.requests.pop_front() {
        let chain = execution_queue.new_root_chain(cast.handle);
        let context = AbilityActivationContext::direct(cast.player, chain);

        execution_queue.push_activation(cast.player, cast.target, cast.handle, context);
    }
}

app.init_resource::<PendingCasts>().add_systems(
    FixedUpdate,
    queue_pending_casts.in_set(GameplayAbilitySystemSet::RequestProducers),
);
```

`PendingCasts` 是一次性消费缓冲，不会因 Resource 持续存在而每 tick 重复激活。若输入或 AI
系统也在同一个 `FixedUpdate` 写入该缓冲，必须让写入系统 `.before(queue_pending_casts)`，或将
二者 `.chain()` 后放入 `RequestProducers`，避免消费者先运行而产生一 tick 延迟。
