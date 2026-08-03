# 10 — 支撑基础设施

## 唯一名称 (`unique_names/`)

使用 `FixedHasher` 的字符串驻留池。将字符串转换为 `u32` 索引，实现高效的存储和比较。

### `UniqueName`

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct UniqueName(u32);
```

### `UniqueNamePool`

```rust
#[derive(Resource)]
pub struct UniqueNamePool {
    entry_pool: Vec<String>,
    lookup_hash: HashMap<u64, HashBucket>,
}

impl UniqueNamePool {
    pub fn new_name(&mut self, name: &str) -> Result<UniqueName, UniqueNameError>;
    pub fn get_display_str(&self, name: &UniqueName) -> &str;
    pub fn clear(&mut self);
}
```

哈希值仅用于定位候选桶；桶内始终比较完整字符串，因此不同名称即使发生哈希碰撞，也会获得不同句柄。句柄空间耗尽时返回 `UniqueNameError::CapacityExceeded`，库代码不会为此触发 panic。调用方应显式处理注册失败。

- 空字符串预留在索引 0
- Debug 构建中，哈希冲突会触发 panic
- 使用 `FixedHasher` 确保确定性哈希

## 随机数 (`randoms/`)

确定性、带种子的 RNG，封装为 Bevy `Resource`。

### `Random`

```rust
#[derive(Resource, Debug)]
pub struct Random {
    rng: StdRng,
}

impl Random {
    pub const DEFAULT_SEED: u64 = 123456;

    pub fn from_seed(seed: u64) -> Self;
    pub fn set_seed(&mut self, seed: u64);
    pub fn random_range<T, R>(&mut self, range: R) -> T;
    pub fn random_bool(&mut self, probability: f32) -> bool;
    pub fn from_rng<T: FromRng>(&mut self) -> T;
    pub fn sample_interior<S: ShapeSample>(&mut self, shape: &S) -> S::Output;
    pub fn sample_boundary<S: ShapeSample>(&mut self, shape: &S) -> S::Output;
}
```

- 默认种子 (`123456`) 确保可复现的游戏过程
- 更改种子可用于不同对局或程序化生成
- 内部用于效果概率掷骰

## 设置 (`gas/settings.rs`)

全局编译期常量：

```rust
pub struct GameplayAbilitySystemSettings;

impl GameplayAbilitySystemSettings {
    pub const ATTRIBUTE_SET_SIZE: usize = 256;
    pub const HOT_ATTRIBUTE_SET_SIZE: usize = 32;
    pub const COLD_ATTRIBUTE_SET_SIZE: usize = 224;
    pub const GAMEPLAY_TAG_SIZE: usize = 512;
    pub const ABILITY_ACTIVATION_QUEUE_MAX_PER_TICK: usize = 64;
    pub const GAMEPLAY_EFFECT_APPLICATION_QUEUE_MAX_PER_TICK: usize = 64;
    pub const ABILITY_CHAIN_MAX_DEPTH: u8 = 8;
}
```

要调整这些值，修改 `src/gas/settings.rs` 中的常量并重新编译。
