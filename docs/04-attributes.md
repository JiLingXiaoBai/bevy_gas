# 04 — 属性系统

## 概述

属性系统管理数值（HP、MP、力量等），支持**修饰器聚合**和**延迟重算**。每个实体可拥有一个 `AttributeSet` 组件，最多容纳 256 个属性。属性 ID 保持普通连续编号，物理存储由全局 `AttributeIdManager` 分配到 32 个热点槽位或 224 个冷属性槽位。

## 核心类型

### `AttributeId`

标识特定属性类型的 `u16` 句柄。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttributeId(u16);

impl AttributeId {
    pub fn to_index(self) -> usize;
}
```

### `AttributeIdManager`

全局 `Resource`，统一管理 `UniqueName` → `AttributeId`、普通 ID → 冷热物理槽位，以及两个区域的注册计数。

```rust
#[derive(Resource)]
pub struct AttributeIdManager {
    name_to_index: HashMap<UniqueName, u16>,
    next_id_index: u16,
    locations: [Option<AttributeLocation>; 256],
    hot_count: usize,
    cold_count: usize,
}

impl AttributeIdManager {
    pub fn get_attribute_id(&self, unique_name: UniqueName) -> Option<AttributeId>;
    pub fn register_id_internal(
        &mut self,
        unique_name: UniqueName,
        region: AttributeRegion,
    ) -> Result<AttributeId, AttributeIdError>;
    pub fn location(&self, id: AttributeId) -> Option<AttributeLocation>;
    pub const fn hot_count(&self) -> usize;
    pub const fn cold_count(&self) -> usize;
}
```

### `AttributeIdRegister`

用于按名称注册属性 ID 的 `SystemParam`。

```rust
#[derive(SystemParam)]
pub struct AttributeIdRegister<'w> { ... }

impl AttributeIdRegister<'_> {
    pub fn request_or_register_attribute_id(
        &mut self,
        attribute_id_name: &str,
        region: AttributeRegion,
    ) -> Result<AttributeId, AttributeIdError>;
}
```

同一名称再次注册时必须使用相同区域，否则返回 `AttributeIdError::RegionMismatch`。热点或冷区单独达到容量时返回 `RegionCapacityExceeded`。

### `AttributeRegion` / `AttributeLocation`

注册顺序只决定普通 ID；`AttributeIdManager` 在热点和冷区分别维护连续物理槽位。

```rust
pub enum AttributeRegion {
    Hot,
    Cold,
}

