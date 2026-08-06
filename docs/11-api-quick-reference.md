# 11 — API 快速参考

## 公开路径与门面边界

本页用于按领域定位公开入口，不替代编译器生成的 rustdoc。精确签名、参数、返回值和错误以
源码 rustdoc 为准；`#[doc(hidden)]` 的运行时管线类型不列为用户入口。

公开路径分为四层：

| 层级 | 示例 | 约定 |
| --- | --- | --- |
| 精简 prelude | `bevy_tools::prelude::*`、`bevy_tools::gas::prelude::*` | 只包含常用 Plugin、Component、定义、队列和 SystemParam |
| 领域门面 | `bevy_tools::gas::gameplay_effects::GameplayEffect` | 专项 API 的规范所有者 |
| GAS 聚合门面 | `bevy_tools::gas::GameplayEffect` | 对 GAS 领域公开项做显式聚合 |
| crate root | `bevy_tools::GameplayEffect` | 显式兼容重导出；另拥有 `Random` 与 `UniqueName*` 支撑类型 |

`lib.rs` 还显式重导出 `ability_system`、`attributes`、`gameplay_abilities`、
`gameplay_effects`、`gameplay_execution`、`gameplay_tags`、`gameplay_targeting`、
`modifiers` 与 `settings` 模块，因此旧的 `bevy_tools::gameplay_effects::...` 路径仍可用。
所有门面都禁止 `pub use *`；新增内部 `pub` 项不会自动成为公开 API。

### Prelude 的精确范围

当前 prelude 包含：

| 领域 | 项 |
| --- | --- |
| Runtime | `GameplayAbilitySystemPlugin`、`GameplayAbilitySystemRuntimePlugin`、`GameplayAbilitySystemSet` |
| Ability System | `AbilitySystemComponent`、`AbilitySystemParams`、`GameplayAbilitySystemBundle` |
| Attributes | `AttributeId`、`AttributeIdRegister`、`AttributeRegion`、`AttributeSet` |
| Abilities | `AbilityActivationContext`、`AbilitySpecHandle`、`AbilityTags`、`GameplayAbility` |
| Effects | `ActiveGameplayEffects`、`EffectDurationTicks`、`EffectPayload`、`EffectSystemParams`、`EffectTags`、`GameplayEffect`、`StackingPolicy` |
| Execution | `GameplayExecutionQueue` |
| Tags | `GameplayTag`、`GameplayTagContainer`、`GameplayTagRegister`、`TagRequirements` |
| Targeting | `AbilityTargetData`、`TargetingDefinition`、`TargetingRequestQueue` |
| Modifiers | `Modifier`、`ModifierEvaluationContext`、`ModifierMagnitude`、`ModifierMagnitudeCalculation`、`ModifierOperation` |

错误类型、低层 handle/spec、管理器、独立基础插件、系统函数与支撑类型不在 prelude；请从拥有它
的领域门面或 crate root 显式导入。

### 插件

| 项 | 类型 | Prelude | 公开路径 |
| --- | --- | --- | --- |
| `GameplayAbilitySystemPlugin` | `PluginGroup` | 是 | `bevy_tools::gas` / crate root |
| `GameplayAbilitySystemRuntimePlugin` | `Plugin` | 是 | `bevy_tools::gas` / crate root |
| `GameplayAbilitySystemSet` | `SystemSet` | 是 | `bevy_tools::gas` / crate root |
| `GameplayTagPlugin` | `Plugin` | 否 | `bevy_tools::gas` / crate root |
| `UniqueNamePlugin` | `Plugin` | 否 | `bevy_tools::gas` / crate root |
| `RandomPlugin` | `Plugin` | 否 | `bevy_tools::gas` / crate root |

### Gameplay 标签

