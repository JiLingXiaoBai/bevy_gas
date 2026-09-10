# bevy_gas

`bevy_gas` 是面向 Bevy 0.19 的 ECS 优先 Gameplay Ability System（GAS）库，提供
Gameplay Tags、Attributes、Modifiers、Gameplay Effects、Gameplay Abilities、Ability Tasks、
Targeting、技能输入绑定，以及基于 `FixedUpdate` tick 的统一 Gameplay 执行队列。

项目使用 Rust edition 2024，具体依赖版本见 [Cargo.toml](./Cargo.toml)。

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

## 可运行示例

建议先运行完整火球示例，了解一次技能从授予、激活到伤害、结束与冷却到期的过程。
以下命令均在仓库根目录执行：

| 示例 | 内容 | 运行命令 |
| --- | --- | --- |
| [完整火球流程](./examples/ability_effect_flow.rs) | 无窗口演示属性消耗、延迟伤害与独立冷却 | `cargo run --example ability_effect_flow` |
| [标签注册](./examples/tag_registration.rs) | 注册层级标签并读取标签信息 | `cargo run --example tag_registration` |
| [技能输入绑定](./examples/ability_input_bindings.rs) | 无窗口演示技能栏重绑与固定 tick 输入缓冲 | `cargo run --example ability_input_bindings` |

详细流程与日志输出说明见 [示例运行说明](./.docs/12-usage-patterns.md#完整可运行示例)。

## 配置工具

`config/` 维护 Excel 数据表和结构定义，通过固定版本的 `tools/luban/` 工具链导出
Rust 配置代码到 `config/generated/`、二进制到 `config/bin/`。当前使用官方 MiniTemplate
示例验证流程，Rust 配置读取与 GAS 接入仍待实现。

工具支持 Windows x64，需要 PowerShell 7.2+、PATH 中的 .NET Runtime 8+ 和 7-Zip。
在仓库根目录执行：

```powershell
pwsh -NoProfile -File tools/luban/setup.ps1
pwsh -NoProfile -File config/export.ps1
```

首次准备后，日常导表只需执行 `config/export.ps1`。工具链同时固定 Luban.Agent 与
Luban.Mcp 5.0.0；Codex 可通过项目 MCP 配置查询表结构、校验和生成配置。
配置路径、MCP 启用方式与升级约定见 [Luban 配置工程与工具链](./.docs/19-luban-toolchain.md)。

## 文档

- [知识库导航](./.docs/README.md)：推荐阅读路径、架构、领域 API 与行为约束。
- [使用指南](./.docs/12-usage-patterns.md)：接入前提，以及伤害、DoT、Buff 和连招等用法。
- [测试与开发检查](./.docs/13-testing-guide.md)：测试组织、验证方法与提交前检查。
- [源码布局与维护边界](./.docs/17-source-layout-and-maintenance.md)：目录职责、修改路由与文档维护约定。

许可证：[MIT](./LICENSE)。
