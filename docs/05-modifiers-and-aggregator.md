# 05 — 修饰器与聚合器

## 模块边界与文件结构

`modifiers` 是独立的顶层共享领域，同时服务于 Attributes 和 Gameplay Effects：

- `modifiers` 描述“修改什么、如何计算、计算后的值是什么”；
- Gameplay Effects 决定修改何时创建、持续、堆叠和移除；
- Attributes 负责保存、聚合、重算和回滚修改。

```text
src/gas/
├── modifiers.rs                  # Shared-domain facade and explicit exports
├── modifiers/
│   ├── definition.rs            # Definitions and magnitude calculation
│   ├── context.rs               # ModifierEvaluationContext
│   └── spec.rs                  # Evaluated specs and source identity
└── attributes/
    └── aggregation.rs           # Aggregator and internal sparse storage
```

`modifiers` 不依赖 `EffectContext`、`ActiveEffectHandle` 或 Active Effects 存储。
效果侧通过实现中立接口、转换中立来源 ID 与它衔接，因此修改器也可以被其他玩法系统复用。

## 修饰器操作

| 操作         | 效果               | 典型用例       |
| ------------ | ------------------ | -------------- |
| `Add`        | 加上固定值         | `+50 HP`       |
| `PercentAdd` | 加上当前值的百分比 | `+20% 攻击力`  |
| `Multiply`   | 乘以系数           | `×1.5 速度`    |
| `Override`   | 完全覆盖该值       | `将 HP 设为 1` |

## 修饰器类型

### `Modifier`

定义层级的修饰器。Gameplay Effect 可以保存它，其他玩法定义也可以直接复用。

```rust
pub struct Modifier {
    id: AttributeId,
    op: ModifierOperation,
    magnitude: ModifierMagnitude,
}

impl Modifier {
    pub fn new(id: AttributeId, op: ModifierOperation, magnitude: ModifierMagnitude) -> Self;
    pub fn get_operation(&self) -> ModifierOperation;
    pub fn make_spec(&self, context: &dyn ModifierEvaluationContext) -> ModifierSpec;
}
```

### `ModifierMagnitude`

支持静态和动态值：

```rust
pub enum ModifierMagnitude {
    Flat(f32),
    Calculated(Box<dyn ModifierMagnitudeCalculation>),
}

pub trait ModifierMagnitudeCalculation: Send + Sync {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32;
}
```

`Calculated` 幅度只依赖 `ModifierEvaluationContext`，而不是某一种效果运行时上下文。
当前 `EffectContext` 实现了该接口，因此 Gameplay Effect 仍可直接创建
`ModifierSpec`，但 Modifiers 模块不会反向依赖 Gameplay Effects。

### `ModifierEvaluationContext`

动态幅度可读取一组稳定、只读且具有玩法语义的数据：

```rust
pub trait ModifierEvaluationContext {
    fn target(&self) -> Option<Entity>;
    fn source(&self) -> Entity;
    fn instigator(&self) -> Entity;
    fn causer(&self) -> Option<Entity>;
    fn level(&self) -> u32;
    fn source_snapshot(&self) -> Option<&AttributeSetSnapshot>;
    fn attribute_id_manager(&self) -> &AttributeIdManager;
    fn source_tags(&self) -> Option<&GameplayTagContainer>;
    fn target_tags(&self) -> Option<&GameplayTagContainer>;
}
```

接口没有暴露 ECS `Query`、完整 ASC 或效果容器，避免幅度计算依赖具体调度方式。
例如按等级缩放：

```rust
struct LevelScale(f32);

impl ModifierMagnitudeCalculation for LevelScale {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32 {
        context.level() as f32 * self.0
    }
}
```

需要读取快照属性时，应组合 `source_snapshot()` 与
`attribute_id_manager()`；需要读取实时标签时，使用 `source_tags()` 或
`target_tags()`。缺少对应数据时接口返回 `None`，计算实现应提供明确的安全默认值。

### `ModifierSpec`

已经完成幅度求值、可以交给 Attributes 使用的不可变运行时修饰器。

```rust
#[derive(Debug, Clone, Copy)]
pub struct ModifierSpec {
    id: AttributeId,
    op: ModifierOperation,
    value: f32,
}

impl ModifierSpec {
    pub const fn new(id: AttributeId, op: ModifierOperation, value: f32) -> Self;
    pub const fn get_id(&self) -> AttributeId;
    pub const fn get_operation(&self) -> ModifierOperation;
    pub const fn get_value(&self) -> f32;
    pub fn scaled_by_stack(&self, stack_count: u32) -> Self;
}
```

### `ModifierSourceId`

持续修饰器使用中立的、带世代信息的来源标识，而不是直接保存
`ActiveEffectHandle`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModifierSourceId {
    scope: u64,
    slot: u32,
    generation: u32,
}

