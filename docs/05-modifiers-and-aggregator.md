# 05 — 修饰器与聚合器

## 修饰器操作

| 操作         | 效果               | 典型用例       |
| ------------ | ------------------ | -------------- |
| `Add`        | 加上固定值         | `+50 HP`       |
| `PercentAdd` | 加上当前值的百分比 | `+20% 攻击力`  |
| `Multiply`   | 乘以系数           | `×1.5 速度`    |
| `Override`   | 完全覆盖该值       | `将 HP 设为 1` |

## 修饰器类型

### `Modifier`

定义层级的修饰器（存储在 `GameplayEffect` 中）。

```rust
pub struct Modifier {
    id: AttributeId,
    op: ModifierOperation,
    magnitude: ModifierMagnitude,
}

impl Modifier {
    pub fn new(id: AttributeId, op: ModifierOperation, magnitude: ModifierMagnitude) -> Self;
    pub fn get_operation(&self) -> ModifierOperation;
    pub fn make_spec(&self, context: &EffectContext) -> ModifierSpec;
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
    fn calculate(&self, context: &EffectContext) -> f32;
}
```

`Calculated` 幅度接收 `EffectContext`，可访问来源/目标属性、标签和 ASC 数据——实现数据驱动的缩放（例如"造成来源攻击力 150% 的伤害"）。

### `ModifierSpec`

已解析的不可变运行时修饰器（幅度计算后）。

```rust
#[derive(Debug, Clone, Copy)]
pub struct ModifierSpec {
    id: AttributeId,
    op: ModifierOperation,
    value: f32,
}

impl ModifierSpec {
    pub fn get_id(&self) -> AttributeId;
    pub fn get_operation(&self) -> ModifierOperation;
    pub fn get_value(&self) -> f32;
    pub fn scaled_by_stack(&self, stack_count: u32) -> Self;  // value * stack_count
}
```

### `AppliedModifier`

已应用到 `Aggregator` 的修饰器，关联 `ActiveEffectHandle`。

```rust
pub struct AppliedModifier {
    handle: ActiveEffectHandle,
    value: f32,
}
```

## 聚合器 (Aggregator)

`Aggregator` 收集持续修饰器并按固定顺序求值。

```rust
pub struct Aggregator {
    additive: Vec<AppliedModifier>,
    percent_additive: Vec<AppliedModifier>,
    multiplicative: Vec<AppliedModifier>,
    override_value: Option<AppliedModifier>,
    executor: fn(&Aggregator, f32) -> f32,
}
```

### 默认求值顺序

```
1. Override  — 如果存在，直接返回覆盖值
2. Add       — 求和所有加法修饰器
3. PercentAdd — 求和所有百分比修饰器，以 (1.0 + sum) 应用
4. Multiply  — 连乘所有乘法修饰器
```

公式（伪代码）：

```
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

可以替换默认的求值逻辑：

```rust
aggregator.set_executor(Some(my_custom_executor));

fn my_custom_executor(aggregator: &Aggregator, base_value: f32) -> f32 {
    // 自定义逻辑
}
```

### 主要方法

| 方法                                | 说明                     |
| ----------------------------------- | ------------------------ |
| `apply_modifier_spec(spec, handle)` | 将修饰器添加到对应桶中   |
| `remove_modifier_by_handle(handle)` | 移除特定效果的所有修饰器 |
| `reset()`                           | 清除所有修饰器           |
| `modifier_count() -> usize`         | 所有桶中的修饰器总数     |
| `evaluate(base_value) -> f32`       | 运行执行器               |
| `set_executor(executor)`            | 替换求值函数             |

## 即时修饰器 vs. 持续修饰器

| 类型     | 应用方式                                  | 修改对象     | 移除时机             |
| -------- | ----------------------------------------- | ------------ | -------------------- |
| **即时** | `AttributeSet::apply_instant_modifier()`  | `base` 值    | 永不（永久修改）     |
| **持续** | `AttributeSet::apply_duration_modifier()` | `Aggregator` | 效果过期或被显式移除 |

即时修饰器永久改变基础值。持续修饰器是临时的，在其父效果过期时自动移除。
