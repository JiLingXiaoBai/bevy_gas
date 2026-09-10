# 02 — 插件系统与生命周期

## 插件一览

插件组合与 `FixedUpdate` 调度实现在 `src/gas/runtime_plugin.rs`。该实现模块是 crate 内部模块；
公开类型由 `bevy_gas::gas` 和 crate root 显式重导出，其中常用的 Plugin Group、Runtime
Plugin 与 SystemSet 也位于 `bevy_gas::prelude`。

| 插件 | 类型 | 安装内容 |
| --- | --- | --- |
| `UniqueNamePlugin` | `Plugin` | 初始化 `UniqueNamePool` |
| `GameplayTagPlugin` | `Plugin` | 初始化 `GameplayTagManager` |
| `RandomPlugin` | `Plugin` | 初始化确定性 `Random` |
| `GameplayAbilitySystemRuntimePlugin` | `Plugin` | 初始化 GAS 运行时资源并注册 `FixedUpdate` 管线 |
| `GameplayAbilitySystemPlugin` | `PluginGroup` | 按顺序组合以上四个插件 |

通常应安装完整 Plugin Group：

```rust
use bevy::prelude::*;
use bevy_gas::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GameplayAbilitySystemPlugin)
        .add_systems(Startup, register_initial_tags)
        .run();
}

fn register_initial_tags(mut register: GameplayTagRegister) {
    if let Err(error) = register.request_or_register_tag("Effect.Debuff.Stun") {
        error!("failed to register gameplay tag: {error}");
    }
}
```

`GameplayAbilitySystemRuntimePlugin` 只有在应用同时提供 `GameplayTagManager` 和 `Random`
时才能单独安装：它的无条件 `FixedUpdate` 系统会借用这两个 Resource，即使当前没有活跃
Effect。它也不会初始化 `UniqueNamePool`。因此通常应安装完整 Plugin Group；确需自定义组合时，
必须显式安装等价的基础插件或 Resource，注册 Tag/Attribute 名称时还要提供
`UniqueNamePool`。

插件只安装 Resource 和调度，不会自动为游戏实体插入 GAS Component。完整 Gameplay Actor
应显式生成 `GameplayAbilitySystemBundle`；纯 Tags 或 Attributes 实体只需插入对应 Component：

```rust
fn spawn_actors(mut commands: Commands) {
    commands.spawn(GameplayAbilitySystemBundle::default());
    commands.spawn(GameplayTagContainer::default());
    commands.spawn(AttributeSet::default());
}
```

后两个实体不会因为 `GameplayTagContainer` 或 `AttributeSet` 而隐式获得
`ActiveGameplayEffects`。

## FixedUpdate 系统管线

`GameplayAbilitySystemRuntimePlugin` 使用 `GameplayAbilitySystemSet` 固定阶段顺序：

```text
EffectTicks
  Duration -> Requirement -> Period -> Requirement
    |
    v
AbilityTasks
    |
    v
RequestProducers
    |
    v
Targeting
    |
    v
PreGameplayConvergence
    |
    v
GameplayResolve
    |
    v
UpdateEffectTagRequirements
    |
    v
Cleanup
    |
    v
RecalculateAttributes
```

| SystemSet | 系统/用途 | 语义 |
| --- | --- | --- |
| `EffectTicks` | `tick_effect_duration_system` → Requirement → `tick_effect_period_system` → Requirement | 先处理到期和条件变化，再决定本 tick 周期执行 |
| `AbilityTasks` | `tick_ability_tasks_system` | 按稳定实体顺序推进任务，任务可产生 Gameplay 请求 |
| `RequestProducers` | 游戏层自定义系统 | 当前 tick 的公共 Gameplay 请求生产阶段 |
| `Targeting` | `process_targeting_request_queue_system`（`run_if`） | 消费目标请求并可继续产生技能激活请求 |
| `PreGameplayConvergence` | `update_active_effect_tag_requirements_system` | 在统一 FIFO 前收敛外部 Tag 变化 |
| `GameplayResolve` | `process_gameplay_execution_queue_system`（`run_if`） | 按跨类型 FIFO 完整消费 Effect 与 Ability 请求 |
| `UpdateEffectTagRequirements` | `update_active_effect_tag_requirements_system` | Gameplay 执行后再次收敛 |
| `Cleanup` | `cleanup_finished_abilities_system` | 清理 `Ending` / `Cancelled` 技能实例 |
| `RecalculateAttributes` | `recalculate_attribute_sets_system` | 重算 dirty hot/cold 属性槽位 |

