# 11 — API 快速参考

## 公开 API 索引

### 插件

| 项                                   | 类型          | 模块     |
| ------------------------------------ | ------------- | -------- |
| `GameplayAbilitySystemPlugin`        | `PluginGroup` | `lib.rs` |
| `GameplayAbilitySystemRuntimePlugin` | `Plugin`      | `lib.rs` |
| `GameplayTagPlugin`                  | `Plugin`      | `lib.rs` |
| `UniqueNamePlugin`                   | `Plugin`      | `lib.rs` |
| `RandomPlugin`                       | `Plugin`      | `lib.rs` |
| `GameplayAbilitySystemSet`           | `SystemSet`   | `lib.rs` |

### Gameplay 标签

| 项                                | 类型          | 说明                    |
| --------------------------------- | ------------- | ----------------------- |
| `GameplayTag`                     | `struct`      | 标签句柄（u16 位索引）  |
| `GameplayTagContainer`            | `Component`   | 每实体位集 + 引用计数   |
| `GameplayTagManager`              | `Resource`    | 全局标签注册表          |
| `GameplayTagRegister`             | `SystemParam` | 按点分隔名称注册标签    |
| `GameplayTagError`                | `enum`        | 标签注册错误            |
| `GameplayTagBits`                 | `type alias`  | `[u64; MAX_TAG_BLOCKS]` |
| `tag_bits_from_tags`              | `fn`          | 从标签切片构建位集      |
| `tag_bits_from_tags_with_manager` | `fn`          | 构建含继承的位集        |
| `add_bit_with_tag`                | `fn`          | 设置单个位              |

### 属性

| 项                                  | 类型          | 说明                          |
| ----------------------------------- | ------------- | ----------------------------- |
| `AttributeId`                       | `struct`      | 属性句柄（u16）               |
| `AttributeIdManager`                | `Resource`    | 属性 ID、冷热槽位及计数管理器 |
| `AttributeIdRegister`               | `SystemParam` | 按名称注册属性 ID             |
| `AttributeIdError`                  | `enum`        | 注册错误                      |
| `AttributeLocation`                 | `struct`      | 属性的区域及区域内槽位       |
| `AttributeRegion`                   | `enum`        | `Hot` / `Cold` 存储分类      |
| `AttributeClamp`                    | `enum`        | Clamp 范围（None 或 Range）   |
| `AttributeSet`                      | `Component`   | 每实体属性集合                |
| `AttributePostExecute`              | `type alias`  | 修改后回调                    |
| `AttributeSnapshot`                 | `struct`      | 不可变属性值快照              |
| `AttributeSetSnapshot`              | `Component`   | 不可变完整属性集快照          |
| `recalculate_attribute_sets_system` | `fn`          | 系统：重算脏属性集            |

### 修饰器

| 项                             | 类型     | 说明                                   |
| ------------------------------ | -------- | -------------------------------------- |
| `ModifierOperation`            | `enum`   | Add / PercentAdd / Multiply / Override |
| `ModifierMagnitude`            | `enum`   | Flat(f64) 或 Calculated(trait)         |
| `ModifierMagnitudeCalculation` | `trait`  | 动态幅度计算                           |
| `Modifier`                     | `struct` | 修饰器定义                             |
| `ModifierSpec`                 | `struct` | 已解析的不可变修饰器                   |
| `AppliedModifier`              | `struct` | 带句柄的已应用修饰器                   |
| `Aggregator`                   | `struct` | 修饰器收集 + 求值                      |
| `default_executor`             | `fn`     | 默认聚合公式                           |

### Gameplay 效果

