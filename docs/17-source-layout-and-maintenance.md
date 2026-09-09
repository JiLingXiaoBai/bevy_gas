# 17 — 源码布局与维护边界

## 本文职责

本文是源码物理布局和模块所有权的权威说明。行为语义由各领域文档负责，精确公共签名由
rustdoc 负责。目录变化后应优先更新本页，而不是在多篇领域文档中复制完整文件树。

项目按 Gameplay 领域组织，不建立横跨所有领域的 `components/`、`systems/`、`resources/` 或
`utils/` 目录。**所有新添加的功能模块统一使用“门面文件 + 同名实现目录”布局，即使只有一个
实现文件也不例外。** `randoms.rs` + `randoms/random.rs`、`unique_names.rs` +
`unique_names/unique_name.rs` 和 `ability_input.rs` + `ability_input/bindings.rs` 都遵循这一规则。

最小布局以输入绑定模块为例：

```text
ability_input.rs
ability_input/
└── bindings.rs
```

门面只负责模块文档、私有子模块声明和显式重导出：

```rust
//! Optional mappings from logical actions to granted ability handles.

mod bindings;

pub use bindings::{AbilityInputBindingError, AbilityInputBindings};
```

组件、错误类型和相关实现放在 `ability_input/bindings.rs`。门面决定领域边界，实现文件只承担
一种主要变更原因；功能增长后，再按职责在同名目录中增加 `state.rs`、`definition.rs` 或
`execution.rs` 等文件。

这条规则适用于新建的功能模块边界，不要求目录内每个叶子实现文件继续递归建立门面和同名目录。
小型值类型与紧密相关的实现仍放在同一职责文件中，也不因这条规则批量调整既有辅助文件。

## 当前源码树

以下结构对应当前仓库，不包含 `target/` 等生成内容：

```text
src/
├── lib.rs
├── randoms.rs
├── randoms/
│   └── random.rs
├── unique_names.rs
├── unique_names/
│   └── unique_name.rs
├── gas.rs
└── gas/
    ├── prelude.rs
    ├── runtime_plugin.rs
    ├── settings.rs
    ├── gameplay_tags.rs
    ├── gameplay_tags/
    │   ├── tag.rs
    │   ├── bitset.rs
    │   ├── registry.rs
    │   ├── container.rs
    │   └── requirements.rs
    ├── attributes.rs
    ├── attributes/
    │   ├── registry.rs
    │   ├── aggregation.rs
    │   ├── snapshot.rs
    │   ├── attribute_set.rs
    │   └── attribute_set/
    │       ├── state.rs
    │       ├── mutation.rs
    │       └── recalculation.rs
    ├── modifiers.rs
    ├── modifiers/
    │   ├── definition.rs
    │   ├── context.rs
    │   └── spec.rs
    ├── gameplay_effects.rs
    ├── gameplay_effects/
    │   ├── effect_system_params.rs
    │   ├── gameplay_effect_spec.rs
    │   ├── gameplay_effect.rs
    │   ├── gameplay_effect/
    │   │   ├── definition.rs
    │   │   ├── context.rs
    │   │   ├── timing.rs
    │   │   ├── stacking.rs
    │   │   └── effect_tags.rs
    │   ├── active_gameplay_effect.rs
    │   └── active_gameplay_effect/
    │       ├── state.rs
    │       ├── planning.rs
    │       ├── application.rs
    │       ├── execution.rs
    │       ├── modifiers.rs
    │       ├── removal.rs
    │       ├── requirements.rs
    │       └── ticking.rs
    ├── gameplay_abilities.rs
    ├── gameplay_abilities/
    │   ├── gameplay_ability.rs
    │   ├── gameplay_ability_spec.rs
    │   ├── activation_data.rs
    │   ├── activation_context.rs
    │   ├── ability_chain.rs
    │   ├── active_gameplay_ability.rs
    │   ├── active_gameplay_ability/
    │   │   └── state.rs
    │   ├── ability_task.rs
    │   └── ability_task/
    │       ├── context.rs
    │       ├── definition.rs
    │       ├── state.rs
    │       ├── completion.rs
    │       └── ticking.rs
    ├── ability_input.rs
    ├── ability_input/
    │   └── bindings.rs
    ├── ability_system.rs
    ├── ability_system/
    │   ├── component.rs
    │   ├── params.rs
    │   ├── commit.rs
    │   ├── lifecycle.rs
    │   ├── activation.rs
    │   └── activation/
    │       ├── error.rs
    │       ├── validation.rs
    │       ├── startup.rs
    │       └── execution.rs
    ├── gameplay_targeting.rs
    ├── gameplay_targeting/
    │   ├── ability_target_data.rs
    │   ├── activation_targets.rs
    │   ├── targeting_definition.rs
    │   ├── acquisition.rs
    │   ├── targeting_queue.rs
    │   └── targeting_queue/
    │       ├── request.rs
    │       ├── queue.rs
    │       └── processing.rs
    ├── gameplay_execution.rs
    └── gameplay_execution/
        ├── request.rs
        ├── queue.rs
        └── resolver.rs
```

