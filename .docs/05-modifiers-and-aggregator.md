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

完整源码树统一维护在 [17 — 当前源码树](./17-source-layout-and-maintenance.md#当前源码树)。

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
    fn source_attribute(
        &self,
        id: AttributeId,
    ) -> Result<Option<AttributeSnapshot>, AttributeIdError>;
    fn target_attribute(
        &self,
        id: AttributeId,
    ) -> Result<Option<AttributeSnapshot>, AttributeIdError>;
    fn attribute_id_manager(&self) -> &AttributeIdManager;
    fn source_tags(&self) -> Option<&GameplayTagContainer>;
    fn target_tags(&self) -> Option<&GameplayTagContainer>;
}
```

`EffectContext` 实现了该 trait，但 calculator 不依赖 `EffectContext` 的具体类型，也不能直接
访问 ECS Query、ASC 或 Active Effect 存储。

`source_attribute(id)` 与 `target_attribute(id)` 读取求值当时的实时属性，返回单属性的
base/current 值副本。`EffectContext` 通过 `AttributeSet::evaluate_attribute()` 纳入尚未重算的
持续 Buff，不会清除 dirty 位、修改缓存或触发 post-execute 回调，也不需要复制完整属性集。
calculator 仍不接触可变 ECS 状态；自定义聚合 executor 必须保持纯计算。

返回值明确区分：

- `Ok(Some(snapshot))`：实体持有该已初始化属性，`base()` / `current()` 分别取得基础值和聚合值；
- `Ok(None)`：context 没有实时读取能力，或实体、AttributeSet、目标、已初始化属性缺失；
- `Err(AttributeIdError::MissingLocation { .. })`：ID 未在本 context 的注册表中登记；即使实体不存在，
  仍先报告无效 ID。

两个方法提供默认实现：验证 ID 后返回 `Ok(None)`，已有自定义 context 无需立即增加实时读取
能力。`source_snapshot()` 继续返回调用方显式传入的历史快照；实时读取不会隐式回退到它，
也不会用实时值替换历史快照。伤害公式可明确选择发射时的攻击快照，或命中求值时的实时攻击与防御。

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
Gameplay Effects 在调用 Attributes 前通过 `From<ActiveEffectHandle>` 完成转换。效果来源的
`scope` 是容器安装时分配的 `storage_id`，同一 Entity 替换容器后身份不同，旧来源不会误指向新效果。

来源 ID 内部区分调用方域和运行时域：公共 `ModifierSourceId::new` 创建调用方域 ID，
Effects 的转换使用 crate-private 运行时构造。两个域的 scope/slot/generation 即使全部相同，
ID 也不相等，效果移除或容器替换不会误删调用方的独立修饰器。Modifiers 不依赖 Effects；
来源域是中性的身份区分。Getter 返回域内数值字段，不能仅凭这些字段重建一个运行时 ID。

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

### 幅度求值时机

`GameplayEffect::make_spec()` 在效果应用准备阶段求值一次，将 modifier、持续时间和周期的数值
写入规格。新增实时读取只扩大这一次计算可访问的数据，不建立属性依赖图，也不让持续效果自动
追踪来源属性变化。周期效果仍复用应用时捕获的 modifier；之后攻击力或目标防御变化不会使已有
周期伤害自动重算。需要逐次命中取值时，由游戏每次生成新的效果应用，后续有明确用例再扩展周期策略。

### 数值有效性

`Modifier::make_spec()`、`ModifierSpec::new()` 和 `scaled_by_stack()` 保持为纯求值／构造接口，
不改变返回类型。普通 Effect 在 Prepare 和 Execute 边界检查所有幅度，以及实际保留 spec 的
叠层缩放结果；NaN／正负无穷返回 `GameplayEffectApplicationError::NonFiniteModifier { target, id }`。
检查失败发生在本次效果的属性、标签、效果移除和叠层状态修改之前；入口原有的需求收敛和概率
抽取规则不变。技能成本保留既有的 `InsufficientCost` 拒绝语义。

直接调用 `AttributeSet::apply_instant_modifier()` 或 `apply_duration_modifier()` 时，非法幅度
返回 `AttributeSetError::NonFiniteModifier { id }`，不改属性或持续贡献。低层 `Aggregator`
本身不提供这一校验；正常 Gameplay 应通过上述 AttributeSet／Effect 入口修改属性。

这些检查不承诺任意有限输入之间的加法／乘法都不会溢出，也不限制初始化值或自定义 executor、
post-execute 回调的结果。游戏仍需约束属性范围；本轮不引入全局浮点饱和或更改聚合公式。

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

需要命中时实时攻防时，仍通过中立 context 读取数值：

```rust
struct AttackAgainstDefense {
    attack: AttributeId,
    defense: AttributeId,
}

impl ModifierMagnitudeCalculation for AttackAgainstDefense {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32 {
        let (Ok(Some(attack)), Ok(Some(defense))) = (
            context.source_attribute(self.attack),
            context.target_attribute(self.defense),
        ) else {
            return 0.0;
        };
        -(attack.current() - defense.current()).max(0.0)
    }
}
```

示例对缺失或无效属性选择零伤害；具体游戏应明确自己的拒绝、记录或安全值策略，不能假定这类
情况会自动终止效果。calculator 的 `f32` 返回类型目前不能传播业务错误。

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
- 调用方域与运行时域互相隔离；多个自定义来源系统仍须协调调用方域内的 scope/slot/generation。
- `AppliedModifier` 为兼容性保持公开，但正常玩法代码通常只需要 `ModifierSpec` 与
  `AttributeSet`。
- `Aggregator` 的桶和 executor 标志都是私有实现，不应通过字段布局扩展。