| 项                                | 类型          | 说明                    |
| --------------------------------- | ------------- | ----------------------- |
| `GameplayTag`                     | `struct`      | 标签句柄（u16 位索引）  |
| `GameplayTagContainer`            | `Component`   | 每实体位集 + 引用计数   |
| `GameplayTagManager`              | `Resource`    | 全局标签注册表          |
| `GameplayTagRegister`             | `SystemParam` | 按点分隔名称注册标签    |
| `TagRequirements`                 | `struct`      | 要求全部 / 忽略任一标签 |
| `GameplayTagError`                | `enum`        | 标签注册错误            |
| `MAX_TAG_COUNTS`                  | `const`       | 可注册标签总容量        |
| `BLOCK_SIZE_EXPONENT`             | `const`       | 位块索引右移量（6）     |
| `TAG_BITS_PER_BLOCK`              | `const`       | 每个标签位块的位数（64）|
| `MAX_TAG_BLOCKS`                  | `const`       | `GameplayTagBits` 的 u64 块数 |
| `GameplayTagBits`                 | `type alias`  | `[u64; MAX_TAG_BLOCKS]` |
| `tag_bits_from_tags`              | `fn`          | 构建精确位集，失败时返回 `GameplayTagError` |
| `tag_bits_from_tags_with_manager` | `fn`          | 构建含继承的位集，失败时返回 `GameplayTagError` |
| `add_bit_with_tag`                | `fn`          | 设置单个位，索引无效时返回 `InvalidTagIndex` |

### 属性

| 项                                  | 类型          | 说明                          |
| ----------------------------------- | ------------- | ----------------------------- |
| `AttributeId`                       | `struct`      | 属性句柄（u16）               |
| `AttributeIdManager`                | `Resource`    | 属性 ID、冷热槽位及计数管理器 |
| `AttributeIdRegister`               | `SystemParam` | 按名称注册属性 ID             |
| `AttributeIdError`                  | `enum`        | 注册错误                      |
| `AttributeSetError`                 | `enum`        | 属性写入时的 ID 或未初始化错误 |
| `AttributeLocation`                 | `struct`      | 属性的区域及区域内槽位       |
| `AttributeRegion`                   | `enum`        | `Hot` / `Cold` 存储分类      |
| `AttributeSet`                      | `Component`   | 每实体属性集合                |
| `AttributePostExecute`              | `type alias`  | 修改后回调                    |
| `AttributeSnapshot`                 | `struct`      | 不可变属性值快照              |
| `AttributeSetSnapshot`              | `Component`   | 不可变完整属性集快照          |
| `ATTRIBUTE_SET_SIZE`                | `const`       | 总属性容量                    |
| `HOT_ATTRIBUTE_SET_SIZE`            | `const`       | 热点属性槽位容量              |
| `COLD_ATTRIBUTE_SET_SIZE`           | `const`       | 冷属性槽位容量                |
| `recalculate_attribute_sets_system` | `fn`          | 系统：重算脏属性集            |

### 修饰器

| 项                             | 类型     | 说明                                   |
| ------------------------------ | -------- | -------------------------------------- |
| `ModifierOperation`            | `enum`   | Add / PercentAdd / Multiply / Override |
| `ModifierMagnitude`            | `enum`   | Flat(f32) 或 Calculated(trait)         |
| `ModifierMagnitudeCalculation` | `trait`  | 动态幅度计算                           |
| `ModifierEvaluationContext`    | `trait`  | 与 Effect runtime 解耦的只读求值接口   |
| `Modifier`                     | `struct` | 修饰器定义                             |
| `ModifierSpec`                 | `struct` | 已解析的不可变修饰器                   |
| `ModifierSourceId`             | `struct` | 中立的 scope + slot + generation 来源 ID |
| `AppliedModifier`              | `struct` | 带中立来源 ID 的已应用修饰器           |
| `Aggregator`                   | `struct` | 修饰器收集 + 求值                      |
| `default_executor`             | `fn`     | Aggregator 默认求值公式                 |

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
| `EffectSystemParams`                               | `SystemParam`| Effect 专用查询与资源边界                    |
| `ActiveGameplayEffects`                            | `Component`  | 目标持有的稳定槽位效果容器                   |
| `ActiveGameplayEffect`                             | `struct`     | 容器中的运行时效果                           |
| `ActiveEffectHandle`                               | `struct`     | target + slot + generation 稳定句柄          |
| `ActiveEffectDurationTicks`                        | `struct`     | 容器内的剩余持续时间 tick                    |
| `ActiveEffectPeriodTicks`                          | `struct`     | 容器内的周期 tick 状态                       |
| `GameplayEffectApplicationPlan`                    | `struct`     | 准备好的效果应用计划                         |
| `GameplayEffectApplicationError`                   | `enum`       | 效果准备或执行失败的具体原因                 |
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
| `resolve_active_effect_tag_requirements`           | `fn`         | 同步执行确定性固定点收敛                     |