完整技能与效果时间线示例位于 `examples/ability_effect_flow.rs`；标签注册示例位于
`examples/tag_registration.rs`；独立输入绑定示例位于 `examples/ability_input_bindings.rs`。
集成测试布局见本文后半部分。

## 配置工程与工具目录

`config/` 拥有配置输入和项目导表入口，`tools/luban/` 拥有生成器版本、准备和启动逻辑。
详细路径、日常命令及示例来源见 [19 — Luban 配置工程与工具链](./19-luban-toolchain.md)。

| 路径 | 维护边界 |
| --- | --- |
| `config/tables/`、`config/defines/` | 人工维护的 Excel 数据及结构定义，纳入 Git |
| `config/luban.conf`、`config/export.ps1` | 项目输入和输出约定、严格导表入口，纳入 Git |
| `config/generated/` | 自动生成的 `cfg`、`macros` Rust 源码和 Cargo 清单，纳入 Git，不手动修改 |
| `config/bin/` | 自动生成的二进制，Git 忽略，不放手写文件 |
| `tools/luban/` | 固定工具链和本机 .NET 环境检查脚本 |
| `tools/luban/.cache/` | 可重新下载的 Luban 归档、已安装工具及临时验证产物，Git 忽略 |

配置源文件或结构定义变更后，应重新导表并将相关生成代码放在同一次提交中。
仓库使用 `target/` 忽略所有层级的构建目录，包含生成子 crate 的编译产物。

生成的 crate 尚未加入根项目依赖，GAS 运行时继续由 `src/` 拥有。
手写读取库、GAS 适配或自定义模板必须放在生成目录之外；游戏资源部署由使用本库的游戏负责。

## 顶层门面和公开路径

| 文件 | 职责 |
| --- | --- |
| `src/lib.rs` | crate 文档、`gas` 命名空间、Random/UniqueName 和 crate-root 兼容重导出 |
| `src/gas.rs` | 声明 GAS 领域模块并显式聚合公共 API |
| `src/gas/<domain>.rs` | 声明私有实现子模块，显式维护该领域的 `pub use` / `pub(crate) use` |
| `src/gas/prelude.rs` | 只重导出高频 Plugin、Component、定义和 SystemParam |
| `src/gas/runtime_plugin.rs` | Plugin 组合、Resource 初始化和 `FixedUpdate` 阶段排序 |
| `src/gas/settings.rs` | 编译期容量和递归安全上限 |

公开路径分三层：

1. `bevy_gas::prelude::*`：常规接入；
2. `bevy_gas::gas::gameplay_effects::GameplayEffect` 这类领域路径：完整领域 API；
3. `bevy_gas::GameplayEffect` 这类 crate-root 路径：保留的显式兼容重导出。

门面禁止 `pub use *`。新增实现文件中的 `pub` 项不会自动成为 crate API；只有被门面明确重导出的
项才属于领域公共表面。私有文件名可以调整，但不得在没有迁移方案时改变已公开的类型和函数路径。

## 领域内部所有权

### Tags、Attributes 与 Modifiers

| 文件 | 主要所有权 |
| --- | --- |
| `gameplay_tags/tag.rs` | `GameplayTag` 值类型和注册错误 |
| `gameplay_tags/bitset.rs` | 固定容量位集和继承位操作 |
| `gameplay_tags/registry.rs` | 名称注册、父标签递归注册和 `SystemParam` 注册入口 |
| `gameplay_tags/container.rs` | 每实体引用计数 Tag 状态 |
| `gameplay_tags/requirements.rs` | required/blocked/ignored 条件匹配 |
| `attributes/registry.rs` | Attribute ID、Region、Location 和注册表 |
| `attributes/aggregation.rs` | `Aggregator` 与 AttributeSet 内部稀疏聚合器集合 |
| `attributes/snapshot.rs` | 单属性与整套来源快照 |
| `attributes/attribute_set/state.rs` | AttributeSet 存储、dirty 位图和错误 |
| `attributes/attribute_set/mutation.rs` | 初始化、Instant/Duration 修改和来源清理 |
| `attributes/attribute_set/recalculation.rs` | 按需/批量重算与末尾系统 |
| `modifiers/definition.rs` | Modifier 操作、幅度和自定义计算 trait |
| `modifiers/context.rs` | 与具体 Effect runtime 无关的只读求值接口 |
| `modifiers/spec.rs` | 求值后的 Spec、Applied 值和中立来源 ID |

