# 17 — 源码布局与维护边界

## 目标

源码按 Gameplay 领域组织，在每个领域内部再区分定义、运行时状态、执行流程和系统。
这种布局让一次功能修改尽量停留在一个领域中，同时保持 Bevy ECS 的 Component、Resource、
System 和 Event 边界清晰。

本项目不以“每个类型一个文件”或绝对行数作为目标。判断是否拆分文件时，优先检查文件是否
同时承担多个独立的变更原因，例如：

- 同时定义持久状态、应用事务和调度系统；
- 同时包含校验、执行、回滚和清理；
- 测试文件同时覆盖多个彼此独立的行为轴；
- 修改一个局部规则时，需要理解文件中大部分无关实现。

## 顶层组织原则

顶层模块按领域划分，而不是建立全局 `components/`、`systems/`、`resources/` 或
`utils/` 目录：

| 领域 | 门面模块 | 主要职责 | 权威文档 |
| --- | --- | --- | --- |
| Gameplay Tags | `gameplay_tags.rs` | 标签注册、层级位集、引用计数和标签条件 | [03](./03-gameplay-tags.md) |
| Attributes | `attributes.rs` | 属性 ID、存储、聚合、快照与延迟重算 | [04](./04-attributes.md)、[05](./05-modifiers-and-aggregator.md) |
| Modifiers | `modifiers.rs` | Modifier 操作、幅度、规格和聚合输入 | [05](./05-modifiers-and-aggregator.md) |
| Gameplay Effects | `gameplay_effects.rs` | Effect 定义、规格、应用、活跃状态和生命周期 | [06](./06-gameplay-effects.md) |
| Gameplay Abilities | `gameplay_abilities.rs` | Ability 定义、实例、激活上下文和任务 | [07](./07-gameplay-abilities.md)、[08](./08-ability-tasks.md) |
| Ability System | `ability_system.rs` | ASC、激活、commit 和生命周期编排 | [09](./09-ability-system-component.md) |
| Targeting | `gameplay_targeting.rs` | 目标管线、目标数据和请求队列 | [15](./15-gameplay-targeting.md) |
| Execution | `gameplay_execution.rs` | 跨类型 FIFO 与统一请求消费 | [16](./16-gameplay-execution.md) |
| Runtime Plugin | `gas/runtime_plugin.rs` | FixedUpdate 阶段、资源安装和系统排序 | [02](./02-plugins-and-lifecycle.md) |
| Supporting | `randoms.rs`、`unique_names.rs`、`gas/settings.rs` | 确定性随机、名称驻留和容量设置 | [10](./10-supporting-infrastructure.md) |

## 门面文件与实现目录

复杂模块采用“同名门面文件 + 同名目录”的布局：

```text
feature.rs
feature/
├── state.rs
├── application.rs
└── lifecycle.rs
```

门面文件只负责：

1. 声明私有子模块；
2. 通过显式 `pub use submodule::{Type, function}` 维护领域公共 API；
3. 通过 `pub(crate) use` 暴露确实需要跨领域使用的内部入口；
4. 使用 `//!` 说明模块职责和关键不变量。

拆分实现文件不得无意改变用户路径。移动公开类型或函数时，应继续从原门面重导出。
门面不得使用通配公开重导出，因为新增一个内部 `pub` 项不应自动扩大 crate API。

`bevy_tools::gas` 是 GAS 的规范命名空间；原有 crate-root 路径继续显式重导出以保持兼容。
`bevy_tools::prelude` / `bevy_tools::gas::prelude` 只包含 Plugin、核心 Component、常用定义和
SystemParam，不作为完整 API 镜像。

## Gameplay Effect 内部边界

Active Effect 运行时按以下职责分离：

| 子模块 | 所有权 |
| --- | --- |
| `state` | Handle、稳定 slot、Active Effect Component 和 tick 状态 |
| `planning` | 应用 Plan、公开错误、prepare 与错误映射 |
| `application` | 同步入口、堆叠查找、应用条件和免疫 |
| `execution` | Plan validate、Instant、Stack、Create、modifier 变更与回滚 |
| `removal` | 查询、显式移除、标签移除和状态清理 |
| `requirements` | ongoing/removal 条件、抑制和固定点收敛 |
| `ticking` | Duration 与 Period 的 FixedUpdate 系统 |

依赖方向应尽量由流程层指向状态层。低层 slot 存储不应构造高层的 Effect 应用错误；它只返回
内部存储错误，再由 application 层映射为公共错误。

