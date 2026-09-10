# 05 — 修饰器与聚合器

## 职责

`modifiers` 是 Attributes 与 Gameplay Effects 共同使用的中立领域：

- `Modifier` 描述目标属性、操作和幅度来源；
- `ModifierEvaluationContext` 定义动态幅度可读取的数据；
- `ModifierSpec` 保存一次已经完成求值的修改；
- `ModifierSourceId` 标识持续修改的运行时来源；
- `Aggregator` 把同一属性的持续修改合成为 current 值。

Modifiers 不决定效果何时过期，也不保存 `ActiveEffectHandle`。Effects 负责生命周期，Attributes
负责存储与求值。

## 源码布局

```text
src/gas/
├── modifiers.rs                  # Shared-domain facade and explicit exports
├── modifiers/
│   ├── definition.rs            # Operations, magnitudes, and definitions
│   ├── context.rs               # Neutral evaluation context
│   └── spec.rs                  # Evaluated values and source identity
└── attributes/
    └── aggregation.rs           # Public Aggregator and private sparse set
```

`AttributeAggregatorSet` 是 `AttributeSet` 的私有实现，不属于 Modifiers 公共契约。

## 公共 API

### 操作与定义

```rust
pub enum ModifierOperation {
    Add,
    PercentAdd,
    Multiply,
    Override,
}

pub enum ModifierMagnitude {
    Flat(f32),
    Calculated(Box<dyn ModifierMagnitudeCalculation>),
}

pub trait ModifierMagnitudeCalculation: Send + Sync {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32;
}

impl Modifier {
    pub fn new(
        id: AttributeId,
        operation: ModifierOperation,
        magnitude: ModifierMagnitude,
    ) -> Self;
    pub fn get_operation(&self) -> ModifierOperation;
    pub fn make_spec(&self, context: &dyn ModifierEvaluationContext) -> ModifierSpec;
}
```

### `ModifierEvaluationContext`

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

`EffectContext` 实现了该 trait，但 calculator 不依赖 `EffectContext` 的具体类型，也不能直接
访问 ECS Query、ASC 或 Active Effect 存储。

### 已求值规格

```rust
impl ModifierSpec {
    pub const fn new(
        id: AttributeId,
        operation: ModifierOperation,
        value: f32,
    ) -> Self;
    pub const fn get_id(&self) -> AttributeId;
    pub const fn get_operation(&self) -> ModifierOperation;
    pub const fn get_value(&self) -> f32;
    pub fn scaled_by_stack(&self, stack_count: u32) -> Self;
}
```

`scaled_by_stack()` 返回副本，并把原始 `value` 乘以 `stack_count as f32`。它对所有操作一视
同仁，包括 `Multiply` 和 `Override`；调用方必须确认这种线性语义符合设计。

### 来源 ID 与已应用值

```rust
pub struct ModifierSourceId { /* private fields */ }

impl ModifierSourceId {
    pub const fn new(scope: u64, slot: u32, generation: u32) -> Self;
    pub const fn get_scope(self) -> u64;
    pub const fn get_slot(self) -> u32;
    pub const fn get_generation(self) -> u32;
}

impl AppliedModifier {
    pub const fn new(source_id: ModifierSourceId, value: f32) -> Self;
    pub const fn get_source_id(&self) -> ModifierSourceId;
    pub const fn get_handle(&self) -> ModifierSourceId;
    pub const fn get_value(&self) -> f32;
}
```

`get_handle()` 是兼容命名，返回值已经是 `ModifierSourceId`，不是 `ActiveEffectHandle`。
Gameplay Effects 在调用 Attributes 前通过 `From<ActiveEffectHandle>` 完成转换。

### `Aggregator`

```rust
pub fn default_executor(aggregator: &Aggregator, base_value: f32) -> f32;

impl Aggregator {
    pub fn set_executor(&mut self, executor: Option<fn(&Aggregator, f32) -> f32>);
    pub fn apply_modifier_spec(
        &mut self,
        spec: &ModifierSpec,
        source_id: impl Into<ModifierSourceId>,
    );
    pub fn remove_modifiers_by_source(&mut self, source_id: ModifierSourceId);
    pub fn remove_modifier_by_handle(
        &mut self,
        source_id: impl Into<ModifierSourceId>,
    );
    pub fn reset(&mut self);
    pub fn modifier_count(&self) -> usize;
    pub fn evaluate(&self, base_value: f32) -> f32;
}
```