Effect 的准备、应用、移除与同步 Requirement API 接收 `&mut EffectSystemParams`。该参数只包含
Tag/Attribute manager、确定性随机、Attribute/Tag/Active Effect 查询和内部 dirty 状态，不包含
ASC、Active Ability 或 `Commands`。`AbilitySystemParams` 内嵌它并实现 `Deref` / `DerefMut`，
但新的 Effect-only system 与测试应优先直接声明窄参数。

`TagRequirements` 由 Gameplay Tags 领域拥有；Gameplay Effects 门面保留同一类型的兼容重导出。

### Gameplay 执行

| 项                                          | 类型       | 说明                                  |
| ------------------------------------------- | ---------- | ------------------------------------- |
| `GameplayExecutionQueue`                    | `Resource` | 技能与效果共享的跨类型 FIFO           |
| `GameplayExecutionRequest`                  | `enum`     | ActivateAbility / ApplyGameplayEffect |
| `AbilityActivationRequest`                  | `struct`   | 激活全过程共享的标准输入              |
| `GameplayEffectApplicationRequest`          | `struct`   | 捕获后的效果应用请求                  |
| `process_gameplay_execution_queue_system`   | `fn`       | 系统：完整 drain 并按需收敛 Tag 条件  |
| `gameplay_execution_queue_has_work`         | `fn`       | 统一 FIFO 的运行条件                   |

两个请求类型由 `gameplay_execution` 领域拥有。`ability_system` 与 `gameplay_effects` 门面分别
兼容重导出对应请求类型；新代码需要同时处理两类请求时，应从 Execution 门面导入。

### Gameplay 目标抓取

| 项                                      | 类型        | 说明                                      |
| --------------------------------------- | ----------- | ----------------------------------------- |
| `Targetable`                            | `Component` | 标记可被非自身 Selection 选中的实体       |
| `AbilityTargetHit`                      | `struct`    | 实体、世界位置和可选表面法线              |
| `AbilityTargetData`                     | `struct`    | 带原点的确定性有序命中集合                |
| `TargetingDefinition`                   | `struct`    | 已验证的有序目标操作管线                  |
| `TargetingOperation`                    | `enum`      | Selection / Filter / Sort / Limit 操作    |
| `TargetingSortOrder`                    | `enum`      | Ascending / Descending                    |
| `TargetingDefinitionError`              | `enum`      | 非法管线配置                              |
| `TargetingInput`                        | `struct`    | 捕获的原点、方向和显式目标                |
| `TargetingError`                        | `enum`      | 运行时抓取失败                            |
| `TargetingCandidateQuery`               | `type alias` | 目标选择使用的只读 ECS Query              |
| `TargetingRequestId`                    | `struct`    | 队列请求的稳定标识                        |
| `TargetingContinuation`                 | `enum`      | 只发结果或继续激活技能                    |
| `TargetingResultEvent`                  | `Event`     | 请求完成后由 `Commands::trigger` 触发的 Observer Event |
| `TargetingRequestQueue`                 | `Resource`  | 每 tick 全量消费的 FIFO 请求队列           |
| `acquire_targets`                       | `fn`        | 同步执行目标操作管线                      |
| `process_targeting_request_queue_system`| `fn`        | 系统：处理请求并分派 continuation         |
| `targeting_request_queue_has_work`      | `fn`        | 队列系统的运行条件                        |

### Gameplay 技能