固定点收敛、FIFO 可见性、回滚和 fail-closed 行为属于 Gameplay 语义，不能在机械拆文件时
改变。详见 [Gameplay Effects](./06-gameplay-effects.md) 与
[Gameplay Execution](./16-gameplay-execution.md)。

## Ability System 内部边界

Ability System 按以下职责分离：

| 子模块 | 所有权 |
| --- | --- |
| `params` | `AbilitySystemParams`、内嵌 Effect 参数和同批次 pending runtime state |
| `component` | ASC 的 Ability 规格存储、索引和查询 |
| `activation/error` | 公开激活错误和 rejection 分类 |
| `activation/validation` | 标签、冷却与支付能力预检 |
| `activation/startup` | Active Ability 实例和 startup task 创建 |
| `activation/execution` | 同步入口与 batch 激活编排 |
| `commit` | Cost/Cooldown 的准备、支付校验、执行和 commit 错误 |
| `lifecycle` | End、Cancel、回滚和 Cleanup system |

`EffectSystemParams` 是 Effect API 的窄公共边界，不包含 ASC 或 Active Ability 查询。
`AbilitySystemParams` 内嵌它并增加 Ability 编排状态。两者都只是访问边界，不表示实现应放在
同一个文件。

## 低层领域与 ECS 组合

- `gameplay_tags/` 按 `tag → bitset → registry → container → requirements` 分层；
- `attributes/` 按 registry、aggregation、snapshot 和 attribute-set state/mutation/recalculation
  分层；
- `modifiers/` 是 Attributes 与 Effects 共享的领域模块，但通过
  `ModifierEvaluationContext` 和 `ModifierSourceId` 保持对 Effect runtime 的独立；
- `GameplayTagContainer` 与 `AttributeSet` 可以独立存在，不得反向 Required
  `ActiveGameplayEffects`；
- 完整 Actor 使用 `GameplayAbilitySystemBundle` 显式组合 ASC、Attributes、Tags 与 Active
  Effects。

## 可见性约定

从窄到宽选择可见性：

1. 默认私有：只在当前子模块使用；
2. `pub(super)`：同一领域的兄弟子模块需要使用；
3. `pub(crate)`：确实需要跨领域调用的运行时内部接口；
4. `pub`：稳定且有文档的用户 API。

某些 Bevy public system 的函数签名会暴露内部 Resource 类型。此类类型可以使用
`#[doc(hidden)]` 表明不建议用户直接依赖，但在改变 public system 签名前不能直接降低可见性。

## 测试布局

测试按行为组织，而不是简单复制源文件名称：

```text
tests/gas_tests/
├── effects.rs
├── effects/
│   ├── application.rs
│   ├── stacking.rs
│   ├── requirements.rs
│   ├── ticking.rs
│   └── removal.rs
├── abilities.rs
├── abilities/
│   ├── activation.rs
│   ├── commit.rs
│   ├── lifecycle.rs
│   ├── tasks.rs
│   └── chaining.rs
├── support.rs
├── attributes_test.rs
├── gameplay_tags_test.rs
├── gameplay_targeting_test.rs
├── queues_test.rs
└── runtime_paths_test.rs
```

共享设施统一放在 `support`：App 构造、注册 helper、Effect/Ability builder、tick driver 和状态
查询。测试断言仍放在各行为模块中，避免 support 演变成隐藏业务逻辑的通用工具箱。

纯算法和私有不变量可以在源码旁编写单元测试；需要 Bevy World、调度顺序或公共 API 的场景
继续使用集成测试。

## 文档真相来源

项目使用三层文档来源：

- rustdoc：精确签名、参数、返回值和错误；
- `docs/`：行为、时序、不变量、设计理由和跨模块流程；
- `examples/` 与集成测试：由编译器验证的完整用法。

知识库不应长期复制私有 struct 字段或私有 enum 布局。需要解释实现时，优先描述稳定语义并
链接源码；可执行用法优先放在 example 或 doctest 中。

## 修改检查清单

进行源码结构调整时：

1. 先确认工作区基线测试通过；
2. 单个提交只处理一个领域或一个明确的依赖边界；
3. 机械移动阶段不改变 public 名称、错误语义、执行顺序或确定性；
4. 同步更新门面 `//!`、对应知识库章节和测试导航；
5. 检查新增内部接口是否可以使用更窄的可见性；
6. 运行完整检查：

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
```