pub struct AttributeLocation {
    region: AttributeRegion,
    slot: usize,
}
```

冷热分类属于全局静态设计信息，应在初始化阶段确定；运行时不会在两个区域之间迁移属性。

### `Attribute`

核心值类型，具有三层值模型：

```rust
pub struct Attribute {
    base: f64,           // 基础值（被即时效果修改）
    evaluated: f64,      // 聚合值（base + 持续修饰器）
    current: f64,        // Clamp 后的最终值
    aggregator: Aggregator,
    dirty: bool,
    clamp: AttributeClamp,
}
```

**值流转：**

```
base ──► [Aggregator.evaluate()] ──► evaluated ──► [clamp] ──► current
```

**主要方法：**

| 方法                                   | 说明                              |
| -------------------------------------- | --------------------------------- |
| `init(base_value, executor, clamp)`    | 初始化基础值、自定义执行器、Clamp |
| `recalculate()`                        | 脏则重算，然后 Clamp              |
| `get_current_value() -> f64`           | 获取当前值（脏时触发重算）        |
| `get_base_value() -> f64`              | 获取原始基础值                    |
| `set_clamp(clamp)`                     | 设置 Clamp 范围，标记脏           |
| `apply_modifier_spec(spec, handle)`    | 向聚合器添加持续修饰器            |
| `remove_modifier_by_handle(handle)`    | 按效果句柄移除持续修饰器          |
| `modify_base_value(spec)`              | 直接对 base 应用即时修饰器        |
| `reset_aggregator()`                   | 清除所有持续修饰器                |
| `modifier_count() -> usize`            | 已应用修饰器总数                  |
| `make_snapshot() -> AttributeSnapshot` | 捕获当前 base + current 值        |

### `AttributeClamp`

```rust
pub enum AttributeClamp {
    None,
    Range { min: Option<f64>, max: Option<f64> },
}
```

纯静态 Clamp 值，不支持动态属性间引用。

### `AttributeSet`

每实体的 `Component`，持有所有属性。

```rust
#[derive(Component)]
pub struct AttributeSet {
    hot_attributes: Box<[Option<Attribute>; 32]>,
    cold_attributes: Box<[Option<Attribute>; 224]>,
    hot_dirty: [u64; 1],
    cold_dirty: [u64; 4],
    post_execute: Option<AttributePostExecute>,
}
```

两个区域都通过 `AttributeIdManager` O(1) 定位。热点修改只设置热点位图，重算时不会扫描或访问冷属性；冷区同理。未初始化槽位仍使用 `None` 表示，避免把默认值 `0.0` 与“不拥有该属性”混淆。

**主要方法：**

| 方法                                                   | 说明                         |
| ------------------------------------------------------ | ---------------------------- |
| `initialize_attribute(manager, id, base, executor, clamp)` | 按管理器映射初始化属性槽位    |
| `set_attribute_clamp(manager, id, clamp)`                  | 更新属性的 Clamp             |
| `set_post_execute(callback)`                           | 设置修改后回调               |
| `recalculate_attribute(manager, id)`                   | 只重算指定属性               |
| `recalculate_dirty()`                                  | 按冷热位图重算脏属性         |
| `get_current_value(manager, id) -> Option<f64>`        | 获取属性的当前值             |
| `apply_instant_modifier(manager, spec)`                | 应用即时修饰器（修改 base）  |
| `apply_duration_modifier(manager, spec, handle)`       | 应用持续修饰器（加入聚合器） |
| `remove_modifiers(handle)`                             | 移除特定效果句柄的所有修饰器 |
| `remove_modifiers_for_attributes(manager, handle, ids)` | 按属性 ID 精确移除修饰器    |
| `make_snapshot(source_entity) -> AttributeSetSnapshot` | 创建所有属性的完整快照       |

### `AttributePostExecute`

```rust
pub type AttributePostExecute =
    fn(&mut AttributeSet, &AttributeIdManager, AttributeId, f64, f64);
//       attr_set,          manager,         attr_id,     old, new
```

即时修饰器改变属性值后调用的回调。适用于"HP 变化时"等副作用。

### `AttributeSnapshot` / `AttributeSetSnapshot`

捕获某一时刻属性状态的不可变快照。

```rust
pub struct AttributeSnapshot {
    base: f64,
    current: f64,
}

#[derive(Component, Clone)]
pub struct AttributeSetSnapshot {
    hot: Box<[Option<AttributeSnapshot>; 32]>,
    cold: Box<[Option<AttributeSnapshot>; 224]>,
    source_entity: Entity,
}
```

快照用于 `EffectPayload` 中，在效果应用时捕获来源实体的属性，从而支持基于"快照时刻"值的计算。读取快照时传入统一管理器：`snapshot.get_current_value(manager, id)`。

## 重算系统

```rust
pub fn recalculate_attribute_sets_system(
    mut query: Query<&mut AttributeSet, Changed<AttributeSet>>,
) {
    for mut attr_set in query.iter_mut() {
        attr_set.recalculate_dirty();
    }
}
```

在 `RecalculateAttributes` 集合中运行。外层使用 Bevy 的 `Changed<AttributeSet>` 过滤实体，组件内部再通过固定大小 dirty 位图只访问实际变化的热点或冷属性。位图按槽位升序处理，不依赖哈希容器遍历顺序。

## 聚合器

修饰器求值管线详见 [05 — 修饰器与聚合器](./05-modifiers-and-aggregator.md)。