| 项                         | 类型         | 说明                                                  |
| -------------------------- | ------------ | ----------------------------------------------------- |
| `GameplayAbility`          | `struct`     | 技能定义                                              |
| `GameplayAbilitySpec`      | `struct`     | 已授予的技能实例                                      |
| `AbilitySpecHandle`        | `struct`     | 技能规格句柄（u32）                                   |
| `AbilityTags`              | `struct`     | 技能标签配置                                          |
| `ActiveGameplayAbility`    | `Component`  | 运行时活跃技能                                        |
| `ActiveAbilityHandle`      | `type alias` | 活跃技能的 `Entity` 句柄                              |
| `AbilityActivationStatus`  | `enum`       | Active / Ending / Cancelled                           |
| `AbilityActivationContext` | `struct`     | 激活元数据及可选 `AbilityTargetData`                  |
| `AbilityActivationReason`  | `enum`       | Direct / Input / Chained / TaskEvent / GameplayEffect |
| `AbilityChainContext`      | `struct`     | 链追踪（深度 + 循环检测）                             |
| `AbilityChainError`        | `enum`       | 链验证错误                                            |
| `AbilityActivationError`   | `enum`       | 激活失败原因                                          |
| `AbilityCommitError`       | `enum`       | Cost/Cooldown 准备与执行失败原因                      |

### 技能任务

| 项                          | 类型        | 说明                            |
| --------------------------- | ----------- | ------------------------------- |
| `AbilityTaskDef`            | `enum`      | 任务定义（Instant / WaitTicks） |
| `AbilityTaskOnFinishedDef`  | `enum`      | 完成动作定义                    |
| `AbilityTask`               | `Component` | 运行时任务实体                  |
| `AbilityTaskKind`           | `enum`      | Instant / WaitTicks             |
| `AbilityTaskOnFinished`     | `enum`      | 运行时完成动作                  |
| `AbilityTaskEvent`          | `Event`     | EmitEvent 完成时触发的 Observer Event |
| `tick_ability_tasks_system` | `fn`        | 系统：推进所有任务              |

### 技能系统组件

| 项                                        | 类型          | 说明                   |
| ----------------------------------------- | ------------- | ---------------------- |
| `AbilitySystemComponent`                  | `Component`   | 每实体 ASC             |
| `GameplayAbilitySystemBundle`             | `Bundle`      | 显式组合完整 GAS Actor 组件 |
| `AbilitySystemParams`                     | `SystemParam` | Effect 参数 + Ability 编排状态 |
| `try_activate_ability_by_handle`          | `fn`          | 独立的同步激活调用路径        |
| `can_activate_ability`                    | `fn`          | 检查而不激活           |
| `commit_ability`                          | `fn`          | 执行消耗 + 冷却        |
| `end_ability`                             | `fn`          | 将状态设为 Ending      |
| `cancel_ability`                          | `fn`          | 将状态设为 Cancelled   |
| `cleanup_finished_abilities_system`       | `fn`          | 系统：销毁已完成技能   |

`GameplayAbilitySystemBundle` 显式组合 `AbilitySystemComponent`、`AttributeSet`、
`GameplayTagContainer` 与 `ActiveGameplayEffects`。这些 Component 本身没有反向
`#[require(ActiveGameplayEffects)]`，纯 Tags/Attributes 实体可以独立存在。

### 支撑

| 项                              | 类型       | 说明                  |
| ------------------------------- | ---------- | --------------------- |
| `UniqueName`                    | `struct`   | 驻留字符串句柄（u32）                 |
| `UniqueNameError`               | `enum`     | 名称句柄空间耗尽错误                   |
| `UniqueNamePool`                | `Resource` | 碰撞安全的字符串驻留池，注册返回 Result |
| `Random`                        | `Resource` | 种子确定性 RNG                        |
| `GameplayAbilitySystemSettings` | `struct`   | 全局编译期常量                        |

`UniqueName`、`UniqueNameError`、`UniqueNamePool` 与 `Random` 只从 crate root 公开，不在
`bevy_tools::gas` 或 prelude 中。`GameplayAbilitySystemSettings` 由 `gas::settings` 拥有，并由
`gas` 与 crate root 显式重导出；其常量为 `ATTRIBUTE_SET_SIZE`、
`HOT_ATTRIBUTE_SET_SIZE`、`COLD_ATTRIBUTE_SET_SIZE`、`GAMEPLAY_TAG_SIZE` 与
`ABILITY_CHAIN_MAX_DEPTH`。
