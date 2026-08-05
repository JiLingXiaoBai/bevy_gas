# 02 — 插件系统与生命周期

## 插件一览

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
use bevy_tools::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GameplayAbilitySystemPlugin)
        .add_systems(Startup, register_initial_tags)
        .run();
}
```

## FixedUpdate 系统管线

`GameplayAbilitySystemRuntimePlugin` 通过有序 `SystemSet` 组织 FixedUpdate 中的系统：

```
UpdateEffectTagRequirements
        │
        ▼
   EffectTicks
   (持续时间 + 周期)
        │
        ▼
   AbilityTasks
   (推进任务进度)
        │
        ▼
    Targeting
   (目标请求管线)
        │
        ▼
     Queues
   (效果应用 + 技能激活)
        │
        ▼
     Cleanup
   (已完成技能 + 索引清理)
        │
        ▼
RecalculateAttributes
   (脏属性集重算)
```

### SystemSet 详情

| SystemSet                     | 包含的系统                                                  | 职责                             |
| ----------------------------- | ----------------------------------------------------------- | -------------------------------- |
| `UpdateEffectTagRequirements` | `update_active_effect_tag_requirements_system`              | 检查效果的持续/移除标签条件      |
| `EffectTicks`                 | `tick_effect_duration_system`                               | 倒计时并过期持续效果             |
|                               | `tick_effect_period_system`                                 | 周期性执行修饰器                 |
| `AbilityTasks`                | `tick_ability_tasks_system`                                 | 推进技能任务 (等待/立即)         |
| `Targeting`                   | `process_targeting_request_queue_system`                    | 选择、过滤、排序目标并延续请求   |
| `Queues`                      | `process_gameplay_effect_application_queue_system` (run_if) | 消费效果应用队列 (有工作时)      |
|                               | `process_ability_activation_queue_system` (run_if)          | 消费技能激活队列 (有工作时)      |
| `Cleanup`                     | `cleanup_finished_abilities_system`                         | 清理 Ending/Cancelled 状态的技能 |
|                               | `reconcile_active_effect_target_index_system`               | 从索引中清除已移除的效果         |
| `RecalculateAttributes`       | `recalculate_attribute_sets_system`                         | 重算所有脏 `AttributeSet`        |

### RuntimePlugin 初始化的 Resource

| Resource                          | 类型       | 默认行为                     |
| --------------------------------- | ---------- | ---------------------------- |
| `AttributeIdManager`              | `Resource` | ID 256；热点 32 / 冷区 224   |
| `AbilityActivationQueue`          | `Resource` | 每个 tick 按 FIFO 全量消费   |
| `GameplayEffectApplicationQueue`  | `Resource` | 每个 tick 按 FIFO 全量消费   |
| `TargetingRequestQueue`           | `Resource` | 每个 tick 按 FIFO 全量消费   |
| `ActiveGameplayEffectTargetIndex` | `Resource` | —                            |

## 全局设置

定义在 `GameplayAbilitySystemSettings` 中：

| 常量                      | 值  | 说明                             |
| ------------------------- | --- | -------------------------------- |
| `ATTRIBUTE_SET_SIZE`      | 256 | 每个 `AttributeSet` 的最大属性数 |
| `HOT_ATTRIBUTE_SET_SIZE`  | 32  | 热点属性区域容量                 |
| `COLD_ATTRIBUTE_SET_SIZE` | 224 | 冷属性区域容量                   |
| `GAMEPLAY_TAG_SIZE`       | 512 | 最大 Gameplay 标签数             |
| `ABILITY_CHAIN_MAX_DEPTH` | 8   | 链式技能激活的最大深度           |
