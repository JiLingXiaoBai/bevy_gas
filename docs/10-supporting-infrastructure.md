# 10 — 支撑基础设施

## 源码与公开路径

支撑模块位于 GAS 领域目录之外：

```text
src/
├── randoms.rs
├── randoms/random.rs
├── unique_names.rs
├── unique_names/unique_name.rs
└── gas/settings.rs
```

`randoms` 与 `unique_names` 是 crate 私有门面，分别显式重导出内部实现，再由 crate root
公开。因此用户路径是 `bevy_tools::Random`、`bevy_tools::UniqueName`、
`bevy_tools::UniqueNameError` 和 `bevy_tools::UniqueNamePool`；这些类型不属于
`bevy_tools::gas::prelude`。GAS 设置则可从 `bevy_tools::gas::settings`、
`bevy_tools::gas` 或 crate root 导入。

各门面均使用显式公开项，不使用 `pub use *`。新增内部 `pub` 项不会自动扩大 crate API。

## 唯一名称

`UniqueNamePool` 使用 Bevy 的 `FixedHasher` 将字符串定位到候选桶，并以 `u32` 索引作为
`UniqueName` 句柄。哈希只用于查找候选：同一哈希桶内仍比较完整字符串，因此哈希碰撞不会
把不同名称合并。

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct UniqueName(u32);

#[derive(Resource)]
pub struct UniqueNamePool {
    // Internal storage is intentionally private.
}
```

| API | 语义 |
| --- | --- |
| `UniqueNamePool::new_name(name)` | 返回已有句柄或驻留新字符串；容量耗尽时返回 `UniqueNameError` |
| `UniqueNamePool::get_display_str(handle)` | 返回驻留字符串；当前存储中不存在该索引时返回空字符串 |
| `UniqueNamePool::clear()` | 清空所有非空名称并保留索引 0 的空字符串 |

重要不变量：

- 空字符串永久使用索引 0；
- 相同字符串重复注册返回相同句柄；
- `UniqueNameError::CapacityExceeded` 表示 `u32` 句柄空间耗尽，不触发 library panic；
- `clear()` 是全局注册表重置。旧句柄在清空后不再有效，后续名称可能复用其数值索引，调用方
  不得跨 `clear()` 保存并继续使用旧句柄；
- `UniqueNamePool` 由 `UniqueNamePlugin` 初始化，完整
  `GameplayAbilitySystemPlugin` 已包含该插件。

## 随机数

`Random` 封装 `rand::rngs::StdRng`，作为可设置种子的 Bevy `Resource`：

```rust
#[derive(Resource, Debug)]
pub struct Random {
    // StdRng is kept private so call order stays explicit.
}
```

| API | 语义 |
| --- | --- |
| `Random::DEFAULT_SEED` | 默认种子 `123456` |
| `Random::from_seed(seed)` | 使用指定种子构造独立随机源 |
| `Random::set_seed(seed)` | 重置当前随机序列 |
| `random_range(range)` | 从给定范围采样 |
| `random_bool(probability)` | 按概率采样布尔值；调用方必须传入有限的 `0.0..=1.0` |
| `from_rng<T>()` | 构造实现 Bevy `FromRng` 且支持标准分布的值 |
| `sample_interior(shape)` | 在实现 `ShapeSample` 的形状内部采样 |
| `sample_boundary(shape)` | 在形状边界采样 |

在相同依赖版本、相同种子和相同调用顺序下可复现随机序列。不要把不同系统对同一全局
`Random` 的未排序访问当作确定性契约；若调用顺序具有 Gameplay 语义，应通过 SystemSet
排序，或为独立流程使用独立种子的随机状态。

Gameplay Effect 在概率应用前先验证概率范围，再通过 `EffectSystemParams` 中的
`ResMut<Random>` 掷骰。`Random` 由 `RandomPlugin` 初始化，完整
`GameplayAbilitySystemPlugin` 已包含该插件。

## 设置

`src/gas/settings.rs` 定义编译期常量：

```rust
pub struct GameplayAbilitySystemSettings;

impl GameplayAbilitySystemSettings {
    pub const ATTRIBUTE_SET_SIZE: usize = 256;
    pub const HOT_ATTRIBUTE_SET_SIZE: usize = 32;
    pub const COLD_ATTRIBUTE_SET_SIZE: usize =
        Self::ATTRIBUTE_SET_SIZE - Self::HOT_ATTRIBUTE_SET_SIZE;
    pub const GAMEPLAY_TAG_SIZE: usize = 512;
    pub const ABILITY_CHAIN_MAX_DEPTH: u8 = 8;
}
```

`ATTRIBUTE_SET_SIZE` 必须等于 hot 与 cold 容量之和。调整容量会改变固定数组、boxed cold
存储和 Tag 位集的编译期布局，需要重新编译并运行完整测试；这不是运行时配置，也不应在不同
客户端之间出现不一致。

`GameplayExecutionQueue` 与 `TargetingRequestQueue` 都不设置每 tick 消费上限：前者在
`GameplayResolve` 按 Ability/Effect 跨类型 FIFO 完整 drain，后者在 `Targeting` 阶段完整
处理。确定性来自固定阶段、显式生产者排序和稳定的数据顺序，而不是隐藏的全局批次上限。
