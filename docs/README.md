# bevy_gas — 知识库

本知识库描述当前源码中的 Gameplay Ability System。阅读时采用以下事实优先级：

1. rustdoc 和领域门面模块：公共名称、精确签名和错误类型；
2. `docs/`：行为语义、阶段顺序、设计理由和跨模块约束；
3. `examples/` 与 `tests/`：由编译器验证的完整用法和边界场景。

文档不会把所有私有字段复制成长期契约。需要精确 API 时，从
`bevy_gas::gas::<domain>` 的 rustdoc 开始；常用入口可使用 `bevy_gas::prelude::*`。

## 推荐阅读路径

第一次阅读 Ability/Effect 时，建议先运行
[`ability_effect_flow`](../examples/ability_effect_flow.rs)：
`cargo run --example ability_effect_flow`。它完整展示一次火球从授予、激活到伤害、结束与冷却到期的
过程，再沿示例中的公共 API 阅读下面的领域文档。

| 目标 | 建议顺序 |
| --- | --- |
| 第一次接入 GAS | [01 总览](./01-overview.md) → [02 插件与生命周期](./02-plugins-and-lifecycle.md) → [09 ASC](./09-ability-system-component.md) → [12 使用模式](./12-usage-patterns.md) |
| 理解一次技能如何结算 | [07 技能](./07-gameplay-abilities.md) → [08 任务](./08-ability-tasks.md) → [15 目标抓取](./15-gameplay-targeting.md) → [16 统一执行](./16-gameplay-execution.md) |
| 接入输入与技能栏 | [18 输入绑定](./18-ability-input-bindings.md) → [09 ASC](./09-ability-system-component.md) → [16 统一执行](./16-gameplay-execution.md) |
| 扩展数值与状态系统 | [03 标签](./03-gameplay-tags.md) → [04 属性](./04-attributes.md) → [05 修饰器](./05-modifiers-and-aggregator.md) → [06 效果](./06-gameplay-effects.md) |
| 维护 Excel 配置与导表工具 | [19 配置工程与工具链](./19-luban-toolchain.md) |
| 维护或重构源码 | [17 源码布局](./17-source-layout-and-maintenance.md) → [13 测试](./13-testing-guide.md) → [14 扩展系统](./14-extending-the-system.md) |

## 文档目录

### 架构与运行时

| 文档 | 权威内容 |
| --- | --- |
| [01 — 项目概述](./01-overview.md) | 领域职责、ECS 数据所有权、整体数据流和核心不变量 |
| [02 — 插件系统与生命周期](./02-plugins-and-lifecycle.md) | PluginGroup、资源安装、`FixedUpdate` SystemSet 顺序 |
| [09 — 技能系统组件](./09-ability-system-component.md) | ASC、显式 Bundle、Ability/Effect SystemParam 边界 |
| [16 — Gameplay 执行模块](./16-gameplay-execution.md) | 统一请求 FIFO、batch 可见性、同 tick/下一 tick 边界 |

### 数据与规则领域

| 文档 | 权威内容 |
| --- | --- |
| [03 — Gameplay 标签](./03-gameplay-tags.md) | 层级注册、位集、引用计数容器和标签条件 |
| [04 — 属性系统](./04-attributes.md) | ID 注册、冷热存储、快照、dirty 位图和重算 |
| [05 — 修饰器与聚合器](./05-modifiers-and-aggregator.md) | Modifier 共享边界、幅度求值、来源 ID 和聚合顺序 |
| [06 — Gameplay 效果](./06-gameplay-effects.md) | Effect 定义、应用计划、持续/周期、堆叠、免疫和条件收敛 |

### 行为领域

| 文档 | 权威内容 |
| --- | --- |
| [07 — Gameplay 技能](./07-gameplay-abilities.md) | Ability 定义、规格、激活、commit、链和活跃实例 |
| [08 — 技能任务](./08-ability-tasks.md) | Instant/WaitTicks、完成动作、事件和任务生命周期 |
| [15 — Gameplay 目标抓取](./15-gameplay-targeting.md) | 有序目标管线、同步抓取、请求队列和多目标数据 |
| [18 — 技能输入绑定](./18-ability-input-bindings.md) | 逻辑动作与技能栏绑定、输入边界、重绑和生命周期 |

### 用法、参考与维护

| 文档 | 权威内容 |
| --- | --- |
| [10 — 支撑基础设施](./10-supporting-infrastructure.md) | UniqueName、确定性 Random、容量与安全限制 |
| [11 — API 快速参考](./11-api-quick-reference.md) | 按领域查找常用公开入口，不替代 rustdoc |
| [12 — 使用模式与示例](./12-usage-patterns.md) | 可组合的 Gameplay 配置与调用模式 |
| [13 — 测试指南](./13-testing-guide.md) | 当前测试树、fixture、时间推进和提交前检查 |
| [14 — 扩展系统](./14-extending-the-system.md) | 新增规则、任务、目标操作和请求生产系统的方法 |
| [17 — 源码布局与维护边界](./17-source-layout-and-maintenance.md) | 真实文件布局、所有权、可见性、修改路由和文档约束 |
| [19 — Luban 配置工程与工具链](./19-luban-toolchain.md) | 配置路径、导表入口、MCP 接入、AI skills、固定版本和升级约定 |

## 最小初始化

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
注册的 Tag 和 Attribute ID。Bundle 只负责组合 Component，不会替实体初始化具体属性值或授予
技能。

## 公共入口约定

- `bevy_gas::prelude`：Plugin、核心 Component、常用定义和 SystemParam；
- `bevy_gas::gas::<domain>`：领域完整公共 API，例如
  `bevy_gas::gas::gameplay_effects::GameplayEffectApplicationError`；
- `bevy_gas::GameplayEffect` 这类路径：为现有调用方保留的显式 crate-root 兼容重导出；
- 私有子模块文件不是用户导入路径，文档提到它们只用于解释源码所有权。

许可证：MIT。