`modifiers` 可以引用 Attribute ID/快照和 Tag 容器作为求值契约，但不得依赖
`EffectContext`、`ActiveEffectHandle` 或 Active Effect 存储。Attributes 只保存
`ModifierSourceId`；Effect runtime 在跨领域边界完成 Handle 转换。

### Gameplay Effects

| 文件 | 主要所有权 |
| --- | --- |
| `effect_system_params.rs` | Effect 准备、执行、移除和收敛需要的窄 ECS 访问集合 |
| `gameplay_effect/definition.rs` | 不可变 Effect 定义、唯一 StackingPolicy 和 builder/getter |
| `gameplay_effect/context.rs` | Effect payload 与 Modifier 求值上下文适配 |
| `gameplay_effect/timing.rs` | Duration/Period 定义值 |
| `gameplay_effect/stacking.rs` | StackingPolicy 及子策略 |
| `gameplay_effect/effect_tags.rs` | Effect 标签、私有 source/target 条件对和公开链式 builder |
| `gameplay_effect_spec.rs` | 保留 definition `Arc`，捕获 Modifier/Duration/Period 求值结果；不复制 StackingPolicy |
| `active_gameplay_effect/state.rs` | Handle、稳定 slot、Active Effect 状态和目标 Component |
| `active_gameplay_effect/planning.rs` | 应用错误、Plan、prepare 和错误映射 |
| `active_gameplay_effect/application.rs` | 同步应用入口、条件、概率、免疫和堆叠选择 |
| `active_gameplay_effect/execution.rs` | Plan 重验证、Instant/Stack/Create 和回滚 |
| `active_gameplay_effect/modifiers.rs` | 即时/持续属性修改，以及叠层和到期减层共用的修饰器刷新 |
| `active_gameplay_effect/removal.rs` | 显式/按标签移除和 Effect 状态清理 |
| `active_gameplay_effect/requirements.rs` | ongoing/removal 条件和固定点收敛 |
| `active_gameplay_effect/ticking.rs` | Duration 与 Period fixed-tick 系统 |

Effect 公开 mutation API 接收 `EffectSystemParams`。该参数不包含 ASC、Active Ability 或
`Commands`；不要为了方便把 Effect API 再扩回完整 `AbilitySystemParams`。

### Abilities 与 Ability System

`gameplay_abilities` 拥有可共享定义和运行时数据类型；`ability_system` 拥有对这些类型执行激活、
commit 和生命周期编排的流程：

| 文件 | 主要所有权 |
| --- | --- |
| `gameplay_ability.rs` | AbilityTags、startup task、cost/cooldown/activation Effect 定义 |
| `gameplay_ability_spec.rs` | 授予 Handle、level 和 active count |
| `activation_data.rs` | 唯一组合 source、targets 与传播 context 的不可变激活值 |
| `ability_chain.rs`、`activation_context.rs` | 请求与运行实例共用的链保护、传播上下文，以及 Ability → Effect payload 转换 |
| `active_gameplay_ability/state.rs` | 只持有 spec handle、共享激活数据和状态的活跃实例 |
| `ability_task/context.rs` | 公开的 Task 共享 source/spec handle/level 轻量执行上下文；目标由父活跃实例持有 |
| `ability_task/definition.rs` | Instant/WaitTicks 定义和完成动作定义 |
| `ability_task/state.rs` | 运行时 Task Component 与 action-only 完成枚举 |
| `ability_task/completion.rs` | 使用轻量 context 与父 Active targets 向 Event/Effect/Ability 请求分派完成动作 |
| `ability_task/ticking.rs` | Task 稳定推进与清理 |
| `ability_system/component.rs` | ASC 规格存储和显式 `GameplayAbilitySystemBundle` |
| `ability_system/params.rs` | `AbilitySystemParams` 和同 batch pending overlay |
| `ability_system/activation/*` | 错误、预检、startup 创建和同步/batch 激活 |
| `ability_system/commit.rs` | cost/cooldown prepare、支付检查和执行 |
| `ability_system/lifecycle.rs` | end、cancel、回滚和 Cleanup system |
| `ability_input/bindings.rs` | 游戏逻辑动作到同实体 ASC 的技能 Handle 映射、稳定遍历和绑定错误 |

