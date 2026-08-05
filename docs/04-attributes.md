# 04 — 属性系统

## 概述

属性系统管理数值（HP、MP、力量等），支持**修饰器聚合**和**延迟重算**。每个实体可拥有一个 `AttributeSet` 组件，最多容纳 256 个属性。属性 ID 保持普通连续编号，物理存储由全局 `AttributeIdManager` 分配到 32 个热点槽位或 224 个冷属性槽位。

## 文件结构

```text
src/gas/
├── attributes.rs                    # 领域门面与显式公开重导出
└── attributes/
    ├── registry.rs                  # ID、冷热区域、Manager 与 Register
    ├── aggregation.rs               # Aggregator 与内部稀疏 AggregatorSet
    ├── snapshot.rs                  # AttributeSnapshot、AttributeSetSnapshot
    ├── attribute_set.rs             # AttributeSet 子模块门面
    └── attribute_set/
        ├── state.rs                 # 数值状态、组件、错误与 dirty 位图
        ├── mutation.rs              # 初始化、即时/持续修改与移除
        └── recalculation.rs         # 延迟重算、快照与 ECS system
```

原本分散的单属性、AggregatorSet 和两类快照文件已经按共同变更原因合并；
`AttributeSet` 的状态、写入和重算流程则分别放在职责明确的子模块中。

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
    pub fn location(&self, id: AttributeId) -> Result<AttributeLocation, AttributeIdError>;
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

同一名称再次注册时必须使用相同区域，否则返回 `AttributeIdError::RegionMismatch`。热点或冷区单独达到容量时返回 `RegionCapacityExceeded`。名称驻留失败通过 `AttributeIdError::UniqueName` 传播，不会触发 panic。

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

`AttributeSet` 的模块内部值类型，具有基础值和当前值两层模型。外部代码不能直接构造或修改 `Attribute`，所有状态变化必须通过 `AttributeSet`：

```rust
struct Attribute {
    base: f32,           // Base value changed by instant modifiers
    current: f32,        // Current aggregated value
}
```

**值流转：**

```
base + AttributeAggregatorSet.get(location) ──► Aggregator.evaluate() ──► current
```

`Attribute` 只保存数值，不再重复保存可由数组槽位确定的 `AttributeId`，也不拥有持续修饰器或自定义 executor。`AttributeSet` 的冷热 dirty 位图是唯一事实来源：位被取出时根据区域和槽位构造 `AttributeLocation`，查询稀疏 Aggregator 并调用 `Attribute::recalculate()`。存在 Aggregator 时使用该 Aggregator 配置的 executor 求值；不存在时 `current` 直接等于 `base`。

**crate 内部主要方法：**

| 方法                                   | 说明                              |
| -------------------------------------- | --------------------------------- |
| `new(base_value)`                      | 初始化基础值和当前值              |
| `recalculate(aggregator)`              | 使用可选 Aggregator 无条件重算    |
| `get_current_value() -> f32`           | 读取已经计算的当前值              |
| `modify_base_value(spec)`              | 直接对 base 应用即时修饰器        |
| `make_snapshot() -> AttributeSnapshot` | 捕获当前 base + current 值        |

### `AttributeAggregatorSet`

`AttributeSet` 内部的目标级运行时存储，按 `AttributeLocation` 管理所有持续修饰器：

```rust
struct AttributeAggregatorSet {
    entries: Vec<AttributeAggregatorEntry>,
}

struct AttributeAggregatorEntry {
    location: AttributeLocation,
    aggregator: Aggregator,
}
```

`entries` 按 `AttributeLocation` 升序排列，并通过二分查找访问；热点区域先于冷区，每个区域内按槽位升序排列。属性收到持续修饰器或配置自定义 executor 时创建对应 Aggregator。最后一个修饰器被移除后，使用默认 executor 的空 entry 会被删除；带自定义 executor 的空 entry 会继续保留。同一个 location 同时负责定位数值槽位、查找 Aggregator，以及按 `ModifierSourceId` 批量移除时设置正确的冷热 dirty bit，避免在 `Attribute` 中重复存储 ID。

该结构是 UE `FActiveGameplayEffectsContainer::AttributeAggregatorMap` 在当前 ECS 布局中的对应物，但暂时仍作为 `AttributeSet` 的内部子结构，从而保持属性修改与 dirty 标记在一次组件可变借用中完成。

### `AttributeSet`

每实体的 `Component`，持有所有属性。

