# 02 — 插件系统与生命周期

## 插件一览

插件与 FixedUpdate 调度实现在 `src/gas/runtime_plugin.rs`；`lib.rs` 只负责 crate 门面和公共
重导出。

| 插件                                 | 类型          | 用途                                              |
| ------------------------------------ | ------------- | ------------------------------------------------- |
| `UniqueNamePlugin`                   | `Plugin`      | 初始化 `UniqueNamePool` 资源                      |
| `GameplayTagPlugin`                  | `Plugin`      | 初始化 `GameplayTagManager` 资源                  |
| `RandomPlugin`                       | `Plugin`      | 初始化 `Random` 资源（种子随机数）                |
| `GameplayAbilitySystemRuntimePlugin` | `Plugin`      | 核心运行时：初始化所有资源，注册 FixedUpdate 系统 |
| `GameplayAbilitySystemPlugin`        | `PluginGroup` | 组合以上四个插件                                  |

### 使用方式

```rust
use bevy::prelude::*;
use bevy_tools::prelude::*;

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

Plugin 负责资源和调度，不会自动为游戏实体安装 GAS Component。完整角色应显式生成
`GameplayAbilitySystemBundle`；只需要 Tags 或 Attributes 的实体可以只附加对应 Component。

## FixedUpdate 系统管线

`GameplayAbilitySystemRuntimePlugin` 通过有序 `SystemSet` 组织 FixedUpdate 中的系统：

```
   EffectTicks
   (Duration → Requirement → Period → Requirement)
        │
        ▼
   AbilityTasks
   (推进等待任务并生产请求)
        │
        ▼
 RequestProducers
   (游戏层请求生产阶段)
        │
        ▼
    Targeting
   (目标请求管线)
        │
        ▼
PreGameplayConvergence
   (外部 Tag 变化收敛)
        │
        ▼
 GameplayResolve
   (统一 FIFO：效果 + 技能)
        │
        ▼
UpdateEffectTagRequirements
   (执行后再次收敛)
        │
        ▼
     Cleanup
   (已完成技能)
        │
        ▼
RecalculateAttributes
   (脏属性集重算)
```

### SystemSet 详情

| SystemSet                     | 包含的系统/用途                                               | 职责                                      |
| ----------------------------- | ------------------------------------------------------------- | ----------------------------------------- |
| `EffectTicks`                 | Duration → Requirement → Period → Requirement                 | 先处理过期与条件收敛，再决定周期执行      |
| `AbilityTasks`                | `tick_ability_tasks_system`                                   | 按稳定实体顺序推进等待任务并生产请求      |
| `RequestProducers`            | 游戏层自定义系统                                              | 当前 tick 请求的公共生产阶段              |
| `Targeting`                   | `process_targeting_request_queue_system`                      | 选择、过滤、排序目标并产生技能请求        |
| `PreGameplayConvergence`      | `update_active_effect_tag_requirements_system`                | 在消费请求前收敛外部 Tag 变化             |
| `GameplayResolve`             | `process_gameplay_execution_queue_system` (`run_if`)          | 按跨类型 FIFO 消费效果与技能请求          |
| `UpdateEffectTagRequirements` | `update_active_effect_tag_requirements_system`                | 执行后固定点收敛                          |
| `Cleanup`                     | `cleanup_finished_abilities_system`                           | 清理 Ending/Cancelled 状态的技能          |
| `RecalculateAttributes`       | `recalculate_attribute_sets_system`                           | 重算所有脏 `AttributeSet`                 |

### RuntimePlugin 初始化的 Resource

| Resource                     | 类型       | 默认行为                                      |
| ---------------------------- | ---------- | --------------------------------------------- |
| `AttributeIdManager`         | `Resource` | ID 256；热点 32 / 冷区 224                    |
| `GameplayExecutionQueue`     | `Resource` | 技能与效果共享的跨类型 FIFO，每 tick 全量消费 |
| `TargetingRequestQueue`      | `Resource` | 每个 tick 按 FIFO 全量消费                    |

上表列出游戏层可直接使用的 Resource。运行时还维护 Requirement dirty 状态，以及用于同批次
deferred Active Ability 可见性的 pending overlay；二者属于内部收敛实现，不是公共 Gameplay API。

## 请求生产约定

需要在当前 `FixedUpdate` 执行的游戏层生产系统必须放入 `RequestProducers`，或显式位于
`GameplayResolve` 之前。多个生产系统之间若存在先后语义，应使用 `.chain()`、`.before()`
或 `.after()` 固定顺序。resolver 执行期间同步直接追加到 FIFO 的派生请求仍会由本次 drain 消费；
在 `GameplayResolve` 之后才由其他系统写入的请求，则明确定义为下一 tick 处理。这是阶段
语义，不是按负载随机分帧。

统一请求类型、完整 drain 和同步 API 边界详见
[16 — Gameplay 执行模块](./16-gameplay-execution.md)。

## 全局设置

定义在 `GameplayAbilitySystemSettings` 中：

| 常量                      | 值  | 说明                             |
| ------------------------- | --- | -------------------------------- |
| `ATTRIBUTE_SET_SIZE`      | 256 | 每个 `AttributeSet` 的最大属性数 |
| `HOT_ATTRIBUTE_SET_SIZE`  | 32  | 热点属性区域容量                 |
| `COLD_ATTRIBUTE_SET_SIZE` | 224 | 冷属性区域容量                   |
| `GAMEPLAY_TAG_SIZE`       | 512 | 最大 Gameplay 标签数             |
| `ABILITY_CHAIN_MAX_DEPTH` | 8   | 链式技能激活的最大深度           |