`AbilitySystemParams` 内嵌 `EffectSystemParams`，再增加 `Commands`、ASC、来源快照、活跃 Ability
查询和内部 pending overlay。Effect 实现不得反向导入 Ability System。

`ability_input.rs` 门面显式重导出 `ability_input/bindings.rs` 中的组件与错误类型，公开路径
仍为 `bevy_gas::gas::ability_input`；`bindings` 是私有实现模块。

`ability_input` 是独立的可选输入适配领域，只依赖 ASC 的只读规格查询和 `AbilitySpecHandle`。
它不采集物理设备、不保存按下状态、不安装系统，也不进入 `GameplayAbilitySystemBundle`。
输入生产系统由游戏配置，在 `RequestProducers` 将动作解析成 Handle 后写入统一队列；ASC
生命周期不反向查询输入组件。详细职责见 [18 — 技能输入绑定](./18-ability-input-bindings.md)。

### Targeting 与统一 Execution

| 文件 | 主要所有权 |
| --- | --- |
| `ability_target_data.rs` | 稳定排序的 Hit 与多目标结果 |
| `activation_targets.rs` | single/acquired 唯一激活目标值、空数据错误和统一目标遍历 |
| `targeting_definition.rs` | Targetable、合法操作管线和定义错误 |
| `acquisition.rs` | 同步候选查询、选择、过滤、排序和限制 |
| `targeting_queue/request.rs` | 请求输入、continuation、ID 和结果 Event |
| `targeting_queue/queue.rs` | FIFO Resource 与入队 API |
| `targeting_queue/processing.rs` | 整批抓取、抓取目标转换、continuation 便捷入队和结果通知 |
| `gameplay_execution/request.rs` | 只以 handle + activation data 保存 Ability 请求的具体请求类型、Effect 请求与统一枚举 |
| `gameplay_execution/queue.rs` | 跨类型 FIFO、便捷参数到 `AbilityActivationData` 的组装和 Ability chain ID 分配 |
| `gameplay_execution/resolver.rs` | 完整 drain、逐请求 Requirement 收敛和 System 包装 |

具体请求由 `gameplay_execution` 拥有。Ability System 和 Gameplay Effects 门面仅为兼容调用方
重导出各自请求类型，不再保存重复请求文件。

`AbilityActivationData` 由 `gameplay_abilities` 拥有，并由该领域门面、GAS 聚合门面和 crate root
显式重导出；它是组合型生命周期数据，不进入精简 prelude。Targeting 的激活 continuation 在
抓取前除 handle 外仍只保存 Context，获得非空 targets 后才把请求 source、targets 与 Context
组装成完整数据。

## ECS 组合与依赖规则

- `GameplayTagContainer`、`AttributeSet`、`AbilitySystemComponent` 均可独立存在；
- 完整 Gameplay Actor 使用 `GameplayAbilitySystemBundle` 显式组合 ASC、Tags、Attributes 和
  Active Effects；
- 低层 Component 不得通过 Required Component 反向安装高层 Effect runtime；
- Effect mutation 接收 `EffectSystemParams`，Ability 编排接收 `AbilitySystemParams`；
- Gameplay 系统通常生产请求，唯一 resolver 负责消费统一 FIFO；
- 多个请求生产系统如果业务顺序重要，必须使用 `.chain()` / `.before()` / `.after()` 显式排序；
- 定义对象使用 `Arc` 共享，目标持有的运行时状态仍保存在 ECS Component/实体中。

## 可见性与引用路径

从窄到宽选择可见性：

1. 默认私有：只在当前文件使用；
2. `pub(super)`：同一领域的父/兄弟模块需要；
3. `pub(crate)`：确有跨领域运行时调用；
4. `pub`：有稳定语义和 rustdoc 的用户 API。

同一领域内部优先 `use super::...`；跨领域使用 `use crate::...`。避免用多层
`super::super::...` 穿透领域，也避免从私有叶子文件路径导入以绕过门面。

## 测试布局

