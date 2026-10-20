# bevy_gas

`bevy_gas` 是面向 Bevy 0.20 的 ECS 优先 Gameplay Ability System（GAS）库，提供
Gameplay Tags、Attributes、Modifiers、Gameplay Effects、Gameplay Abilities、Ability Tasks、
目标结果契约，以及基于 `FixedUpdate` tick 的统一 Gameplay 执行队列。

项目使用 Rust edition 2024，当前依赖 Bevy 0.20.0，最低要求 Rust 1.97.1。具体依赖版本见 [Cargo.toml](./Cargo.toml)，升级说明见 [插件与生命周期](./.docs/02-plugins-and-lifecycle.md#bevy-020-升级基线)。

本库关闭 Bevy 默认 features，运行依赖只启用 `std`、`multi_threaded` 和 `bevy_log`。
输入绑定与 Bevy 几何采样由模板维护，GAS 随机资源只承担通用范围与效果概率采样。
游戏项目按需为自己的 Bevy 依赖选择窗口、渲染和资源格式。库仍使用标准库。

## 快速开始

在 Bevy 应用中添加 GAS 插件，并创建 Gameplay Actor：

```rust
use bevy::prelude::*;
use bevy_gas::prelude::*;

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

`GameplayAbilitySystemBundle` 组合技能、标签、属性和活跃效果组件；具体属性值和技能需要另行
初始化与授予。只需要标签或属性的实体也可以单独挂载对应 Component。

空间查询、逻辑位置、可选表现插值和命中规则由游戏维护。GAS 接收已确定的 Entity 或
`AbilityTargetData`，负责技能与效果结算。配套模板保留动态 AABB 索敌和独立表现插件，见
[目标结果契约](./.docs/15-gameplay-targeting.md)与
[模板空间索敌](../bevy_gas_template/.docs/05-spatial-targeting.md)。

## 可运行示例

建议先运行完整激光示例，了解一次技能从授予、激活到伤害、结束与冷却到期的过程。
以下命令均在仓库根目录执行：

| 示例 | 内容 | 运行命令 |
| --- | --- | --- |
| [完整激光流程](./examples/ability_effect_flow.rs) | 无窗口演示属性消耗、延迟伤害与独立冷却 | `cargo run --example ability_effect_flow` |
| [标签注册](./examples/tag_registration.rs) | 无窗口注册层级标签，单次运行后退出 | `cargo run --example tag_registration` |

详细流程与日志输出说明见 [示例运行说明](./.docs/12-usage-patterns.md#完整可运行示例)。

输入绑定示例已迁至模板，在模板仓库运行 `cargo run --example ability_input_bindings`。
导入迁移、输入缓冲和独立空间随机资源见
[模板输入与空间随机](../bevy_gas_template/.docs/06-input-and-spatial-random.md)。

## 外部配置

游戏项目负责自己的数据表、结构定义、生成代码、加载与配置校验，并通过公开 API 构造
共享的 GAS 定义。配套游戏起始模板
[bevy_gas_template](../bevy_gas_template/README.md) 包含独立配置工程，并通过 `../bevy_gas` 本地 path 依赖本库。

额外消耗直接使用游戏定义的 i32 资源 ID 与正整数数量，Provider 按 ID 检查和支付，无需资源名称注册。
接入步骤与库和游戏的职责划分见 [外部配置集成](./.docs/19-external-configuration.md)。

## 文档

- [知识库导航](./.docs/README.md)：推荐阅读路径、架构、领域 API 与行为约束。
- [使用指南](./.docs/12-usage-patterns.md)：接入前提，以及伤害、DoT、Buff 和连招等用法。
- [测试与开发检查](./.docs/13-testing-guide.md)：测试组织、验证方法与提交前检查。
- [源码布局与维护边界](./.docs/17-source-layout-and-maintenance.md)：目录职责、修改路由与文档维护约定。

许可证：[MIT](./LICENSE)。
