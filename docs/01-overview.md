# 01 — 项目概述

## 什么是 `bevy_tools`？

`bevy_tools` 是一个为 [Bevy](https://bevyengine.org/) 游戏引擎 (0.19) 打造的
**Gameplay Ability System (GAS)** 库，设计灵感来源于虚幻引擎的 GAS 框架。
它提供了模块化、ECS 友好的架构，用于构建复杂的 RPG / MOBA / ARPG 游戏机制。

## 核心目标

| 优先级 | 目标         | 说明                                                                                  |
| ------ | ------------ | ------------------------------------------------------------------------------------- |
| 1      | **正确性**   | 所有系统在所有合法输入下必须产生正确结果                                              |
| 2      | **可读性**   | 代码应易于理解和维护                                                                  |
| 3      | **性能**     | 仅在 Profiling 后进行优化；优先选择缓存友好、栈分配的数据结构                         |
| 4      | **确定性**   | Gameplay 逻辑应保持确定性 — 避免依赖 HashMap 遍历顺序、平台相关浮点行为、隐藏全局状态 |
| 5      | **最小依赖** | 仅依赖 `bevy` 0.19 和 `rand` 0.10.2，不引入不必要的第三方 crate                       |

## 架构总览

```
┌──────────────────────────────────────────────────────────────────┐
│                     GameplayAbilitySystemPlugin                  │
│  (PluginGroup: UniqueName + GameplayTag + Random + Runtime)      │
├──────────────────────────────────────────────────────────────────┤
│  FixedUpdate 管线 (有序 SystemSet)                               │
│                                                                  │
│  EffectTicks → AbilityTasks → RequestProducers → Targeting       │
│       → PreGameplayConvergence → GameplayResolve                 │
│       → UpdateEffectTagRequirements → Cleanup                    │
│       → RecalculateAttributes                                    │
├──────────────────────────────────────────────────────────────────┤
│  核心模块                                                        │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐    │
│  │ GameplayTags │  │  属性系统    │  │      修饰器          │    │
│  │ (位集)       │  │ (聚合器)     │  │ (加/百分比/乘/覆盖)  │    │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘    │
│         │                 │                     │                │
│         ▼                 ▼                     ▼                │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │                   GameplayEffects                        │    │
│  │  即时 / 持续Tick / 无限                                  │    │
│  │  堆叠 · 周期 · 免疫 · 抑制 · 标签要求                    │    │
│  └──────────────────────────┬───────────────────────────────┘    │
│                             │                                    │
│                             ▼                                    │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │                  GameplayTargeting                       │    │
│  │  选择 · 过滤 · 排序 · 限制 · 确定性目标数据              │    │
│  └──────────────────────────┬───────────────────────────────┘    │
│                             ▼                                    │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │                  GameplayAbilities                       │    │
│  │  冷却 · 消耗 · 激活效果 · 技能任务                       │    │
│  └──────────────────────────┬───────────────────────────────┘    │
│                             │                                    │
│                             ▼                                    │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │               AbilitySystemComponent (ASC)               │    │
│  │  每实体技能授予 · 激活 · 活跃计数                        │    │
│  └──────────────────────────────────────────────────────────┘    │
│                                                                  │
│  GameplayExecution：技能 + 效果共享的跨类型 FIFO 与结算器        │
├──────────────────────────────────────────────────────────────────┤
│  支撑基础设施                                                    │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐    │
│  │ UniqueNames  │  │   Randoms    │  │     Settings         │    │
│  │ (字符串驻留) │  │ (种子随机数) │  │ (全局常量)           │    │
│  └──────────────┘  └──────────────┘  └──────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
```

## 源码树

```
src/
├── lib.rs                        # crate 文档与公共重导出
├── gas.rs                        # GAS 门面模块与公共重导出
├── gas/                          # Gameplay Ability System (核心)
│   ├── runtime_plugin.rs          # PluginGroup、FixedUpdate 阶段和系统排序
│   ├── prelude.rs                 # 精简常用 API
│   ├── gameplay_tags.rs          # 标签领域门面
│   ├── gameplay_tags/            # tag、bitset、registry、container、requirements
│   ├── attributes.rs             # 属性领域门面
│   ├── attributes/               # registry、aggregation、snapshot、attribute_set/*
│   ├── modifiers.rs              # Modifier 领域门面
│   ├── modifiers/                # definition、context、spec
│   ├── gameplay_effects.rs       # Gameplay Effect 领域门面
│   ├── gameplay_effects/
│   │   ├── effect_system_params.rs # 独立 Effect ECS 访问边界
│   │   ├── gameplay_effect.rs    # Effect 定义门面
│   │   ├── gameplay_effect/      # context、timing、stacking、effect_tags、definition
│   │   ├── active_gameplay_effect.rs  # Active Effect 运行时门面
│   │   └── active_gameplay_effect/    # state、planning、application、execution、removal、requirements、ticking
│   ├── gameplay_abilities.rs     # Ability 领域门面
│   ├── gameplay_abilities/       # 定义、规格、active_gameplay_ability/* 与 ability_task/*
│   ├── ability_system.rs         # ASC 领域门面
│   ├── ability_system/            # component、params、commit、lifecycle、activation/*
│   ├── gameplay_execution.rs     # Gameplay 执行门面模块
│   ├── gameplay_execution/       # request、queue、resolver
│   ├── gameplay_targeting.rs     # Targeting 领域门面
│   ├── gameplay_targeting/       # target data、definition、acquisition、targeting_queue/*
│   └── settings.rs               # 全局常量
├── randoms.rs + randoms/         # 确定性 RNG 封装 (Bevy Resource)
└── unique_names.rs + unique_names/ # 字符串驻留池 (hash → u32)

examples/
└── tag_registration.rs           # 可运行的标签注册示例

tests/
├── gas_tests.rs                  # GAS 集成测试门面
├── gas_tests/                    # 按行为拆分的 Effect/Ability 测试与共享 support
├── randoms_tests.rs
└── unique_names_tests.rs
```

复杂领域统一采用 `foo.rs + foo/` 的门面布局；具体所有权和可见性约定见
[17 — 源码布局与维护边界](./17-source-layout-and-maintenance.md)。

## 核心设计原则

1. **ECS 优先** — 一切皆为 Component、Resource、System 或 Event。不使用 OOP 风格的继承。
2. **Tick 计时** — 所有持续时间、周期、任务等待均以 `FixedUpdate` tick 为单位，而非挂钟秒数。
3. **统一执行 FIFO** — 技能激活和效果应用按跨类型 FIFO 在 `GameplayResolve` 完整消费；
   请求量不会把 Gameplay mutation 隐式推迟到后续 tick。
4. **脏标记模式** — `AttributeSet` 的冷热位图是唯一脏状态；`Changed<AttributeSet>` 驱动按位重算。
5. **引用计数标签** — `GameplayTagContainer` 追踪每个位被设置的次数，防止重叠的效果授予/移除互相干扰。
6. **Arc 共享定义** — `GameplayEffect` 和 `GameplayAbility` 定义通过 `Arc` 共享；规格通过 `Arc::ptr_eq` 比较。
