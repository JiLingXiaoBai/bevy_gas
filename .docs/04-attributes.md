# 04 — 属性系统

## 职责

Attributes 负责全局属性 ID 注册、每实体基础值与当前值、持续修饰器聚合、脏值重算和不可变
快照。它不负责效果的持续时间或堆叠策略；Gameplay Effects 只通过公开的
`AttributeSet`/`ModifierSpec` API 修改属性。

当前总容量为 256，其中 32 个 Hot 槽位内联存储，224 个 Cold 槽位放在 boxed 区域。
容量由 `GameplayAbilitySystemSettings` 的关联常量定义。

## 源码布局

```text
src/gas/
├── attributes.rs                    # Domain facade and explicit exports
└── attributes/
    ├── registry.rs                  # IDs, regions, manager, and register param
    ├── aggregation.rs               # Public Aggregator and private sparse storage
    ├── snapshot.rs                  # Immutable snapshots
    ├── attribute_set.rs             # AttributeSet facade
    └── attribute_set/
        ├── state.rs                 # Component state and errors
        ├── mutation.rs              # Initialization and modifier mutation
        └── recalculation.rs         # Lazy recalculation, snapshots, and system
```

`Attribute` 与 `AttributeAggregatorSet` 都是实现细节，不是外部扩展点。公开契约集中在本篇
列出的 ID、`AttributeSet`、快照和 `Aggregator` API。

## 公共 API

### 属性注册

```rust
pub struct AttributeId(u16);

impl AttributeId {
    pub fn to_index(self) -> usize;
}

pub enum AttributeRegion {
    Hot,
    Cold,
}

pub struct AttributeLocation { /* private fields */ }

impl AttributeLocation {
    pub const fn region(self) -> AttributeRegion;
    pub const fn slot(self) -> usize;
}
```

`AttributeIdManager` 管理名称到普通 ID、以及 ID 到 Hot/Cold 物理槽位的全局映射：

| API | 语义 |
| --- | --- |
| `get_attribute_id(unique_name)` | 查找已注册 ID |
| `location(id)` | 返回区域和区域内槽位 |
| `hot_count()` / `cold_count()` | 返回两个区域的注册数 |
| `register_id_internal(name, region)` | 低层注册入口；通常优先使用 `AttributeIdRegister` |

推荐在 system 中按字符串名称注册：

```rust
impl AttributeIdRegister<'_> {
    pub fn request_or_register_attribute_id(
        &mut self,
        name: &str,
        region: AttributeRegion,
    ) -> Result<AttributeId, AttributeIdError>;
}
```

同一名称再次请求时必须使用原区域。

```rust
pub enum AttributeIdError {
    UniqueName(UniqueNameError),
    CapacityExceeded { max: usize },
    RegionCapacityExceeded { region: AttributeRegion, max: usize },
    RegionMismatch {
        existing: AttributeRegion,
        requested: AttributeRegion,
    },
    MissingLocation { id: AttributeId },
}
```

### `AttributeSet`

`AttributeSet::default()` 创建一个尚未初始化任何属性的组件。主要 API 为：

```rust
impl AttributeSet {
    pub fn initialize_attribute(
        &mut self,
        manager: &AttributeIdManager,
        id: AttributeId,
        base_value: f32,
        executor: Option<fn(&Aggregator, f32) -> f32>,
    ) -> Result<(), AttributeIdError>;

    pub fn set_post_execute(&mut self, callback: Option<AttributePostExecute>);
    pub fn recalculate_attribute(
        &mut self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Result<(), AttributeIdError>;
    pub fn recalculate_dirty(&mut self);
    pub fn get_current_value(
        &mut self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Result<Option<f32>, AttributeIdError>;

    pub fn apply_instant_modifier(
        &mut self,
        manager: &AttributeIdManager,
        spec: &ModifierSpec,
    ) -> Result<(), AttributeSetError>;
    pub fn apply_duration_modifier(
        &mut self,
        manager: &AttributeIdManager,
        spec: &ModifierSpec,
        source_id: impl Into<ModifierSourceId>,
    ) -> Result<(), AttributeSetError>;
    pub fn remove_modifiers(&mut self, source_id: impl Into<ModifierSourceId>);
    pub fn remove_modifiers_for_attributes(
        &mut self,
        manager: &AttributeIdManager,
        source_id: impl Into<ModifierSourceId>,
        ids: impl IntoIterator<Item = AttributeId>,
    ) -> Result<(), AttributeIdError>;

    pub fn make_snapshot(&mut self, source: Entity) -> AttributeSetSnapshot;
}
```

修改失败使用 `AttributeSetError`：

```rust
pub enum AttributeSetError {
    AttributeId(AttributeIdError),
    UninitializedAttribute { id: AttributeId },
}
```

读取一个已注册但未初始化的属性返回 `Ok(None)`；即时和持续修改同一情况则返回
`UninitializedAttribute`。`recalculate_attribute()` 只验证 ID 已注册，未初始化槽位是无操作。

### Post-execute 回调

```rust
pub type AttributePostExecute =
    fn(&mut AttributeSet, &AttributeIdManager, AttributeId, f32, f32);
```