```text
tests/
├── gas_test.rs
├── gas_test/
│   ├── ability_input_test.rs
│   ├── support_test.rs
│   ├── attributes_test.rs
│   ├── gameplay_tags_test.rs
│   ├── gameplay_targeting_test.rs
│   ├── queues_test.rs
│   ├── runtime_paths_test.rs
│   ├── effects_test.rs
│   ├── effects_test/
│   │   ├── application_test.rs
│   │   ├── removal_test.rs
│   │   ├── requirements_test.rs
│   │   ├── stacking_test.rs
│   │   └── ticking_test.rs
│   ├── abilities_test.rs
│   └── abilities_test/
│       ├── activation_test.rs
│       ├── chaining_test.rs
│       ├── commit_test.rs
│       ├── lifecycle_test.rs
│       └── tasks_test.rs
├── randoms_test.rs
└── unique_names_test.rs
```

集成测试的顶层归属与 crate 顶层功能领域一致：`src/gas/` 内功能的集成测试统一放在
`tests/gas_test/`，由 `tests/gas_test.rs` 声明和加载；新增 GAS 子模块不新建顶层测试目标。
因此输入绑定测试使用 `tests/gas_test/ability_input_test.rs`。`randoms_test.rs` 和
`unique_names_test.rs` 对应 crate 顶层的独立领域，可以保留独立测试目标。

领域内部按外部行为拆分测试，不要求镜像私有源码文件，也不要求叶子测试文件递归套用门面结构。
具体命名和运行命令见 [13 — 测试指南](./13-testing-guide.md)。

`support_test.rs` 只保存 App/注册/builder/tick/query 等共享 fixture，不隐藏业务断言。需要 World、
调度顺序、公开 API 或跨模块可见性的场景使用集成测试；纯算法和私有不变量可以就近写单元测试。
`tests/` 下所有文件的文件名主干和所有子目录名统一以 `_test` 结尾；重命名时必须同步更新模块
声明、`#[path]`、导入路径、文档导航和 Cargo 集成测试目标名。

## 修改路由

下表的源码路径相对于 `src/gas/`，测试路径相对于 `tests/gas_test/`；crate 根门面另行标明。

| 修改目标 | 首选源码位置 | 首选测试 | 同步文档 |
| --- | --- | --- | --- |
| Tag 注册/匹配/引用计数 | `gameplay_tags/` | `gameplay_tags_test.rs` | 03 |
| Attribute 注册/存储/重算 | `attributes/` | `attributes_test.rs` | 04 |
| Modifier 求值/聚合顺序 | `modifiers/`、`attributes/aggregation.rs` | `attributes_test.rs`、`runtime_paths_test.rs` | 05 |
| Effect 应用/堆叠/条件/tick | `gameplay_effects/` | `effects_test/*` | 06 |
| Ability 定义/实例/task | `gameplay_abilities/` | `abilities_test/*` | 07、08 |
| ASC 激活/commit/lifecycle | `ability_system/` | `abilities_test/*` | 09 |
| 输入动作绑定、重绑和清理 | `ability_input/bindings.rs` | `ability_input_test.rs` | 18 |
| 目标管线与队列 | `gameplay_targeting/` | `gameplay_targeting_test.rs` | 15 |
| 统一 FIFO 与阶段可见性 | `gameplay_execution/`、`runtime_plugin.rs` | `queues_test.rs`、`runtime_paths_test.rs` | 02、16 |
| 公共导出/prelude | 各门面、`src/gas.rs`、`src/lib.rs`、`prelude.rs` | 全目标编译/rustdoc | 11、17 |

## 何时继续拆文件

新功能模块从一开始就使用门面和同名目录；本节讨论的是何时进一步拆分目录内的实现文件。
不以固定行数作为唯一标准。出现下列情况时优先拆分：

- 一个文件同时定义持久状态、配置、事务执行和调度系统；
- 校验、提交、回滚、清理需要独立推理；
- 修改一个局部策略必须阅读大量无关逻辑；
- 测试文件同时覆盖多个可以独立失败的行为轴。

不要为了“每个类型一个文件”制造过深目录；小型值类型和紧密相关的实现应留在同一职责文件中。
拆分时先保持行为和公共路径不变，再单独进行语义修改。

## 文档同步清单

源码结构或公共 API 变化后：

1. 更新领域门面 `//!` 与 rustdoc；
2. 更新本页的文件树、所有权或修改路由；
3. 更新对应领域文档中的行为和边界；
4. 只在 API 快速参考中列高频公开入口，避免复制整个 rustdoc；
5. 更新或新增能够编译的 example/集成测试；
6. 扫描旧文件名、旧签名、通配 `pub use *` 和断开的 Markdown 相对链接；
7. 执行：

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
cargo doc --no-deps --all-features
```