impl ModifierSourceId {
    pub const fn new(scope: u64, slot: u32, generation: u32) -> Self;
    pub const fn get_scope(self) -> u64;
    pub const fn get_slot(self) -> u32;
    pub const fn get_generation(self) -> u32;
}
```

`scope` 区分拥有该来源的运行时存储，`slot + generation` 防止旧来源误删复用槽位中的
新修饰器。Gameplay Effects 提供从 `ActiveEffectHandle` 到 `ModifierSourceId` 的转换；
Attributes 只认识后者，因此不再依赖 Effects。

### `AppliedModifier`

已应用到 `Aggregator` 的求值结果，关联中立的 `ModifierSourceId`。

```rust
pub struct AppliedModifier {
    source_id: ModifierSourceId,
    value: f32,
}
```

`get_source_id()` 是新的明确命名；`get_handle()` 暂时保留为兼容入口，但同样返回
`ModifierSourceId`。

## 聚合器（Aggregator）

`Aggregator` 收集持续修饰器并按固定顺序求值。

```rust
pub struct Aggregator {
    additive: Vec<AppliedModifier>,
    percent_additive: Vec<AppliedModifier>,
    multiplicative: Vec<AppliedModifier>,
    override_value: Option<AppliedModifier>,
    executor: fn(&Aggregator, f32) -> f32,
    has_custom_executor: bool,
}
```

Aggregator 不再内嵌在 `Attribute` 中。每个目标的 `AttributeSet` 通过内部的
`AttributeAggregatorSet` 稀疏保存实际存在持续修饰器的属性，并以
`AttributeLocation` 为键进行二分查找。`Attribute` 本身只保存 base/current 数值，
不重复保存 ID。

效果运行时在跨领域调用边界把 `ActiveEffectHandle` 转换为 `ModifierSourceId`，聚合器
不保存任何 Effect 类型：

```text
ActiveGameplayEffect
        │ From<ActiveEffectHandle>
        ▼
ModifierSourceId ──► AttributeAggregatorSet[AttributeLocation]
                           │
                           ▼
                       Aggregator
```

效果移除后，如果某个 Aggregator 已没有 modifier，使用默认 executor 的 entry 会从
稀疏集合中删除；带自定义 executor 的 entry 会继续保留。executor 属于 Aggregator，
而不是 `Attribute` 数值记录。不存在 Aggregator 时，当前值直接回退到基础值。

### 默认求值顺序

```text
1. Override   — 如果存在，直接返回覆盖值
2. Add        — 求和所有加法修饰器
3. PercentAdd — 求和所有百分比修饰器，以 (1.0 + sum) 应用
4. Multiply   — 连乘所有乘法修饰器
```

公式（伪代码）：

```text
如果 override 存在：
    返回 override.value

result = base_value
对每个 add：        result += add.value
percent_sum = 所有 PercentAdd 值的总和
result *= 1.0 + percent_sum
对每个 multiply：   result *= multiply.value
返回 result
```

### 自定义执行器

初始化属性时可以为其 Aggregator 配置自定义求值函数：

```rust
attributes.initialize_attribute(&manager, id, base_value, Some(my_custom_executor));

fn my_custom_executor(aggregator: &Aggregator, base_value: f32) -> f32 {
    default_executor(aggregator, base_value)
}
```

也可以直接通过 `Aggregator::set_executor()` 配置独立 Aggregator。配置自定义 executor
会使目标级稀疏容器保留该 Aggregator，即使它暂时没有 modifier。

### 主要方法

| 方法                                    | 说明                                      |
| --------------------------------------- | ----------------------------------------- |
| `apply_modifier_spec(spec, source_id)`  | 将修饰器添加到对应操作桶                  |
| `remove_modifiers_by_source(source_id)` | 移除特定运行时来源的所有修饰器            |
| `remove_modifier_by_handle(source_id)`  | 兼容命名，内部同样使用 `ModifierSourceId` |
| `reset()`                               | 清除所有修饰器                            |
| `modifier_count() -> usize`             | 所有桶中的修饰器总数                      |
| `evaluate(base_value) -> f32`           | 使用当前 executor 求值                    |
| `set_executor(executor)`                | 设置自定义求值函数                        |

## 即时修饰器与持续修饰器

| 类型     | 应用方式                                  | 修改对象                                      | 移除时机             |
| -------- | ----------------------------------------- | --------------------------------------------- | -------------------- |
| **即时** | `AttributeSet::apply_instant_modifier()`  | `base` 值                                     | 永不（永久修改）     |
| **持续** | `AttributeSet::apply_duration_modifier()` | `AttributeAggregatorSet` 中的 `Aggregator`    | 来源结束或被显式移除 |

即时修饰器永久改变基础值。持续修饰器是临时的，由 `ModifierSourceId` 标识其运行时来源；
Gameplay Effect 过期或被显式移除时，效果系统使用该来源 ID 清理对应聚合值。
