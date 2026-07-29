# 14 — 扩展系统

## 新增修饰器操作

1. 在 `src/gas/modifiers/modifier.rs` 的 `ModifierOperation` 中添加变体
2. 在 `Aggregator::apply_modifier_spec()` 中处理——新增一个桶
3. 在 `Aggregator::remove_modifier_by_handle()` 中处理——清理新桶
4. 在 `Aggregator::reset()` 中处理——清空新桶
5. 在 `Aggregator::modifier_count()` 中处理——纳入新桶
6. 在 `default_executor()` 中更新新操作的求值逻辑
7. 在 `Attribute::modify_base_value()` 中处理即时修饰器支持

## 新增堆叠策略变体

1. 在 `src/gas/gameplay_effects/gameplay_effect.rs` 的相应 `Stack*` 枚举中添加变体
2. 在 `find_stackable_active_effect()` 中处理——匹配逻辑
3. 在 `execute_stack_existing_effect()` 中处理——应用逻辑
4. 在 `tests/gas_tests/gameplay_effects_test.rs` 中添加测试

## 新增属性

1. 通过 `AttributeIdRegister::request_or_register_attribute_id("MyAttribute", AttributeRegion::Hot/Cold)` 注册并确定全局存储区域
2. 从 `AttributeIdManager` Resource 取得统一管理器
3. 在 `AttributeSet::initialize_attribute(&manager, id, base_value, executor, clamp)` 中初始化
4. 通过返回的普通连续 `AttributeId` 引用；所有访问使用同一个管理器定位冷热槽位

## 新增 Gameplay 标签

1. 通过 `GameplayTagRegister::request_or_register_tag("Parent.Child")` 注册
2. 父标签递归自动注册
3. 通过返回的 `GameplayTag` 引用

## 新增系统

1. 将系统函数添加到对应模块
2. 在 `src/lib.rs` 的 `GameplayAbilitySystemRuntimePlugin::build()` 中注册
3. 使用 `.in_set()` 选择正确的 `GameplayAbilitySystemSet`
4. 如需排序，使用 `.before()` / `.after()`
5. 考虑添加 `run_if` 条件以提高效率

## 新增 AbilityTask 类型

1. 在 `src/gas/gameplay_abilities/ability_task.rs` 的 `AbilityTaskDef` 中添加变体
2. 在 `AbilityTaskKind` 中添加对应变体（如需运行时状态）
3. 在 `AbilityTaskDef::instantiate()` 中处理——创建运行时 `AbilityTask`
4. 在 `AbilityTask::tick()` 中处理——完成逻辑
5. 如需新的完成动作，在 `AbilityTaskOnFinishedDef` 中添加变体
6. 在 `AbilityTaskOnFinishedDef::instantiate()` 中处理
7. 在 `tick_ability_tasks_system()` 中处理——执行完成动作

## 设计约束

- **禁止 `unsafe`**，除非明确要求且充分论证
- **禁止 `panic!()`** 在库代码中——返回 `Result` 或使用 `debug_assert!`
- **禁止 `unwrap()` / `expect()`**，除非不变量可证明不可能被违反
- **ECS 优先** — 优先选择 Component、Resource、System、Event，而非 OOP 模式
- **最小依赖** — 任何新的第三方 crate 都需要论证
- **确定性** — 避免 HashMap 遍历顺序、平台相关浮点行为、隐藏全局状态
