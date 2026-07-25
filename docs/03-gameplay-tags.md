# 03 — Gameplay 标签

## 概述

Gameplay 标签是一个**层级化、基于位集**的标签系统，提供 O(1) 查询性能。父标签自动传播到子标签（例如 `Effect.Debuff.Stun` 蕴含 `Effect.Debuff` 和 `Effect`）。

**容量：** `GAMEPLAY_TAG_SIZE = 512` 个标签（可在 `settings.rs` 中配置）。

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
    ref_counts: Box<[u16]>,       // 每个标签的引用计数
}
```

**主要方法：**

| 方法                         | 说明                                         |
| ---------------------------- | -------------------------------------------- |
| `add_tag(tag, manager)`      | 添加标签 + 所有父标签；递增引用计数；OR 位集 |
| `remove_tag(tag, manager)`   | 递减引用计数；仅当计数归零时清除位           |
| `add_tags(tags, manager)`    | 批量添加                                     |
| `remove_tags(tags, manager)` | 批量移除                                     |
| `has_tag(tag) -> bool`       | O(1) 单标签检查                              |
| `has_all(tags) -> bool`      | 检查是否**全部**指定标签都存在               |
| `has_all_bits(bits) -> bool` | `has_all` 的位集版本                         |
| `has_any(tags) -> bool`      | 检查是否**任意**指定标签存在                 |
| `has_any_bits(bits) -> bool` | `has_any` 的位集版本                         |

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
| `get_inherited_bits(tag) -> Option<&GameplayTagBits>`     | 获取预计算的位集（自身 + 所有祖先） |
| `check_has_active_descendants(index, ref_counts) -> bool` | DFS 检查是否有活跃的子标签          |

### `GameplayTagRegister`

用于通过点分隔名称注册标签的 `SystemParam`。

```rust
#[derive(SystemParam)]
pub struct GameplayTagRegister<'w> { ... }

impl GameplayTagRegister<'_> {
    pub fn request_or_register_tag(
        &mut self,
        full_tag_name: &str,  // 例如 "Effect.Debuff.Stun"
    ) -> Result<GameplayTag, GameplayTagError>;
}
```

父标签会**递归自动注册**。例如，注册 `"Effect.Debuff.Stun"` 也会自动注册 `"Effect.Debuff"` 和 `"Effect"`。

### `GameplayTagError`

```rust
pub enum GameplayTagError {
    CapacityExceeded { max: usize },
    InvalidTagIndex { index: usize },
}
```

## 位集内部实现

- 块大小：`2^6 = 64` 位每 `u64` 块
- `MAX_TAG_BLOCKS = GAMEPLAY_TAG_SIZE / 64`（向上取整）
- `GameplayTagBits = [u64; MAX_TAG_BLOCKS]`

**辅助函数：**

| 函数                                                                        | 说明                           |
| --------------------------------------------------------------------------- | ------------------------------ |
| `tag_bits_from_tags(tags) -> Option<GameplayTagBits>`                       | 从标签切片构建位集（不含继承） |
| `tag_bits_from_tags_with_manager(tags, manager) -> Option<GameplayTagBits>` | 构建含完整继承的位集           |
| `add_bit_with_tag(bits, tag) -> Option<()>`                                 | 在位集中设置单个位             |

## 引用计数

每个标签位关联一个 `u16` 引用计数。当多个效果授予同一个标签时，计数递增。仅当计数归零时，位才会从位集中清除。这防止了一个效果的移除干扰另一个效果的标签授予。

## 使用示例

```rust
fn register_initial_tags(mut register: GameplayTagRegister) {
    let stun = register.request_or_register_tag("Effect.Debuff.Stun").unwrap();
    // "Effect" 和 "Effect.Debuff" 作为父标签自动注册
}

fn check_tags(
    container: &GameplayTagContainer,
    stun: GameplayTag,
) {
    if container.has_tag(&stun) {
        // 实体处于眩晕状态
    }
}
```
