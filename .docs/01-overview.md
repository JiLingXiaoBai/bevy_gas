# 01 — 项目与架构总览

## 项目定位

`bevy_gas` 是面向 Bevy 0.20 的 ECS-first Gameplay Ability System。它提供构建 RPG、MOBA、
ARPG 等 Gameplay 规则所需的基础能力。空间查询、逻辑位置、攻击命中、表现插值、具体输入映射、
动画、视觉资源、网络同步和资源序列化格式由游戏决定。

当前依赖基线为 Bevy 0.20.0、Rust 1.97.1+（edition 2024）；引擎接口迁移说明见
[02 — 插件与生命周期](./02-plugins-and-lifecycle.md#bevy-020-升级基线)。
库的 Bevy 依赖关闭默认 features，仅启用 `std`、`multi_threaded` 和 `bevy_log`。
输入绑定和 Bevy 几何采样由模板维护，GAS 不直接依赖 `bevy_shape`。窗口、渲染及资源格式
由游戏工程选择，库仍依赖标准库，不承诺 `no_std`。

设计优先级是：

1. 正确性；
2. 可读性与长期维护性；
3. 经 profiling 证明有价值的性能优化；
4. 可观察且尽量确定的 fixed-tick Gameplay 语义；
5. 最少的第三方依赖。

## 领域边界

| 领域 | 拥有的概念 | 不负责 |
| --- | --- | --- |
| Gameplay Tags | 层级 Tag、位集、注册和引用计数容器 | Effect 生命周期和 Ability 激活 |
| Attributes | Attribute ID、冷热存储、聚合器、快照和重算 | Modifier 何时生效 |
| Modifiers | 修改操作、幅度求值接口、求值结果和来源 ID | Effect 持续时间与 Attribute 存储 |
| Gameplay Effects | Effect 定义、应用计划、活跃效果、堆叠、免疫和条件收敛 | 技能实例生命周期 |
| Gameplay Abilities | Ability 定义、规格、激活上下文、活跃实例和任务定义 | ASC 的授予索引和统一请求调度 |
| Ability System | ASC、激活校验、cost/cooldown commit、结束和取消 | 通用目标搜索 |
| Targeting 结果契约 | 有序目标结果、非空激活目标值 | 空间查询、碰撞、锁定与表现 |
| Gameplay Execution | Ability/Effect 共用的跨类型 FIFO 和 batch resolver | 定义具体 Gameplay 规则 |
| Runtime Plugin | GAS Resource 安装与 FixedUpdate 跨领域阶段排序 | 游戏侧移动/请求策略与表现调度 |

实际文件和子模块所有权集中记录在
[17 — 源码布局与维护边界](./17-source-layout-and-maintenance.md)，本页不重复私有文件细节。

## ECS 数据所有权

完整 Gameplay Actor 使用 `GameplayAbilitySystemBundle` 显式组合四个彼此独立的 Component：

| Component | 持有的数据 |
| --- | --- |
| `AbilitySystemComponent` | 已授予的 `GameplayAbilitySpec`、Handle 索引和 Ability 阻止状态 |
| `GameplayTagContainer` | 当前拥有 Tag 的层级位集与每个位的引用计数 |
| `AttributeSet` | 已初始化属性、稀疏 Aggregator 和 hot/cold dirty 位图 |
| `ActiveGameplayEffects` | 目标当前持有的 Duration/Infinite Effect 稳定槽位 |

`GameplayTagContainer` 和 `AttributeSet` 可以单独挂载；它们不会反向安装
`ActiveGameplayEffects`。Bundle 也不会注册 Tag/Attribute ID、初始化具体属性值或授予技能，这些
仍由游戏初始化系统显式完成。

以下运行时状态不放进 Bundle：

- `ActiveGameplayAbility`：一次活跃技能实例，存在于独立运行时实体；
- `AbilityTask`：需要跨 tick 等待的任务实体；
- `AttributeSetSnapshot`：按玩法需要显式捕获和挂载的来源属性快照；
- 全局注册表、队列和内部同步状态：由 Plugin 作为 Resource 安装。

## 定义与运行时数据流

### 定义持有关系

```mermaid
flowchart TB
    AbilitySpec["ASC 中的 GameplayAbilitySpec"]
    AbilityDef["GameplayAbility（Arc 共享）"]
    TaskDef["startup tasks：AbilityTaskDef"]
    EffectDef["GameplayEffect（Arc 共享）"]
    ModifierDef["Modifier"]

    AbilitySpec -- "共享" --> AbilityDef
    AbilityDef -- "包含" --> TaskDef
    AbilityDef -- "引用" --> EffectDef
    EffectDef -- "包含" --> ModifierDef
```

第一张图只表示值的持有与引用关系。`GameplayAbility` 通过 cost、cooldown 和 startup task
引用 `GameplayEffect`。`GameplayEffect` 在 prepare 时把 `Modifier` 求值为
`ModifierSpec`；定义对象不会主动触发运行时执行。

### 运行时主路径

```mermaid
flowchart TB
    Game["游戏输入、AI、空间查询或命中检测"]
    Targets["Entity / AbilityTargetData"]
    Activation["AbilityActivationTargets"]
    Execution["GameplayExecutionQueue"]
    Game --> Targets --> Activation
    Activation -- "激活请求" --> Execution
    Game -- "效果应用请求" --> Execution
```

请求生产者包括 `AbilityTasks`、`RequestProducers`、游戏系统、AI 和输入适配。进入统一队列后，
请求按跨类型 FIFO 顺序执行：

```mermaid
flowchart TB
    Execution["GameplayExecutionQueue"]
    AbilityRuntime["Ability 激活：校验、commit、startup"]
    EffectRuntime["Effect 应用：prepare、execute"]
    AbilityState["ASC 和 ActiveGameplayAbility"]
    WaitTask["WaitTicks 创建 AbilityTask 实体"]
    AbilityEffects["cost、cooldown、startup Instant 效果"]
    EffectLifetime["持续效果和授予标签"]
    AttributeState["属性 base、Aggregator 和 dirty bits"]

    Execution --> AbilityRuntime
    Execution --> EffectRuntime
    AbilityRuntime --> AbilityState
    AbilityRuntime --> WaitTask
    AbilityRuntime -- "直接应用" --> AbilityEffects
    AbilityEffects --> EffectRuntime
    EffectRuntime --> EffectLifetime
    EffectRuntime --> AttributeState
```

后两张图表示游戏接入与 GAS 主运行时数据流，需注意：

- 游戏负责生成目标结果，并决定何时提交激活或效果请求；GAS 不强制安装任何空间索敌后端；
- startup `Instant` 在技能激活内逐动作同步结算效果与链式子技能 startup，不创建任务实体；
  `WaitTicks` 创建 `AbilityTask`，游戏层也可手动创建运行时 Instant。运行时任务的
  `ActivateAbility` 与 `ApplyGameplayEffect*` 仍追加 Gameplay 请求；`EmitEvent` 保持 deferred，
  `EndAbility` 更新技能状态；
- Instant/periodic Effect 通过 `ModifierSpec` 修改 Attribute base；非周期 Duration/Infinite Effect
  把 modifier 写入 Aggregator 并标记 dirty。只有 Duration/Infinite Effect 存入
  `ActiveGameplayEffects`，其 granted tags 随 Active Effect 生命周期维护；dirty Attribute 在读取时
  按需重算，未被读取的值由 `RecalculateAttributes` 统一重算；
- 同步 `try_activate_ability_by_handle()` / `apply_gameplay_effect()` API 可以绕过全局 FIFO；常规
  运行时生产者应优先写入 `GameplayExecutionQueue`，以保留跨类型顺序。

## FixedUpdate 管线

`GameplayAbilitySystemRuntimePlugin` 配置以下稳定阶段：

```text
EffectTicks
  → AbilityTasks
  → RequestProducers
  → PreGameplayConvergence
  → GameplayResolve
  → UpdateEffectTagRequirements
  → Cleanup
  → RecalculateAttributes
```

关键边界：

- `EffectTicks` 内部按 Duration → Requirement → Period → Requirement 串行执行；
- 游戏通过公开阶段插入移动、空间同步和目标查询；模板由自己的插件维护该顺序与表现；
- `AbilityTasks`、`RequestProducers` 及显式排序在 resolver 前的游戏系统产生的请求可以在当前
  fixed tick 消费；
- `GameplayResolve` 完整 drain Ability/Effect 共用 FIFO，包括 drain 期间直接追加的有限请求；
- resolver 之后才入队的请求属于下一 fixed tick；
- Effect Requirement 在 tick、执行前后设置收敛点；
- Cleanup 先结束 Ability，最后才统一重算仍为 dirty 的 Attribute。

精确时序和 Observer/deferred command 边界分别见
[02 — 插件系统与生命周期](./02-plugins-and-lifecycle.md) 与
[16 — Gameplay 执行模块](./16-gameplay-execution.md)。

## 公共 API 层次

```rust
// Common integration surface.
use bevy_gas::prelude::*;

// Complete domain surface.
use bevy_gas::gas::gameplay_effects::{
    GameplayEffectApplicationError,
    prepare_gameplay_effect,
};
```

- `bevy_gas::prelude` 有意保持精简；
- `bevy_gas::gas::<domain>` 是按领域查找完整 API 的规范位置；
- crate root 继续显式重导出现有 API，以降低迁移成本；
- 私有实现文件不是稳定导入路径，公开项由同名领域门面显式控制。

## 核心不变量

1. **Tick 计时**：Effect duration、period 和 task wait 都以 `FixedUpdate` tick 表示；
2. **显式组合**：完整 GAS Actor 使用 Bundle，低层 Component 保持可独立使用；
3. **跨类型顺序**：Ability activation 与 Effect application 共享一个 FIFO；
4. **请求间可见性**：同 batch 较早请求产生的 Active Effect、Tag 和 Attribute 变化对后续请求可见；
5. **引用计数 Tag**：重叠授予和移除不会互相误清理；
6. **带世代句柄**：Active Effect slot 复用不会让旧 Handle 命中新 Effect；
7. **脏位图重算**：Attribute 只重算被标记的 hot/cold slot；
8. **确定性顺序**：需要顺序时显式排序，不依赖 HashMap 或 Bevy 并行调度的偶然顺序；
9. **共享不可变定义**：Effect/Ability 定义使用 `Arc`，运行时规格与状态独立保存；
10. **具体错误**：外部输入、容量、配置和 ECS 状态失败返回 `Result`，不依赖 panic。

## 下一步阅读

- 接入 Plugin 与阶段：[02 — 插件系统与生命周期](./02-plugins-and-lifecycle.md)
- 创建完整 Gameplay Actor：[09 — 技能系统组件](./09-ability-system-component.md)
- 理解 Effect 结算：[06 — Gameplay 效果](./06-gameplay-effects.md)
- 理解统一 FIFO：[16 — Gameplay 执行模块](./16-gameplay-execution.md)
- 查找实际目录和修改位置：[17 — 源码布局与维护边界](./17-source-layout-and-maintenance.md)
