# 14 — 扩展系统

## 新增修饰器操作

1. 在 `src/gas/modifiers/definition.rs` 的 `ModifierOperation` 中添加变体
2. 在 `src/gas/attributes/aggregation.rs` 的 `Aggregator::apply_modifier_spec()` 中处理——新增一个桶
3. 在 `Aggregator::remove_modifiers_by_source()` 中处理——清理新桶
4. 在 `Aggregator::reset()` 中处理——清空新桶
5. 在 `Aggregator::modifier_count()` 中处理——纳入新桶
6. 在 `default_executor()` 中更新新操作的默认求值逻辑
7. 在 `src/gas/attributes/attribute_set/mutation.rs` 中处理即时修饰器，并确保所有调用仍由 `AttributeSet` 负责设置 dirty bit

## 新增堆叠策略变体

1. 在 `src/gas/gameplay_effects/gameplay_effect/stacking.rs` 的相应 `Stack*` 枚举中添加变体
2. 在 `find_stackable_active_effect()` 中处理——匹配逻辑
3. 在 `execute_stack_existing_effect()` 中处理——应用逻辑
4. 在 `tests/gas_tests/effects/stacking.rs` 中添加测试

## 新增属性

1. 通过 `AttributeIdRegister::request_or_register_attribute_id("MyAttribute", AttributeRegion::Hot/Cold)` 注册并确定全局存储区域
2. 从 `AttributeIdManager` Resource 取得统一管理器
3. 在 `AttributeSet::initialize_attribute(&manager, id, base_value, executor)` 中初始化
4. 通过返回的普通连续 `AttributeId` 引用；所有访问使用同一个管理器定位冷热槽位

## 新增 Gameplay 标签

1. 通过 `GameplayTagRegister::request_or_register_tag("Parent.Child")` 注册
2. 父标签递归自动注册
3. 通过返回的 `GameplayTag` 引用

## 新增系统

1. 将系统函数添加到对应模块
2. 在 `src/gas/runtime_plugin.rs` 的 `GameplayAbilitySystemRuntimePlugin::build()` 中注册
3. 使用 `.in_set()` 选择正确的 `GameplayAbilitySystemSet`
4. 如需排序，使用 `.before()` / `.after()`
5. 考虑添加 `run_if` 条件以提高效率

### 生产 Gameplay 请求的系统

会调用 `GameplayExecutionQueue::push_activation()` 或 `push_application()` 的游戏层系统，
应放入 `GameplayAbilitySystemSet::RequestProducers`。这样请求能在同一个 `FixedUpdate` 的
`GameplayResolve` 阶段消费：

```rust
app.add_systems(
    FixedUpdate,
    produce_ai_ability_requests.in_set(GameplayAbilitySystemSet::RequestProducers),
);
```

如果多个生产系统的请求有先后语义，必须通过 `.chain()`、`.before()` 或 `.after()` 明确
排序；仅放入同一个 set 不代表跨系统 FIFO 顺序有稳定契约。Targeting、AI 决策和寻路完成
系统可以并行计算自己的只读结果，但向 Gameplay FIFO 提交有顺序依赖的请求时应进入明确
排序的生产阶段。

位于 `GameplayResolve` 之后的生产系统所写请求会保留到下一 tick。这是公开的阶段边界，
不应依赖系统负载或队列长度改变。

完整的生产/消费契约、同步入口边界和 AI/异步结果接入规则见
[16 — Gameplay 执行模块](./16-gameplay-execution.md)。

## 新增 AbilityTask 类型

1. 在 `src/gas/gameplay_abilities/ability_task/definition.rs` 的 `AbilityTaskDef` 中添加定义变体
2. 若任务需要跨 tick 状态，在 `AbilityTaskKind` 中添加运行时变体，并更新
   `AbilityTaskDef::instantiate()` 与 `src/gas/gameplay_abilities/ability_task/state.rs`
3. 更新 `src/gas/ability_system/activation/startup.rs` 中的
   `start_startup_ability_tasks()`，明确新任务在技能
   激活时是立即派发，还是创建任务实体后由后续 tick 推进
4. 如需新的完成动作，在 `AbilityTaskOnFinishedDef` 和 `AbilityTaskOnFinished` 中添加变体，
   并更新 `AbilityTaskOnFinishedDef::instantiate()`
5. 在 `ability_task/completion.rs` 的 `dispatch_ability_task_completion()` 中实现完成动作；
   `ability_task/ticking.rs` 的 `tick_ability_tasks_system()` 只负责按稳定实体顺序推进任务并调用该分派函数
6. 分别添加 startup 路径和运行时 tick 路径测试，验证执行 tick、FIFO 顺序及结束语义

## 设计约束

- **禁止 `unsafe`**，除非明确要求且充分论证
- **运行时代码禁止 `panic!()`、`unwrap()` 和 `expect()`**——可恢复失败返回 `Result`，
  内部不变量使用 `debug_assert!`；测试可用 `unwrap()` / `expect()` 明确断言成功
- **ECS 优先** — 优先选择 Component、Resource、System、Event，而非 OOP 模式
- **最小依赖** — 任何新的第三方 crate 都需要论证
- **确定性** — 避免 HashMap 遍历顺序、平台相关浮点行为、隐藏全局状态