| 项                                                 | 类型         | 说明                                         |
| -------------------------------------------------- | ------------ | -------------------------------------------- |
| `GameplayEffect`                                   | `struct`     | 效果定义                                     |
| `GameplayEffectSpec`                               | `struct`     | 已解析的效果规格                             |
| `EffectDurationTicks`                              | `enum`       | Instant / DurationTicks / Infinite           |
| `EffectDurationTicksSpec`                          | `enum`       | 已解析的持续时间                             |
| `EffectPeriodTicks`                                | `struct`     | 周期性执行配置                               |
| `EffectPeriodTicksSpec`                            | `struct`     | 已解析的周期配置                             |
| `EffectTags`                                       | `struct`     | 完整标签配置                                 |
| `TagRequirements`                                  | `struct`     | 要求全部 / 忽略任一标签                      |
| `GameplayEffectImmunityQuery`                      | `struct`     | 免疫匹配查询                                 |
| `StackingPolicy`                                   | `struct`     | 堆叠配置                                     |
| `StackingType`                                     | `enum`       | None / AggregateBySource / AggregateByTarget |
| `StackMagnitudePolicy`                             | `enum`       | None / Linear                                |
| `StackDurationPolicy`                              | `enum`       | KeepExisting / RefreshOnSuccessfulStack      |
| `StackPeriodPolicy`                                | `enum`       | KeepCurrentTick / ResetOnSuccessfulStack     |
| `StackOverflowPolicy`                              | `enum`       | RejectApplication / RefreshDuration          |
| `StackExpirationPolicy`                            | `enum`       | RemoveAllStacks / RemoveSingleStack          |
| `EffectPayload`                                    | `struct`     | 效果执行元数据                               |
| `EffectContext`                                    | `struct`     | 计算用的世界查询包装                         |
| `ActiveGameplayEffect`                             | `Component`  | 运行时活跃效果                               |
| `ActiveEffectHandle`                               | `type alias` | 活跃效果的 `Entity` 句柄                     |
| `ActiveEffectDurationTicks`                        | `Component`  | 剩余持续时间 tick                            |
| `ActiveEffectPeriodTicks`                          | `Component`  | 周期 tick 追踪                               |
| `ActiveGameplayEffectTargetIndex`                  | `Resource`   | O(1) 目标 → 效果查找                         |
| `GameplayEffectApplicationPlan`                    | `struct`     | 准备好的效果应用计划                         |
| `GameplayEffectApplicationKind`                    | `enum`       | Instant / StackExisting / CreateActive       |
| `GameplayEffectApplicationQueue`                   | `Resource`   | 延迟效果应用队列                             |
| `GameplayEffectApplicationRequest`                 | `struct`     | 单个队列中的应用请求                         |
| `prepare_gameplay_effect`                          | `fn`         | 阶段 1：准备应用                             |
| `execute_gameplay_effect_plan`                     | `fn`         | 阶段 2：执行计划                             |
| `apply_gameplay_effect`                            | `fn`         | 便捷：准备 + 执行                            |
| `remove_active_effect`                             | `fn`         | 移除特定活跃效果                             |
| `remove_active_effects_with_tags`                  | `fn`         | 移除匹配标签的效果                           |
| `get_active_effects_on_target`                     | `fn`         | 查询目标上的活跃效果                         |
| `has_active_effect_with_tags`                      | `fn`         | 检查是否存在带标签的效果                     |
| `tick_effect_duration_system`                      | `fn`         | 系统：持续时间倒计时                         |
| `tick_effect_period_system`                        | `fn`         | 系统：周期性执行                             |
| `update_active_effect_tag_requirements_system`     | `fn`         | 系统：抑制/移除检查                          |
| `cleanup_active_gameplay_effect`                   | `fn`         | 移除效果 + 清理                              |
| `reconcile_active_effect_target_index_system`      | `fn`         | 系统：索引维护                               |
| `process_gameplay_effect_application_queue_system` | `fn`         | 系统：消费队列                               |
| `gameplay_effect_application_queue_has_work`       | `fn`         | 队列系统的运行条件                           |

### Gameplay 技能

| 项                         | 类型         | 说明                                                  |
| -------------------------- | ------------ | ----------------------------------------------------- |
| `GameplayAbility`          | `struct`     | 技能定义                                              |
| `GameplayAbilitySpec`      | `struct`     | 已授予的技能实例                                      |
| `AbilitySpecHandle`        | `struct`     | 技能规格句柄（u32）                                   |
| `AbilityTags`              | `struct`     | 技能标签配置                                          |
| `ActiveGameplayAbility`    | `Component`  | 运行时活跃技能                                        |
| `ActiveAbilityHandle`      | `type alias` | 活跃技能的 `Entity` 句柄                              |
| `AbilityActivationStatus`  | `enum`       | Activating / Active / Ending / Cancelled              |
| `AbilityActivationContext` | `struct`     | 激活元数据                                            |
| `AbilityActivationReason`  | `enum`       | Direct / Input / Chained / TaskEvent / GameplayEffect |
| `AbilityChainContext`      | `struct`     | 链追踪（深度 + 循环检测）                             |
| `AbilityChainError`        | `enum`       | 链验证错误                                            |
| `AbilityActivationError`   | `enum`       | 激活失败原因                                          |

### 技能任务

| 项                          | 类型        | 说明                            |
| --------------------------- | ----------- | ------------------------------- |
| `AbilityTaskDef`            | `enum`      | 任务定义（Instant / WaitTicks） |
| `AbilityTaskOnFinishedDef`  | `enum`      | 完成动作定义                    |
| `AbilityTask`               | `Component` | 运行时任务实体                  |
| `AbilityTaskKind`           | `enum`      | Instant / WaitTicks             |
| `AbilityTaskOnFinished`     | `enum`      | 运行时完成动作                  |
| `AbilityTaskEvent`          | `Event`     | EmitEvent 完成时发出            |
| `tick_ability_tasks_system` | `fn`        | 系统：推进所有任务              |

### 技能系统组件

| 项                                        | 类型          | 说明                   |
| ----------------------------------------- | ------------- | ---------------------- |
| `AbilitySystemComponent`                  | `Component`   | 每实体 ASC             |
| `AbilitySystemParams`                     | `SystemParam` | 聚合的 GAS 查询 + 资源 |
| `AbilityActivationQueue`                  | `Resource`    | 延迟激活队列           |
| `AbilityActivationRequest`                | `struct`      | 单个队列中的激活请求   |
| `try_activate_ability_by_handle`          | `fn`          | 主激活入口             |
| `can_activate_ability`                    | `fn`          | 检查而不激活           |
| `commit_ability`                          | `fn`          | 执行消耗 + 冷却        |
| `end_ability`                             | `fn`          | 将状态设为 Ending      |
| `cancel_ability`                          | `fn`          | 将状态设为 Cancelled   |
| `cleanup_finished_abilities_system`       | `fn`          | 系统：销毁已完成技能   |
| `process_ability_activation_queue_system` | `fn`          | 系统：消费队列         |
| `ability_activation_queue_has_work`       | `fn`          | 队列系统的运行条件     |

### 支撑

| 项                              | 类型       | 说明                  |
| ------------------------------- | ---------- | --------------------- |
| `UniqueName`                    | `struct`   | 驻留字符串句柄（u32） |
| `UniqueNamePool`                | `Resource` | 字符串驻留池          |
| `Random`                        | `Resource` | 种子确定性 RNG        |
| `GameplayAbilitySystemSettings` | `struct`   | 全局编译期常量        |
