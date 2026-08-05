# 03 — Gameplay 标签

## 概述

Gameplay 标签是一个**层级化、基于位集**的标签系统，提供 O(1) 查询性能。添加子标签时会同时设置其祖先标签（例如 `Effect.Debuff.Stun` 蕴含 `Effect.Debuff` 和 `Effect`）。

**容量：** `GAMEPLAY_TAG_SIZE = 512` 个标签（可在 `settings.rs` 中配置）。

## 文件结构

```text
src/gas/
├── gameplay_tags.rs          # 领域门面与显式公开重导出
└── gameplay_tags/
    ├── tag.rs                # GameplayTag、GameplayTagError
    ├── bitset.rs             # 固定大小位集、常量与纯位操作
    ├── registry.rs           # GameplayTagManager、GameplayTagRegister
    ├── container.rs          # 引用计数 GameplayTagContainer
    └── requirements.rs       # TagRequirements
```

依赖方向保持为“标签标识与位集 → 注册表 → 容器与条件”。低层标签模块不再依赖
Gameplay Effects。

## 核心类型

### `GameplayTag`

封装 `u16` 位索引的轻量包装。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameplayTag(u16);

impl GameplayTag {
    pub fn get_bit_index_u16(&self) -> u16;
    pub fn get_bit_index_usize(&self) -> usize;
}
```

### `GameplayTagContainer`

每实体的 `Component`，存储带**引用计数**的位集。

```rust
#[derive(Component)]
pub struct GameplayTagContainer {
    tag_bits: GameplayTagBits,    // [u64; MAX_TAG_BLOCKS]
    ref_counts: Box<[u16]>,       // Per-tag reference counts
}
```

`GameplayTagContainer` 是可独立使用的组件，不再通过
`#[require(ActiveGameplayEffects)]` 隐式添加效果容器。仅需要标签能力的实体可以单独
插入它；需要完整 GAS 运行时的实体应显式生成
`GameplayAbilitySystemBundle::default()`，由 Bundle 一次组合 ASC、Attributes、Tags
与 Active Effects。这样标签领域不会反向依赖效果领域，实体实际拥有的能力也更加直观。

**主要方法：**

| 方法                         | 说明                                         |
| ---------------------------- | -------------------------------------------- |
| `add_tag(tag, manager) -> Result`      | 添加标签 + 所有父标签；递增引用计数；OR 位集 |
| `remove_tag(tag, manager) -> Result`   | 递减引用计数；仅当计数归零时清除位           |
| `add_tags(tags, manager) -> Result`    | 批量添加                                     |
| `remove_tags(tags, manager) -> Result` | 批量移除                                     |
| `has_tag(tag) -> bool`       | O(1) 单标签检查                              |
| `has_all(tags) -> bool`      | 检查是否**全部**指定标签都存在               |
| `has_all_bits(bits) -> bool` | `has_all` 的位集版本                         |
| `has_any(tags) -> bool`      | 检查是否**任意**指定标签存在                 |
| `has_any_bits(bits) -> bool` | `has_any` 的位集版本                         |

每个实体上的单个标签（包括由子标签继承得到的父标签）最多支持 `u16::MAX`，即 65,535 个并发引用。超过该上限属于内部不变量被破坏：Debug 构建会通过 `debug_assert!` 立即暴露问题；Release 构建会将计数保持在 `u16::MAX`，避免整数回绕或 library panic。因此玩法和效果配置不应产生超过此上限的重叠标签授予。

### `TagRequirements`

通用标签条件由一组“必须全部存在”的标签和一组“任一存在即拒绝”的标签组成。构造时预计算
两组位集，之后可以检查 `GameplayTagContainer`、普通标签切片或已构建的
`GameplayTagBits`。

`TagRequirements` 由 Gameplay Tags 领域拥有，并同时供 Gameplay Effects 和 Targeting
使用。为了保持已有用户代码兼容，Gameplay Effects 门面仍会重导出该类型。

| 方法 | 说明 |
| --- | --- |
| `new(require_all, ignore_any)` | 校验标签并预计算位集 |
| `passes(container)` | 检查可选标签容器；非空条件遇到 `None` 时失败 |
| `passes_tag_slice(tags, manager)` | 展开标签继承后检查普通标签切片 |
| `passes_tag_bits(bits)` | 直接检查已构建位集 |
| `is_empty()` | 两组条件都为空时返回 `true` |

### `GameplayTagManager`

全局 `Resource`，注册标签并追踪继承关系。