回调只在成功执行即时修饰器后触发。参数依次为 set、manager、ID、修改前 current 和修改后
current；修改后值已经重新应用仍然存在的持续聚合器。成本预演不会调用该回调，也不模拟回调
对整个属性集的修改；回调的可观察副作用只发生在真实即时修改时。

### 快照

```rust
impl AttributeSnapshot {
    pub const fn new(base: f32, current: f32) -> Self;
    pub const fn base(&self) -> f32;
    pub const fn current(&self) -> f32;
}

impl AttributeSetSnapshot {
    pub fn get_current_value(
        &self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Result<Option<f32>, AttributeIdError>;
    pub fn get_base_value(
        &self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Result<Option<f32>, AttributeIdError>;
    pub const fn get_source_entity(&self) -> Entity;
}
```

完整 set 快照只能通过 `AttributeSet::make_snapshot()` 获得。它会先重算所有 dirty 属性，
之后保存独立的 base/current 副本；后续修改源 set 不会改变快照。

### 重算系统

```rust
pub fn recalculate_attribute_sets_system(
    query: Query<&mut AttributeSet, Changed<AttributeSet>>,
);
```

完整插件把该系统放在 `GameplayAbilitySystemSet::RecalculateAttributes`，位于效果、技能任务、
Gameplay FIFO、Requirement 收敛和技能清理之后。

## 关键语义

### Base 与 Current

- `base` 是持久基础值；即时修饰器直接改变它。
- `current` 是 `base` 经过当前 Aggregator 求值后的缓存。
- 初始化、持续修饰器变化和移除只设置对应 Hot/Cold dirty 位。
- `get_current_value()`、快照或重算系统在需要时刷新缓存。

这也是 `get_current_value()` 需要 `&mut self` 的原因：一次读取可能消费 dirty 位并更新
current。

### 即时与持续修改

即时操作直接作用于 base：

| `ModifierOperation` | 对 base 的行为 |
| --- | --- |
| `Add` | `base += value` |
| `PercentAdd` | `base *= 1.0 + value` |
| `Multiply` | `base *= value` |
| `Override` | `base = value` |

持续修改不改变 base，而是按 `ModifierSourceId` 保存在内部稀疏 Aggregator 中。移除一个不存在
的来源是安全无操作。`remove_modifiers_for_attributes()` 会先解析全部 ID，再开始修改，避免无效
ID 造成部分移除。

重新调用 `initialize_attribute()` 会清空该槽位已有的 Aggregator 和持续修饰器，并用新 base
重新初始化；`executor: None` 恢复默认求值器，`Some(function)` 安装函数指针执行器。

### 成本数值预演

Ability 支付检查通过 Effect 层进入 AttributeSet 的内部预演入口。预演在栈上的 Hot/Cold
临时槽位中保留每个受影响属性的计算状态，重复属性按原条目顺序累计 base；每一步与实际即时
修改共用 base 运算和 current 重算函数，使用同一个默认或自定义聚合 executor。

通常直接借用已有聚合器；成本计划包含效果移除时，才复制内部稀疏聚合器并临时移除对应
`ModifierSourceId`。预演读取真实 base，不依赖可能尚未重算的 current 缓存，不消费真实 dirty
位，也不写入属性或执行 post-execute 回调。是否可支付由 Ability 领域提供的数值条件判断。

### Hot/Cold 存储

普通 `AttributeId` 按全局注册顺序连续增长；Hot 与 Cold 只影响物理槽位。两个区域分别使用
连续槽位，因此 `AttributeId::to_index()` 不能替代 `AttributeIdManager::location()` 访问实体
存储。

### 显式 ECS 组合

`AttributeSet` 不会隐式添加 `ActiveGameplayEffects`，可用于纯属性实体：

```rust
commands.spawn(AttributeSet::default());
```

完整 GAS Actor 使用：

```rust
commands.spawn(GameplayAbilitySystemBundle::default());
```

## 示例

```rust
fn create_combat_attributes(
    manager: &AttributeIdManager,
    health: AttributeId,
    movement_speed: AttributeId,
) -> Result<AttributeSet, AttributeIdError> {
    let mut attributes = AttributeSet::default();
    attributes.initialize_attribute(manager, health, 100.0, None)?;
    attributes.initialize_attribute(manager, movement_speed, 6.0, None)?;
    Ok(attributes)
}

fn read_health(
    attributes: &mut AttributeSet,
    manager: &AttributeIdManager,
    health: AttributeId,
) -> Result<Option<f32>, AttributeIdError> {
    attributes.get_current_value(manager, health)
}
```

## 边界与注意事项

- `AttributeId` 不携带 manager 身份，不要混用不同注册表生命周期产生的 ID。
- 不要依赖 `AttributeSet` 的数组、boxed 区域、dirty 位图或稀疏集合字段；它们是私有实现。
- `AttributeAggregatorSet` 不是公共 API；需要自定义求值时使用公开的 `Aggregator` 函数指针
  接口，详见 [05 — 修饰器与聚合器](./05-modifiers-and-aggregator.md)。
- `set_post_execute()` 不是普通 change observer，只覆盖即时 modifier 成功执行路径。
- 直接读取快照不会访问 World；snapshot 使用调用时传入的 manager 解析 ID。