```rust
#[derive(Component)]
pub struct AttributeSet {
    hot_attributes: [Option<Attribute>; 32],
    cold_attributes: Box<[Option<Attribute>; 224]>,
    aggregators: AttributeAggregatorSet,
    hot_dirty: [u64; 1],
    cold_dirty: [u64; 4],
    post_execute: Option<AttributePostExecute>,
}
```

`AttributeSet` 可以作为独立属性组件使用，不再通过
`#[require(ActiveGameplayEffects)]` 隐式插入效果容器。这使纯属性模拟、快照生成和单元测试
不必携带完整 GAS 状态。需要完整技能/属性/标签/效果能力的实体应显式生成：

```rust
commands.spawn(GameplayAbilitySystemBundle::default());
```

该 Bundle 同时包含 `AbilitySystemComponent`、`AttributeSet`、
`GameplayTagContainer` 和 `ActiveGameplayEffects`。

两个数值区域都通过 `AttributeIdManager` O(1) 定位。32 个热点槽位直接内联在组件中，避免高频访问时额外的堆分配和指针间接访问；容量较大的冷区继续使用 `Box`，控制 ECS 表内组件大小。热点修改只设置热点位图，重算时不会扫描或访问冷属性；冷区同理。Aggregator 不做冷热分区，而是使用单个稀疏有序 Vec，因为属性读取频率不等于 Aggregator 访问频率。未初始化槽位仍使用 `None` 表示，避免把默认值 `0.0` 与“不拥有该属性”混淆。

`AttributeSet` 是属性修改的唯一入口。读取指定属性时会检查并清除对应 dirty bit；只有该 bit 原先被设置时才执行内部重算，重复读取干净属性不会再次求值。

读取与写入采用不同语义：读取未初始化的合法属性返回 `Ok(None)`；即时或持续修饰器要求目标属性必须存在，未初始化时返回 `AttributeSetError::UninitializedAttribute`，不会静默跳过。无效 ID 则包装为 `AttributeSetError::AttributeId`。

**主要方法：**

| 方法                                                   | 说明                         |
| ------------------------------------------------------ | ---------------------------- |
| `initialize_attribute(manager, id, base, executor) -> Result` | 初始化属性并可配置聚合器 executor |
| `set_post_execute(callback)`                           | 设置修改后回调               |
| `recalculate_attribute(manager, id) -> Result`         | 只重算指定属性               |
| `recalculate_dirty()`                                  | 按冷热位图重算脏属性         |
| `get_current_value(manager, id) -> Result<Option<f32>, AttributeIdError>` | 获取属性的当前值 |
| `apply_instant_modifier(manager, spec) -> Result<(), AttributeSetError>` | 应用即时修饰器；属性未初始化时失败 |
| `apply_duration_modifier(manager, spec, source_id) -> Result<(), AttributeSetError>` | 应用持续修饰器；来源可转换为 `ModifierSourceId` |
| `remove_modifiers(source_id)`                         | 移除特定运行时来源的所有修饰器 |
| `remove_modifiers_for_attributes(manager, source_id, ids) -> Result` | 按属性 ID 精确移除该来源的修饰器 |
| `make_snapshot(source_entity) -> AttributeSetSnapshot` | 创建所有属性的完整快照       |

### `AttributePostExecute`

```rust
pub type AttributePostExecute =
    fn(&mut AttributeSet, &AttributeIdManager, AttributeId, f32, f32);
//       set,               manager,         id,          old, new
```

即时修饰器改变属性值后调用的回调。适用于"HP 变化时"等副作用。

### `AttributeSnapshot` / `AttributeSetSnapshot`

捕获某一时刻属性状态的不可变快照。

```rust
pub struct AttributeSnapshot {
    base: f32,
    current: f32,
}

#[derive(Component, Clone)]
pub struct AttributeSetSnapshot {
    hot: [Option<AttributeSnapshot>; 32],
    cold: Box<[Option<AttributeSnapshot>; 224]>,
    source_entity: Entity,
}
```

快照用于 `EffectPayload` 中，在效果应用时捕获来源实体的属性，从而支持基于"快照时刻"值的计算。读取快照时传入统一管理器：`snapshot.get_current_value(manager, id)`，返回 `Result<Option<f32>, AttributeIdError>`。其中 `Err` 表示 ID 与 Manager 不匹配，`Ok(None)` 表示该实体未初始化对应属性。

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
