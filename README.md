# bevy_tools

`bevy_tools` 是面向 Bevy 0.19 的 ECS-first Gameplay Ability System（GAS）库，提供
Gameplay Tags、Attributes、Modifiers、Gameplay Effects、Gameplay Abilities、Ability Tasks、
Targeting，以及确定性的 fixed-tick Gameplay 执行队列。

项目使用 Rust edition 2024，运行时依赖只有：

- `bevy = 0.19.0`
- `rand = 0.10.2`

## 快速开始

```rust
use bevy::prelude::*;
use bevy_tools::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GameplayAbilitySystemPlugin)
        .add_systems(Startup, spawn_gameplay_actor)
        .run();
}

fn spawn_gameplay_actor(mut commands: Commands) {
    commands.spawn(GameplayAbilitySystemBundle::default());
}
```

`GameplayAbilitySystemBundle` 显式组合完整 Gameplay Actor 所需的
`AbilitySystemComponent`、`GameplayTagContainer`、`AttributeSet` 和
`ActiveGameplayEffects`。只需要标签或属性的实体可以单独挂载对应 Component。

标签注册的可运行示例见
[`examples/tag_registration.rs`](./examples/tag_registration.rs)：

```bash
cargo run --example tag_registration
```

## 架构入口

- [知识库导航](./docs/README.md)：按使用、运行时、领域和维护场景组织的文档入口
- [项目与架构总览](./docs/01-overview.md)：领域边界、数据所有权和 fixed-tick 数据流
- [使用模式](./docs/12-usage-patterns.md)：伤害、DoT、Buff、连招和事件驱动示例
- [源码布局与维护边界](./docs/17-source-layout-and-maintenance.md)：真实目录结构和修改路由

公共导入建议：常用类型使用 `bevy_tools::prelude::*`，完整 API 从
`bevy_tools::gas::<domain>` 导入；crate root 的显式重导出继续作为兼容入口。

## 项目原则

设计优先级依次为正确性、可读性和性能，并遵循以下约束：

- Gameplay 状态由 Component、Resource、System、Event/Message 表达；
- 持续时间、周期和任务等待均使用 `FixedUpdate` tick；
- 技能激活与效果应用共享确定性的跨类型 FIFO；
- 定义通过 `Arc<GameplayEffect>` / `Arc<GameplayAbility>` 共享；
- Tags 和 Attributes 可独立使用，完整 GAS 组合由 Bundle 明确表达；
- 不依赖容器遍历顺序表达 Gameplay 语义。

## 当前边界

- `GameplayAbilitySpec` 保存输入 ID 和按下状态，但尚未内置玩家输入到技能激活的完整适配层；
- 尚未提供 serde/ron 配置序列化；
- Ability cost 必须是仅含 `ModifierOperation::Add` 的 Instant Effect。

## 开发检查

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
```

许可证：MIT。
