# bevy_tools — 知识库

> 为 Bevy 游戏引擎打造的 Gameplay Ability System (GAS) 库，灵感来源于虚幻引擎的 GAS 框架。

## 目录

| #   | 文档                                                   | 说明                                        |
| --- | ------------------------------------------------------ | ------------------------------------------- |
| 01  | [项目概述](./01-overview.md)                           | 架构、设计原则、源码树                      |
| 02  | [插件系统与生命周期](./02-plugins-and-lifecycle.md)    | 插件、FixedUpdate 管线、SystemSet、全局设置 |
| 03  | [Gameplay 标签](./03-gameplay-tags.md)                 | 层级位集标签、引用计数、注册                |
| 04  | [属性系统](./04-attributes.md)                         | 属性系统、延迟重算、Clamp、快照             |
| 05  | [修饰器与聚合器](./05-modifiers-and-aggregator.md)     | 修饰器操作、幅度类型、求值顺序              |
| 06  | [Gameplay 效果](./06-gameplay-effects.md)              | Buff/Debuff 系统、堆叠、抑制、免疫、队列    |
| 07  | [Gameplay 技能](./07-gameplay-abilities.md)            | 技能定义、激活流程、链式激活                |
| 08  | [技能任务](./08-ability-tasks.md)                      | 时间线编排、任务类型、事件系统              |
| 09  | [技能系统组件 (ASC)](./09-ability-system-component.md) | ASC、AbilitySystemParams、激活 API          |
| 10  | [支撑基础设施](./10-supporting-infrastructure.md)      | UniqueName、Random、Settings                |
| 11  | [API 快速参考](./11-api-quick-reference.md)            | 完整公开 API 索引                           |
| 12  | [使用模式与示例](./12-usage-patterns.md)               | 常见模式：直伤、DoT、Buff、连招、事件驱动   |
| 13  | [测试指南](./13-testing-guide.md)                      | 测试组织、模式、提交前检查清单              |
| 14  | [扩展系统](./14-extending-the-system.md)               | 如何新增操作、策略、任务、系统              |

## 快速开始

```rust
use bevy::prelude::*;
use bevy_tools::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GameplayAbilitySystemPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut tag_register: GameplayTagRegister, mut attr_register: AttributeIdRegister) {
    // 注册标签
    let stun = tag_register.request_or_register_tag("Effect.Debuff.Stun").unwrap();

    // 注册属性
    let health = attr_register
        .request_or_register_attribute_id("Health", AttributeRegion::Hot)
        .unwrap();
    let max_health = attr_register
        .request_or_register_attribute_id("MaxHealth", AttributeRegion::Cold)
        .unwrap();
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
| **AbilitySystemParams**    | `SystemParam` | 聚合所有 GAS 查询与资源的系统参数  |

## 数据流

```
GameplayAbility (定义)
    │
    ├──► 冷却 GameplayEffect ──► 激活时应用到来源
    ├──► 消耗 GameplayEffect ──► 激活时应用到来源
    ├──► 激活效果列表          ──► 激活时应用到目标
    └──► AbilityTaskDef[]     ──► 生成为 AbilityTask 实体
            │
            ├──► ApplyGameplayEffectToTarget ──► 向目标队列效果
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
            ├── StackingType         → None / BySource / ByTarget
            ├── StackLimit           → 最大堆叠数
            └── 子策略               → 幅度、持续时间、周期、溢出、过期
```

## 许可证

MIT
