# bevy_gas

`bevy_gas` 是面向 Bevy 0.20 的 ECS 优先 Gameplay Ability System（GAS）库，提供
Gameplay Tags、Attributes、Modifiers、Gameplay Effects、Gameplay Abilities、Ability Tasks、
目标结果契约，以及基于 `FixedUpdate` tick 的统一 Gameplay 执行队列。

项目使用 Rust edition 2024，当前依赖 Bevy 0.20.1，最低要求 Rust 1.97.1。具体依赖版本见 [Cargo.toml](./Cargo.toml)，升级说明见 [插件与生命周期](./.docs/02-plugins-and-lifecycle.md#bevy-020-升级基线)。

本库关闭 Bevy 默认 features，运行依赖只启用 `std`、`multi_threaded` 和 `bevy_log`。
输入绑定与 Bevy 几何采样由模板维护，GAS 随机资源只承担通用范围与效果概率采样。
游戏项目按需为自己的 Bevy 依赖选择窗口、渲染和资源格式。库仍使用标准库。

## 快速开始

在 Bevy 应用中添加 GAS 插件，按顺序注册基础标签和属性，再创建 Gameplay Actor：

```rust
use bevy::prelude::*;
use bevy_gas::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GameplayAbilitySystemPlugin)
        .add_systems(
            Startup,
            (
                register_initial_tags,
                register_initial_attributes,
                spawn_gameplay_actor,
            )
                .chain(),
        )
        .run();
}

fn register_initial_tags(mut tags: GameplayTagRegister) {
    if let Err(error) = tags.request_or_register_tag("Effect.Debuff.Stun") {
        error!("failed to register gameplay tag: {error}");
    }
}

fn register_initial_attributes(mut attributes: AttributeIdRegister) {
    if let Err(error) =
        attributes.request_or_register_attribute_id("Health", AttributeRegion::Hot)
    {
        error!("failed to register Health: {error}");
    }
}

fn spawn_gameplay_actor(mut commands: Commands) {
    commands.spawn(GameplayAbilitySystemBundle::default());
}
```

注册顺序必须由调用方显式确定；同一 `Startup` 中使用 `.chain()` 可以保证后续初始化看到已经
注册的 Tag 和 Attribute ID。`GameplayAbilitySystemBundle` 只负责组合技能、标签、属性和活跃效果
Component，不会替实体初始化具体属性值或授予技能。只需要标签或属性的实体也可以单独挂载对应 Component。

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
| [背包道具消耗](./examples/inventory_bomb.rs) | 演示外部资源成本的检查、支付与补偿 | `cargo run --example inventory_bomb` |

详细流程与日志输出说明见 [示例运行说明](./.docs/12-usage-patterns.md#完整可运行示例)。
背包等额外消耗的接入说明见 [14 — 扩展系统](./.docs/14-extending-the-system.md#接入背包等额外消耗)。

输入绑定示例已迁至模板，在模板仓库运行 `cargo run --example ability_input_bindings`。
导入迁移、输入缓冲和独立空间随机资源见
[模板输入与空间随机](../bevy_gas_template/.docs/06-input-and-spatial-random.md)。

## 外部配置

游戏项目负责自己的数据表、结构定义、生成代码、加载与配置校验，并通过公开 API 构造
共享的 GAS 定义。配套游戏起始模板
[bevy_gas_template](../bevy_gas_template/README.md) 包含独立配置工程，并通过 `../bevy_gas` 本地 path 依赖本库。

额外消耗直接使用游戏定义的 i32 资源 ID 与正整数数量，Provider 按 ID 检查和支付，无需资源名称注册。
接入步骤与库和游戏的职责划分见 [外部配置集成](./.docs/19-external-configuration.md)。

## 文档导航

以下专题文档描述当前源码中的 Gameplay Ability System，统一从本页导航。阅读时采用以下事实优先级：

1. rustdoc 和领域门面模块：公共名称、精确签名和错误类型；
2. `.docs/`：行为语义、阶段顺序、设计理由和跨模块约束；
3. `examples/` 与 `tests/`：由编译器验证的完整用法和边界场景。

文档不会把所有私有字段复制成长期契约。需要精确 API 时，从
`bevy_gas::gas::<domain>` 的 rustdoc 开始；常用入口可使用 `bevy_gas::prelude::*`。

### 推荐阅读路径

| 目标 | 建议顺序 |
| --- | --- |
| 第一次接入 GAS | [01 总览](./.docs/01-overview.md) → [02 插件与生命周期](./.docs/02-plugins-and-lifecycle.md) → [09 ASC](./.docs/09-ability-system-component.md) → [12 使用模式](./.docs/12-usage-patterns.md) |
| 理解一次技能如何结算 | [07 技能](./.docs/07-gameplay-abilities.md) → [08 任务](./.docs/08-ability-tasks.md) → [15 目标结果](./.docs/15-gameplay-targeting.md) → [16 统一执行](./.docs/16-gameplay-execution.md) |
| 接入游戏空间索敌与表现 | [模板空间索敌](../bevy_gas_template/.docs/05-spatial-targeting.md) → [15 目标结果](./.docs/15-gameplay-targeting.md) |
| 接入输入与技能栏 | [18 输入绑定迁移](./.docs/18-ability-input-bindings.md) → [09 ASC](./.docs/09-ability-system-component.md) → [16 统一执行](./.docs/16-gameplay-execution.md) |
| 扩展数值与状态系统 | [03 标签](./.docs/03-gameplay-tags.md) → [04 属性](./.docs/04-attributes.md) → [05 修饰器](./.docs/05-modifiers-and-aggregator.md) → [06 效果](./.docs/06-gameplay-effects.md) |
| 接入游戏配置 | [19 外部配置集成](./.docs/19-external-configuration.md) → [14 扩展系统](./.docs/14-extending-the-system.md) |
| 维护或重构源码 | [17 源码布局](./.docs/17-source-layout-and-maintenance.md) → [13 测试](./.docs/13-testing-guide.md) → [14 扩展系统](./.docs/14-extending-the-system.md) |

完整源码树只维护在 [17 — 当前源码树](./.docs/17-source-layout-and-maintenance.md#当前源码树)，
完整测试与示例树只维护在 [13 — 测试与示例组织](./.docs/13-testing-guide.md#测试与示例组织)。
各领域页通过职责说明和链接导航到实际实现。

### 架构与运行时

| 文档 | 权威内容 |
| --- | --- |
| [01 — 项目概述](./.docs/01-overview.md) | 领域职责、ECS 数据所有权、整体数据流和核心不变量 |
| [02 — 插件系统与生命周期](./.docs/02-plugins-and-lifecycle.md) | PluginGroup、独立表现插件、资源安装与固定阶段顺序 |
| [09 — 技能系统组件](./.docs/09-ability-system-component.md) | ASC、显式 Bundle、Ability/Effect SystemParam 边界 |
| [16 — Gameplay 执行模块](./.docs/16-gameplay-execution.md) | 统一请求 FIFO、batch 可见性、同 tick/下一 tick 边界 |

### 数据与规则领域

| 文档 | 权威内容 |
| --- | --- |
| [03 — Gameplay 标签](./.docs/03-gameplay-tags.md) | 层级注册、位集、引用计数容器和标签条件 |
| [04 — 属性系统](./.docs/04-attributes.md) | ID 注册、冷热存储、快照、dirty 位图和重算 |
| [05 — 修饰器与聚合器](./.docs/05-modifiers-and-aggregator.md) | 六种 Modifier 运算、只读贡献访问、自定义聚合、幅度求值和来源 ID |
| [06 — Gameplay 效果](./.docs/06-gameplay-effects.md) | Effect 定义、应用计划、持续/周期、堆叠、免疫和条件收敛 |

### 行为领域

| 文档 | 权威内容 |
| --- | --- |
| [07 — Gameplay 技能](./.docs/07-gameplay-abilities.md) | Ability 定义、规格、激活、commit、链和活跃实例 |
| [08 — 技能任务](./.docs/08-ability-tasks.md) | Instant/WaitTicks、完成动作、事件和任务生命周期 |
| [15 — Gameplay 目标结果契约](./.docs/15-gameplay-targeting.md) | 有序目标数据、非空激活目标与外部查询接入 |
| [18 — 输入绑定迁移](./.docs/18-ability-input-bindings.md) | 模板所有权、新导入路径与 GAS 输入请求边界 |

### 用法、参考与维护

| 文档 | 权威内容 |
| --- | --- |
| [10 — 支撑基础设施](./.docs/10-supporting-infrastructure.md) | UniqueName、确定性 Random、容量与安全限制 |
| [11 — API 快速参考](./.docs/11-api-quick-reference.md) | 按领域查找常用公开入口，不替代 rustdoc |
| [12 — 使用模式与示例](./.docs/12-usage-patterns.md) | 可组合的 Gameplay 配置与调用模式 |
| [13 — 测试指南](./.docs/13-testing-guide.md) | 当前测试树、fixture、时间推进和提交前检查 |
| [14 — 扩展系统](./.docs/14-extending-the-system.md) | 新增规则、任务、目标操作和请求生产系统的方法 |
| [17 — 源码布局与维护边界](./.docs/17-source-layout-and-maintenance.md) | 真实文件布局、所有权、可见性、修改路由和文档约束 |
| [19 — 外部配置集成](./.docs/19-external-configuration.md) | 游戏配置所有权、公共运行时接口与升级验证 |
| [20 — 审计复核与后续项](./.docs/20-audit-findings-and-fixes.md) | 复核后保留的低优先级观察、决策条件与既有设计边界，不替代行为契约 |

## 公共入口约定

- `bevy_gas::prelude`：Plugin、核心 Component、常用定义和 SystemParam；
- `bevy_gas::gas::<domain>`：领域完整公共 API，例如
  `bevy_gas::gas::gameplay_effects::GameplayEffectApplicationError`；
- `bevy_gas::GameplayEffect` 这类路径：为现有调用方保留的显式 crate-root 兼容重导出；
- 私有子模块文件不是用户导入路径，文档提到它们只用于解释源码所有权。

许可证：[MIT](./LICENSE)。