这里的 Requirement 调用不是重复注册错误：Duration 到期、Period 执行和 Gameplay FIFO 都可能
改变 Tag 或 Active Effect 状态，阶段间收敛保证后续阶段观察到一致状态。

## Resource 所有权

| 安装者 | Resource | 公共用途 |
| --- | --- | --- |
| `UniqueNamePlugin` | `UniqueNamePool` | 字符串驻留；从 crate root 导入 |
| `GameplayTagPlugin` | `GameplayTagManager` | 标签注册表与继承位集 |
| `RandomPlugin` | `Random` | 带种子的随机源；从 crate root 导入 |
| Runtime Plugin | `AttributeIdManager` | 属性 ID 与冷热槽位注册表 |
| Runtime Plugin | `GameplayExecutionQueue` | Ability/Effect 共用的跨类型 FIFO |
| Runtime Plugin | `TargetingRequestQueue` | 目标抓取 FIFO |

Runtime Plugin 还初始化 `ActiveEffectRequirementSync` 与
`PendingActiveGameplayAbilities`。前者是私有的 Requirement dirty 状态；后者因出现在公共 Bevy
system 签名中保持可见但标记为 `#[doc(hidden)]`。它们都是运行时管线设施，不应作为游戏层
状态直接读写。

`EffectSystemParams` 与 `AbilitySystemParams` 是 SystemParam 访问边界，不是 Resource：

- Effect 的准备、应用、移除和 Requirement 收敛使用较窄的 `EffectSystemParams`；
- Ability 激活、commit 和生命周期编排使用 `AbilitySystemParams`，其内部包含
  `EffectSystemParams`，并额外访问 `Commands`、ASC 与 Active Ability；
- 请求生产系统通常只需要 `ResMut<GameplayExecutionQueue>`，不应无故声明完整参数集。

## 请求生产约定

希望在当前 `FixedUpdate` 结算的游戏层生产系统，应放入 `RequestProducers`，或显式排序到
`GameplayResolve` 之前。多个生产系统的入队顺序具有玩法语义时，必须使用 `.chain()`、
`.before()` 或 `.after()` 固定顺序；仅处于同一个 SystemSet 不保证并行系统之间的确定顺序。

resolver 会完整 drain 当前 FIFO，消费期间追加的派生请求也在同一次 drain 内继续处理。
`GameplayResolve` 之后才入队的请求会保留到下一 tick；这是公开阶段边界，不是按请求数量或
运行负载随机分帧。

统一请求类型、完整 drain 和同步 API 边界详见
[16 — Gameplay 执行模块](./16-gameplay-execution.md)。

## 全局设置

编译期容量与安全限制位于 `GameplayAbilitySystemSettings`：

| 常量 | 当前值 | 说明 |
| --- | ---: | --- |
| `ATTRIBUTE_SET_SIZE` | 256 | 每个 `AttributeSet` 的总属性槽位 |
| `HOT_ATTRIBUTE_SET_SIZE` | 32 | 内联 hot 属性槽位 |
| `COLD_ATTRIBUTE_SET_SIZE` | 224 | boxed cold 属性槽位，由前两项相减得到 |
| `GAMEPLAY_TAG_SIZE` | 512 | 可注册 Gameplay Tag 位数 |
| `ABILITY_CHAIN_MAX_DEPTH` | 8 | 技能链最大嵌套深度 |

设置、随机数与唯一名称的具体契约见
[10 — 支撑基础设施](./10-supporting-infrastructure.md)。
