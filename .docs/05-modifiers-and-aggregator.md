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
    AddBase = 0,
    MultiplyAdditive = 1,
    DivideAdditive = 2,
    Override = 3,
    MultiplyCompound = 4,
    AddFinal = 5,
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

六种运算的名称、编号和幅度输入约定与
[UE 5.7 官方 `GameplayModOp` 定义](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/GameplayModOp?application_version=5.7)
对齐：

| 运算 | 编号 | 持续聚合语义 |
| --- | --- | --- |
| `AddBase` | `0` | 在百分比和倍率之前相加 |
| `MultiplyAdditive` | `1` | 倍率相对 1 的增量相加，乘以 `1 + sum(value - 1)` |
| `DivideAdditive` | `2` | 除数相对 1 的增量相加，除以 `1 + sum(value - 1)` |
| `Override` | `3` | 使用当前存储顺序中的第一个覆盖值，忽略其他运算 |
| `MultiplyCompound` | `4` | 独立贡献相乘 |
| `AddFinal` | `5` | 所有乘除完成后相加 |

`MultiplyAdditive(1.5)` 表示增加 50%，`MultiplyAdditive(0.75)` 表示降低 25%；
两个值为 1.5 的持续乘法修饰器合成为 2 倍。
`DivideAdditive(2.0)` 表示除以 2，两个值为 2 的持续除法修饰器合成为除以 3。
`MultiplyCompound(1.5)` 则直接乘以 1.5，两个这样的修饰器合成为 2.25 倍。

这是破坏性迁移：旧 `Add` 改名为 `AddBase`，旧 `Multiply` 改名为 `MultiplyCompound`，
旧 `PercentAdd` 改名为 `MultiplyAdditive`，且其幅度必须加 1。动态 calculator 的输出也须
迁移，例如原先返回 `0.5` 的增益现在返回 `1.5`。模板配置同步采用上表编号；旧配置的
`Multiply=2` 应迁移到 `MultiplyCompound=4`，`DivideAdditive=4` 应迁移到 `DivideAdditive=2`。