`remove_modifier_by_handle()` 同样只是兼容命名。新代码应使用
`remove_modifiers_by_source()`。

## 关键语义

### 默认聚合顺序

默认 executor 的逻辑为：

```text
if override exists:
    return override.value

value = base
value += sum(additive)
value *= 1.0 + sum(percent_additive)
value *= each multiplicative modifier in insertion order
return value
```

因此给定 base `100`、Add `10`、PercentAdd `0.5`、Multiply `2`，结果为
`(100 + 10) * 1.5 * 2 = 330`。Override 存在时会忽略 base 和其他三个桶。

Aggregator 只保留一个 override 槽；后应用的 override 会替换当前值，而不是形成可恢复的
override 栈。移除当前 override 来源后，先前被覆盖的 override 不会自动恢复。

### Instant 与 Duration 的区别

`AttributeSet::apply_instant_modifier()` 直接修改 base：

| 操作 | 对 base 的行为 |
| --- | --- |
| `Add` | `base += value` |
| `PercentAdd` | `base *= 1.0 + value` |
| `Multiply` | `base *= value` |
| `Override` | `base = value` |

`AttributeSet::apply_duration_modifier()` 不改 base，而是把值交给 Aggregator，并使用
`ModifierSourceId` 支持之后的精确移除。

### 自定义 executor

```rust
fn clamp_non_negative(aggregator: &Aggregator, base: f32) -> f32 {
    default_executor(aggregator, base).max(0.0)
}

attributes.initialize_attribute(
    manager,
    health,
    100.0,
    Some(clamp_non_negative),
)?;
```

executor 是普通函数指针，不能捕获环境。对 `Aggregator::set_executor(None)` 的调用是无操作，
不会恢复默认 executor；重新初始化 AttributeSet 中的属性并传 `None` 才会清除该槽位旧的
Aggregator 并使用默认逻辑。

executor 同时用于实际属性重算和成本数值预演，必须保持纯计算：相同输入产生相同输出，
不修改外部状态，也不依赖调用次数。实际提交后的通知或其他副作用应使用 post-execute 回调。

### 确定性与分配

每个操作桶保持插入顺序。默认加法与百分比先顺序求和，乘法按插入顺序执行；不依赖
HashMap 遍历。Aggregator 的 Vec 可能分配堆内存，因此应让 profiling 决定是否需要进一步
优化，而不是依赖其私有布局。

## 示例

```rust
struct LevelScaledDamage {
    base_damage: f32,
}

impl ModifierMagnitudeCalculation for LevelScaledDamage {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32 {
        -(self.base_damage + context.level() as f32 * 5.0)
    }
}

let damage = Modifier::new(
    health,
    ModifierOperation::Add,
    ModifierMagnitude::Calculated(Box::new(LevelScaledDamage {
        base_damage: 20.0,
    })),
);
```

读取快照时显式处理缺失或错误：

```rust
fn captured_current(
    context: &dyn ModifierEvaluationContext,
    id: AttributeId,
) -> Option<f32> {
    context
        .source_snapshot()?
        .get_current_value(context.attribute_id_manager(), id)
        .ok()?
}
```

## 边界与注意事项

- `ModifierMagnitudeCalculation::calculate()` 返回 `f32`，不能传播 `Result`。实现必须自行决定
  缺失快照、标签或属性时的安全值。
- 不要在 calculator 中使用 `unwrap()`、`expect()` 或依赖隐藏全局状态。
- `ModifierSourceId` 的 `scope` 语义由来源系统定义；只有同一来源命名空间内的三元组才有
  唯一性保证。
- `AppliedModifier` 为兼容性保持公开，但正常玩法代码通常只需要 `ModifierSpec` 与
  `AttributeSet`。
- `Aggregator` 的桶和 executor 标志都是私有实现，不应通过字段布局扩展。