```rust
#[derive(Resource)]
pub struct GameplayTagManager {
    tag_name_to_index: HashMap<UniqueName, u16>,
    tag_parent_index: Vec<Option<u16>>,
    tag_children: Vec<Vec<u16>>,
    tag_inherited_bits: Vec<GameplayTagBits>,
    next_tag_index: u16,
}
```

**主要方法：**

| 方法                                                      | 说明                                |
| --------------------------------------------------------- | ----------------------------------- |
| `get_tag(unique_name) -> Option<GameplayTag>`             | 通过 `UniqueName` 查找标签          |
| `get_inherited_bits(tag) -> Result<&GameplayTagBits, GameplayTagError>` | 获取预计算的位集（自身 + 所有祖先） |
| `check_has_active_descendants(index, ref_counts) -> bool` | DFS 检查是否有活跃的子标签          |

### `GameplayTagRegister`

用于通过点分隔名称注册标签的 `SystemParam`。

```rust
#[derive(SystemParam)]
pub struct GameplayTagRegister<'w> { ... }

impl GameplayTagRegister<'_> {
    pub fn request_or_register_tag(
        &mut self,
        full_tag_name: &str,  // For example, "Effect.Debuff.Stun"
    ) -> Result<GameplayTag, GameplayTagError>;
}
```

父标签会**递归自动注册**。例如，注册 `"Effect.Debuff.Stun"` 也会自动注册 `"Effect.Debuff"` 和 `"Effect"`。

内部注册 API 收到无效的父标签索引时返回 `GameplayTagError::InvalidTagIndex`，不会把
子标签静默注册成根标签。

名称驻留失败会通过 `GameplayTagError::UniqueName` 传播，不会触发 panic；不同完整名称即使产生相同哈希值，也仍会注册为不同标签。

### `GameplayTagError`

```rust
pub enum GameplayTagError {
    UniqueName(UniqueNameError),
    CapacityExceeded { max: usize },
    InvalidTagIndex { index: usize },
}
```

## 位集内部实现

- 块大小：`2^6 = 64` 位每 `u64` 块
- `MAX_TAG_BLOCKS = GAMEPLAY_TAG_SIZE / 64`（向上取整）
- `GameplayTagBits = [u64; MAX_TAG_BLOCKS]`

**辅助函数：**

| 函数                                                                                         | 说明                                         |
| -------------------------------------------------------------------------------------------- | -------------------------------------------- |
| `tag_bits_from_tags(tags) -> Result<GameplayTagBits, GameplayTagError>`                       | 从标签切片构建位集（不含继承）               |
| `tag_bits_from_tags_with_manager(tags, manager) -> Result<GameplayTagBits, GameplayTagError>` | 构建含完整继承的位集                         |
| `add_bit_with_tag(bits, tag) -> Result<(), GameplayTagError>`                                 | 设置单个位；索引无效时返回 `InvalidTagIndex` |

当标签索引超出容量，或标签未在传入的 `GameplayTagManager` 中注册时，上述函数返回
`GameplayTagError::InvalidTagIndex`。

`GameplayTagContainer` 的添加和移除 API 同样传播该错误，不再静默忽略来自错误
Manager 的标签。

容器、条件和位集辅助 API 接收普通的 `&GameplayTagManager`，不把 `Res<T>` 暴露为领域
接口。Bevy system 中的 `Res<GameplayTagManager>` 可通过自动解引用直接传入。

## 引用计数

每个标签位关联一个 `u16` 引用计数。当多个效果授予同一个标签时，计数递增。仅当计数归零时，位才会从位集中清除。这防止了一个效果的移除干扰另一个效果的标签授予。

## 使用示例

```rust
fn register_initial_tags(mut register: GameplayTagRegister) {
    let _stun = match register.request_or_register_tag("Effect.Debuff.Stun") {
        Ok(tag) => tag,
        Err(error) => {
            error!("failed to register gameplay tag: {error}");
            return;
        }
    };
    // Parent tags "Effect" and "Effect.Debuff" are registered automatically.
}

fn check_tags(
    container: &GameplayTagContainer,
    stun: GameplayTag,
) {
    if container.has_tag(&stun) {
        // The entity is stunned.
    }
}
```

仅使用标签时可以独立生成组件：

```rust
commands.spawn(GameplayTagContainer::default());
```

参与完整技能、属性与效果生命周期的实体应使用显式 Bundle：

```rust
commands.spawn(GameplayAbilitySystemBundle::default());
```