行为核对依据是公开的 [GameplayAbilities 源码快照（2025-09-29）](https://github.com/oblogin/GameplayAbilities/tree/371e12fc35be974c4cea4b2205c3fd10f99b337e/Source/GameplayAbilities)，
包括 [GameplayEffectAggregator.cpp](https://github.com/oblogin/GameplayAbilities/blob/371e12fc35be974c4cea4b2205c3fd10f99b337e/Source/GameplayAbilities/Private/GameplayEffectAggregator.cpp)
的求值、移除和更新，以及 [GameplayEffectTypes.cpp](https://github.com/oblogin/GameplayAbilities/blob/371e12fc35be974c4cea4b2205c3fd10f99b337e/Source/GameplayAbilities/Private/GameplayEffectTypes.cpp)
的堆叠幅度计算。该快照没有声明精确 UE 版本，因此这里对齐六种运算的数值行为，不宣称整个 GAS 与
UE 5.7 完全一致；求值通道、标签筛选和自定义求值元数据仍不在本次范围内。

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

`scaled_by_stack()` 返回副本，按操作的 bias 缩放单层幅度 `m`，层数为 `n`：

| 操作 | 叠层后的幅度 |
| --- | --- |
| `AddBase`、`MultiplyCompound`、`AddFinal` | `m * n` |
| `MultiplyAdditive`、`DivideAdditive` | `(m - 1) * n + 1` |
| `Override` | `m`，不随层数变化 |

例如 `MultiplyAdditive(1.5)` 两层为 `2.0`；同一 GE 的 `MultiplyCompound(1.5)` 两层为
`3.0`，不是 `2.25`。两个独立的 `MultiplyCompound(1.5)` 贡献才聚合为 `2.25`。`Linear`
描述的是同一效果的幅度缩放策略，不表示逐层执行一次操作。直接调用此方法传入零层时，
前两组分别得到 `0` 和 `1`，`Override` 仍保持原值；正常效果生命周期不会保留零层效果。

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
    pub fn modifiers(
        &self,
        operation: ModifierOperation,
    ) -> impl DoubleEndedIterator<Item = &AppliedModifier> + ExactSizeIterator + '_;
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

`modifiers(operation)` 借用对应运算的已保留修饰器，按当前存储顺序返回只读引用；空运算组返回空
迭代器，并支持反向遍历和精确剩余长度。调用不会复制修饰器或分配临时列表，也不提供修改
聚合器内部存储的能力。每项可通过
`get_value()` 和 `get_source_id()` 读取求值幅度和中立来源 ID。
`Override` 也保留全部活动候选，迭代器不筛掉暂时未被默认 executor 选中的覆盖值。
新贡献追加在末尾；按来源移除时使用与 UE 一致的 swap removal，尾项可以移动到被删除的位置。
效果刷新贡献时先移除旧项再追加新项，因此当前顺序不保证等于最初的应用顺序。

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
post-execute 回调的结果。游戏仍需约束属性范围；新增运算不引入全局浮点饱和。

### 默认聚合顺序

默认 executor 的逻辑为：

```text
if an override exists:
    return first_override_in_current_storage_order.value

add_base = sum(AddBase.value, initial = 0)
multiply_additive = biased_sum(MultiplyAdditive.value - 1, initial = 1)
divide_additive = biased_sum(DivideAdditive.value - 1, initial = 1)
multiply_compound = product(MultiplyCompound.value, initial = 1)
add_final = sum(AddFinal.value, initial = 0)
if abs(divide_additive) <= 1e-8:
    divide_additive = 1
return ((base + add_base) * multiply_additive / divide_additive)
       * multiply_compound + add_final
```

因此给定 base `100`、AddBase `10`、MultiplyAdditive `1.5`、DivideAdditive `2`、MultiplyCompound `2`、
AddFinal `5`，结果为 `((100 + 10) * 1.5 / 2) * 2 + 5 = 170`。
各操作先独立合并，再应用公式；加法空组为 0，倍率、除数和乘积空组为 1。
Override 存在时会忽略 base 和其他五种运算。

**近零除数约定：** 默认 executor 在聚合除数满足 `abs(divisor) <= 1e-8` 时使用中性除数 1；
即时 `DivideAdditive` 对输入幅度使用相同判断，仍进行正常重算和执行后回调。这包括两个持续
`DivideAdditive(0.5)` 合成零分母的情况。超过容差的正负除数正常参与计算，可能产生很大的值或
浮点溢出；持续倍率合并还受 `f32` 舍入影响。自定义 executor 负责自身的除数与数值边界；
下方示例沿用此中性规则。

直接调用 `AttributeSet::apply_instant_modifier()` 时，对传入 spec 的原始幅度判断容差。
通过 GameplayEffect 执行时，单层也先执行 bias 缩放，不绕过 `(m - 1) * 1 + 1`；例如
`DivideAdditive(1.000001e-8)` 在此步骤舍入为 0，因此该即时 GE 保持 base 不变，而直接
应用相同 spec 会执行除法。这是单层缩放的 `f32` 舍入差异，不是两条路径使用不同的容差。

Aggregator 保留所有活动 override 候选，默认选择当前存储顺序中的第一项。例如依次添加
`A=10`、`B=20`、`C=30` 时选 A；移除 A 后，swap removal 将 C 放到首位，因此选 C，随后
移除 C 才选 B。这不是按最大值或最新应用选择；游戏可通过只读候选实现自己的优先级。

### Instant 与 Duration 的区别

`AttributeSet::apply_instant_modifier()` 直接修改 base：

| 操作 | 对 base 的行为 |
| --- | --- |
| `AddBase` | `base += value` |
| `MultiplyAdditive` | `base *= value` |
| `MultiplyCompound` | `base *= value` |
| `Override` | `base = value` |
| `DivideAdditive` | `abs(value) > 1e-8` 时 `base /= value`；近零值为中性除数 1 |
| `AddFinal` | `base += value` |

`AttributeSet::apply_duration_modifier()` 不改 base，而是把值交给 Aggregator，并使用
`ModifierSourceId` 支持之后的精确移除。

即时修饰器按效果定义顺序逐项执行，不先按运算类型分组：即时 `AddFinal` 与 `AddBase` 都直接加到
base，不会等到整批修饰器末尾；连续两个即时 `DivideAdditive(2)` 会除以 4，而两个持续
`DivideAdditive(2)` 聚合后除以 3。非周期持续效果才使用上述默认聚合顺序。

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

游戏可以通过 `modifiers()` 定制已有运算的筛选和组合规则。例如，下列移动速度策略保留所有
百分比增益，只让最强百分比减速参与，并继续处理其他五种运算：

```rust
use bevy_gas::{Aggregator, ModifierOperation};

fn strongest_slow(aggregator: &Aggregator, base: f32) -> f32 {
    if let Some(modifier) = aggregator.modifiers(ModifierOperation::Override).next() {
        return modifier.get_value();
    }

    let mut value = base;
    for modifier in aggregator.modifiers(ModifierOperation::AddBase) {
        value += modifier.get_value();
    }

    let mut bonuses = 0.0;
    let mut slow = 0.0_f32;
    for modifier in aggregator.modifiers(ModifierOperation::MultiplyAdditive) {
        let magnitude = modifier.get_value() - 1.0;
        if magnitude >= 0.0 {
            bonuses += magnitude;
        } else {
            slow = slow.min(magnitude);
        }
    }
    value *= 1.0 + bonuses + slow;

    let mut divisor = 1.0;
    for modifier in aggregator.modifiers(ModifierOperation::DivideAdditive) {
        divisor += modifier.get_value() - 1.0;
    }
    if divisor.abs() > 1.0e-8 {
        value /= divisor;
    }
    for modifier in aggregator.modifiers(ModifierOperation::MultiplyCompound) {
        value *= modifier.get_value();
    }
    for modifier in aggregator.modifiers(ModifierOperation::AddFinal) {
        value += modifier.get_value();
    }
    value.max(0.0)
}
```

通过 `initialize_attribute(..., Some(strongest_slow))` 安装游戏函数。未选中的减速仍保存在
聚合器中，最强减速移除后，下一次求值会自然选中剩余减速。不要为实现筛选而删除暂时未参与
计算的来源。模板可以把配置中的策略名映射为游戏函数；增加此类策略无需修改 GAS。

只读访问不增加运算类别、通道或优先级数据；自定义规则只能使用保留的幅度和来源信息。
需要装备／技能分组、已移除来源的历史或原始堆叠层数时，应先补充相应数据契约。运行中切换策略也
不能通过重新初始化模拟，否则会清空该属性的持续修饰器。

### 确定性与分配

每个操作桶按当前存储顺序求值，追加、按来源 swap removal 和重新追加都具有明确规则，
不依赖 HashMap 遍历。相同的应用／移除序列得到相同顺序，但删除会改变剩余项的顺序，
也可能改变浮点累加结果。自定义 executor 也必须使用明确顺序和稳定的并列选择
规则。只读迭代不分配；Aggregator 保存新增贡献时的 Vec 仍可能分配堆内存，应让 profiling
决定是否需要进一步优化，而不是依赖其私有布局。

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
    ModifierOperation::AddBase,
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
- `AppliedModifier` 提供只读幅度与来源访问；正常属性修改仍使用 `ModifierSpec` 与
  `AttributeSet`，自定义聚合通过 `Aggregator::modifiers()` 读取贡献。
- `Aggregator` 的桶和 executor 标志都是私有实现，不应通过字段布局扩展。
