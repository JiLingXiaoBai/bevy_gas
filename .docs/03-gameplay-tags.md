# 03 — Gameplay 标签

## 职责

Gameplay Tags 提供层级化标签注册、固定大小位集、每实体引用计数容器，以及可复用的标签条件。
它负责回答“实体拥有哪些玩法标签”和“某组标签条件是否通过”，不负责效果生命周期。

默认容量由 `GameplayAbilitySystemSettings::GAMEPLAY_TAG_SIZE` 决定，当前为 512。单标签查询
是 O(1)；集合查询扫描固定数量的 `u64` 块，成本有确定上界。

## 源码布局

```text
src/gas/
├── gameplay_tags.rs          # Domain facade and explicit exports
└── gameplay_tags/
    ├── tag.rs                # GameplayTag and GameplayTagError
    ├── bitset.rs             # Fixed-size bitset and pure bit operations
    ├── registry.rs           # GameplayTagManager and GameplayTagRegister
    ├── container.rs          # Reference-counted component
    └── requirements.rs       # TagRequirements
```

Gameplay Tags 不依赖 Gameplay Effects。`TagRequirements` 的所有权也在本领域；Effects 仅为
兼容旧导入路径而重导出它。

## 公共 API

### 注册与标识

`GameplayTag` 是不可由外部直接构造的 `u16` 索引句柄。正常代码通过
`GameplayTagRegister` 请求名称：

```rust
pub struct GameplayTag(u16);

impl GameplayTag {
    pub const fn get_bit_index_u16(&self) -> u16;
    pub const fn get_bit_index_usize(&self) -> usize;
}

impl GameplayTagRegister<'_> {
    pub fn request_or_register_tag(
        &mut self,
        full_tag_name: &str,
    ) -> Result<GameplayTag, GameplayTagError>;
}
```

点分隔名称会递归注册父级。例如注册 `Effect.Debuff.Stun` 时，也会注册
`Effect.Debuff` 和 `Effect`。

`GameplayTagManager` 是全局注册表：

| API | 语义 |
| --- | --- |
| `get_tag(unique_name)` | 查找已经注册的名称 |
| `get_inherited_bits(tag)` | 返回自身与所有祖先的缓存位集 |
| `register_tag_internal(name, parent_index)` | 低层注册入口；通常优先使用 `GameplayTagRegister` |
| `check_has_active_descendants(index, ref_counts)` | 面向自定义存储的低层后代检查 |

错误类型只有以下三类：

```rust
pub enum GameplayTagError {
    UniqueName(UniqueNameError),
    CapacityExceeded { max: usize },
    InvalidTagIndex { index: usize },
}
```

### 位集辅助 API

```rust
pub type GameplayTagBits = [u64; MAX_TAG_BLOCKS];

pub fn tag_bits_from_tags(
    tags: &[GameplayTag],
) -> Result<GameplayTagBits, GameplayTagError>;

pub fn tag_bits_from_tags_with_manager(
    tags: &[GameplayTag],
    manager: &GameplayTagManager,
) -> Result<GameplayTagBits, GameplayTagError>;

pub fn add_bit_with_tag(
    bits: &mut GameplayTagBits,
    tag: &GameplayTag,
) -> Result<(), GameplayTagError>;
```

`tag_bits_from_tags()` 只设置传入标签的精确位；带 manager 的版本会合并每个标签的祖先位。

### `GameplayTagContainer`

`GameplayTagContainer` 是可独立插入实体的 `Component`：

| API | 语义 |
| --- | --- |
| `add_tag(tag, manager)` | 增加标签及其全部祖先的引用计数 |
| `remove_tag(tag, manager)` | 减少引用计数，仅在归零时清位 |
| `add_tags(tags, manager)` | 先验证整个切片，再批量添加 |
| `remove_tags(tags, manager)` | 先验证整个切片，再批量移除 |
| `has_tag(tag)` | 检查一个标签位 |
| `has_all(tags)` / `has_any(tags)` | 检查精确标签切片 |
| `has_all_bits(bits)` / `has_any_bits(bits)` | 检查预计算位集 |

