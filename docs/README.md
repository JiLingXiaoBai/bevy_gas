# bevy_tools — 知识库

> 为 Bevy 游戏引擎打造的 Gameplay Ability System (GAS) 库，灵感来源于虚幻引擎的 GAS 框架。

## 目录

### 入门

| 文档 | 说明 |
| --- | --- |
| [01 — 项目概述](./01-overview.md) | 架构、设计原则、源码树 |
| [12 — 使用模式与示例](./12-usage-patterns.md) | 直伤、DoT、Buff、连招和事件驱动 |

### 运行时架构

| 文档 | 说明 |
| --- | --- |
| [02 — 插件系统与生命周期](./02-plugins-and-lifecycle.md) | Plugin、FixedUpdate 管线和 SystemSet |
| [16 — Gameplay 执行模块](./16-gameplay-execution.md) | 统一请求、跨类型 FIFO、阶段边界与收敛 |
| [17 — 源码布局与维护边界](./17-source-layout-and-maintenance.md) | 模块所有权、门面结构、可见性和测试布局 |

### 领域模块

| 文档 | 说明 |
| --- | --- |
| [03 — Gameplay 标签](./03-gameplay-tags.md) | 层级位集标签、引用计数和注册 |
| [04 — 属性系统](./04-attributes.md) | 属性存储、延迟重算和快照 |
| [05 — 修饰器与聚合器](./05-modifiers-and-aggregator.md) | 操作、幅度类型和求值顺序 |
| [06 — Gameplay 效果](./06-gameplay-effects.md) | Buff/Debuff、堆叠、抑制、免疫与条件收敛 |
| [07 — Gameplay 技能](./07-gameplay-abilities.md) | 技能定义、激活流程和链式激活 |
| [08 — 技能任务](./08-ability-tasks.md) | 时间线编排、任务类型和事件系统 |
| [09 — 技能系统组件](./09-ability-system-component.md) | ASC、AbilitySystemParams 和激活 API |
| [10 — 支撑基础设施](./10-supporting-infrastructure.md) | UniqueName、Random 和 Settings |
| [15 — Gameplay 目标抓取](./15-gameplay-targeting.md) | 目标管线、队列、多目标技能和确定性 |

### 维护与参考

| 文档 | 说明 |
| --- | --- |
| [11 — API 快速参考](./11-api-quick-reference.md) | 常用公共入口导航；精确签名以 rustdoc 为准 |
| [13 — 测试指南](./13-testing-guide.md) | 测试组织、模式和提交前检查清单 |
| [14 — 扩展系统](./14-extending-the-system.md) | 如何新增操作、策略、任务和系统 |

## 快速开始

```rust
use bevy::prelude::*;
use bevy_tools::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GameplayAbilitySystemPlugin)
        .add_systems(
            Startup,
            (register_initial_tags, register_initial_attributes).chain(),
        )
        .run();
}

fn register_initial_tags(mut register: GameplayTagRegister) {
    if let Err(error) = register.request_or_register_tag("Effect.Debuff.Stun") {
        error!("failed to register gameplay tag: {error}");
    }
}

fn register_initial_attributes(mut register: AttributeIdRegister) {
    if let Err(error) =
        register.request_or_register_attribute_id("Health", AttributeRegion::Hot)
    {
        error!("failed to register Health: {error}");
        return;
    }
    if let Err(error) =
        register.request_or_register_attribute_id("MaxHealth", AttributeRegion::Cold)
    {
        error!("failed to register MaxHealth: {error}");
    }
}
```

## 核心概念速览

| 概念                       | Bevy 类型     | 用途                               |
| -------------------------- | ------------- | ---------------------------------- |
| **GameplayTag**            | `struct`      | 层级标签（位集存储，O(1) 查询）    |
| **AttributeSet**           | `Component`   | 每实体数值属性，含修饰器聚合       |
| **GameplayEffect**         | `Arc<struct>` | Buff/Debuff 定义（即时/持续/无限） |
| **GameplayAbility**        | `Arc<struct>` | 技能定义（冷却、消耗、任务）       |
| **AbilitySystemComponent** | `Component`   | 每实体技能授予与激活管理           |
| **GameplayAbilitySystemBundle** | `Bundle` | 显式组合完整 GAS Actor 所需组件 |
| **EffectSystemParams**     | `SystemParam` | Effect 专用的窄查询与资源边界      |
| **AbilitySystemParams**    | `SystemParam` | Effect 参数 + Ability 编排状态     |
| **TargetingDefinition**    | `Arc<struct>` | 有序的目标选择、过滤、排序管线      |
| **AbilityTargetData**      | `struct`      | 确定性排序的目标抓取结果            |
| **GameplayExecutionQueue** | `Resource`    | 技能与效果共享的确定性 FIFO         |

## 数据流

```
GameplayAbility (定义)
    │
    ├──► 冷却 GameplayEffect ──► 激活时应用到来源
    ├──► 消耗 GameplayEffect ──► 激活时应用到来源
    ├──► 激活效果列表          ──► 激活时应用到目标
    └──► AbilityTaskDef[]
            │
            ├──► Instant                        ──► 激活 batch 内派发
            ├──► WaitTicks                      ──► 生成 AbilityTask 实体
            ├──► ApplyGameplayEffectToTarget(s) ──► 写入统一 Gameplay FIFO
            ├──► ActivateAbility             ──► 链式激活另一个技能
            ├──► EmitEvent                   ──► 触发 AbilityTaskEvent
            └──► EndAbility                  ──► 结束当前技能

GameplayEffect (定义)
    │
    ├──► Modifier[] ──► ModifierSpec[] ──► 应用到 AttributeSet
    │       ├── Instant   → 修改 base 值
    │       └── Duration  → 加入 Aggregator
    ├──► EffectTags
    │       ├── asset_tags           → 身份标识
    │       ├── granted_tags         → 激活期间授予
    │       ├── application tags     → 来源/目标必须满足
    │       ├── ongoing tags         → 必须维持（抑制检查）
    │       ├── removal tags         → 触发移除
    │       └── immunity queries     → 阻止传入效果
    └──► StackingPolicy
            ├── StackingType         → None / AggregateBySource / AggregateByTarget
            ├── StackLimit           → 最大堆叠数
            └── 子策略               → 幅度、持续时间、周期、溢出、过期
```

## 许可证

MIT