`has_all(&[])` 为 `true`，`has_any(&[])` 为 `false`。移除从未持有的合法标签是无操作，
不会清除其他标签。

### `TagRequirements`

`TagRequirements` 由“全部必须存在”和“任一存在即阻止”两组标签组成：

```rust
impl TagRequirements {
    pub fn new(
        require_all: Vec<GameplayTag>,
        ignore_any: Vec<GameplayTag>,
    ) -> Result<Self, GameplayTagError>;

    pub fn is_empty(&self) -> bool;
    pub fn passes(&self, tags: Option<&GameplayTagContainer>) -> bool;
    pub fn passes_tag_slice(
        &self,
        tags: &[GameplayTag],
        manager: &GameplayTagManager,
    ) -> Result<bool, GameplayTagError>;
    pub fn passes_tag_bits(&self, bits: &GameplayTagBits) -> bool;
    pub fn get_required_tags(&self) -> &[GameplayTag];
    pub fn get_ignored_tags(&self) -> &[GameplayTag];
}
```

空条件总是通过，包括 `passes(None)`。非空条件遇到 `None` 时失败。

## 关键语义

### 层级与引用计数

向容器添加 `Effect.Debuff.Stun` 时，自身、`Effect.Debuff` 和 `Effect` 都会增加一次引用。
若另一个子标签也引用同一父标签，移除其中一个子标签不会过早清除父标签。这是重叠 Buff、
Debuff 能安全共同授予标签的基础。

每个实体的每个标签最多有 `u16::MAX` 个引用。Debug 构建会用 `debug_assert!` 暴露溢出；
Release 构建使用饱和加法，避免整数回绕。
计数更新按块序、从低位到高位扫描继承位集，使用标准库 `isolate_lowest_one()` 取得当前最低置位。

### 条件中的继承

`TagRequirements::new()` 预计算条件标签的精确位。容器本身已经保存祖先位，因此要求父标签
可以匹配持有子标签的实体。检查普通标签切片时，应使用 `passes_tag_slice()`，由 manager
先展开候选标签的祖先。

### 显式 ECS 组合

`GameplayTagContainer` 不会隐式添加 `ActiveGameplayEffects`：

```rust
commands.spawn(GameplayTagContainer::default());
```

需要完整技能、属性、标签与效果运行时的实体应使用组合根：

```rust
commands.spawn(GameplayAbilitySystemBundle::default());
```

## 示例

```rust
fn register_initial_tags(mut register: GameplayTagRegister) {
    match register.request_or_register_tag("Effect.Debuff.Stun") {
        Ok(stun) => {
            info!(index = stun.get_bit_index_u16(), "registered stun tag");
        }
        Err(error) => {
            error!("failed to register gameplay tag: {error}");
        }
    }
}

fn apply_stun_tag(
    tags: &mut GameplayTagContainer,
    manager: &GameplayTagManager,
    stun: GameplayTag,
) -> Result<(), GameplayTagError> {
    tags.add_tag(&stun, manager)
}
```

在 Bevy system 中可把 `Res<GameplayTagManager>` 自动解引用为普通 manager 引用；领域 API
本身不要求调用者暴露 `Res<T>`。

## 边界与注意事项

- `GameplayTag` 不携带 manager 身份。不要混用来自不同注册表生命周期的句柄；索引恰好存在
  时无法检测名称语义是否一致。
- `TagRequirements::new()` 只能检查索引是否超出编译期容量；需要确认标签属于当前 manager
  时，使用 manager-aware API。
- `has_tag()`、`has_all()` 和 `has_any()` 不展开输入标签；继承信息来自容器已有的位集。
- 批量添加和移除会在修改前验证全部标签，避免只完成切片的一部分。
- Gameplay Tags 不负责自动触发效果 Requirement 收敛；使用完整插件时由固定更新流水线处理。
