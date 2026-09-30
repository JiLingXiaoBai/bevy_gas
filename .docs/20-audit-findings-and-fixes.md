# 20 — 审计发现与修改建议

> **文档性质：** 本文不是"当前源码行为"的描述（与 01–19 的定位不同），而是一次代码审计在
> **commit `c6700b8`（118 commits，工作区干净）** 上的问题快照。每条都给出可验证的证据、
> 定位和建议。**修完对应条目后请删除该条**；当本文全部条目清空或失效后，删除整份文档并在
> [README](./README.md) 目录中移除链接。不要把本文的结论当成长期契约。

## 摘要

| 项目     | 结果                                                             |
| -------- | ---------------------------------------------------------------- |
| 审计范围 | `src/` 全部 128 个文件、`tests/`、`examples/`、`.docs/`、构建配置 |
| 规模     | `src` 约 9.6k 行、测试约 8.2k 行                                 |
| 基线     | `cargo test --all-features`：181 passed / 0 failed / 1 ignored   |
| 基线     | `cargo clippy --all-targets --all-features -- -D warnings`：0 警告 |
| 基线     | `src/` 中 `unwrap/expect/panic!/unsafe/todo!`：0 处              |

结论：**架构与错误处理纪律良好，问题集中在"热路径未收敛"和"设计边界未定义"两类**，
没有发现会破坏常规玩法正确性的缺陷。已排除的疑点见 [第 5 节](#5-已排除的疑点，保留证据以免重复排查)。
## 1. 阅读须知：证据等级

每条建议标注证据等级，请在动手前按等级决定是否需要复验：

| 标记        | 含义                                                       |
| ----------- | ---------------------------------------------------------- |
| **[实测]**  | 我实际运行代码/命令得到了证据，含具体数值或输出             |
| **[读码]**  | 从源码逐行确认，未运行验证                                 |
| **[推理]**  | 有源码依据但结论依赖假设，动手前需要先构造验证             |

## 2. 问题汇总

| 编号 | 问题                                                       | 严重度 | 证据   | 类型       |
| ---- | ---------------------------------------------------------- | ------ | ------ | ---------- |
| F1   | 标签需求收敛每 tick 无条件全表扫描 4 次                    | 高     | [实测] | 性能       |
| F2   | 全局 `Random` 使概率效果的结果不确定                       | 高     | [读码] | 确定性     |
| F3   | `give_ability` 句柄静默回绕并产生别名                      | 中     | [读码] | 正确性     |
| F4   | `ActiveEffectHandle` 不含世界身份，跨 World 完全混叠       | 中     | [实测] | 正确性     |
| F5   | 缺少 CI 与基准，唯一测量用例被 `#[ignore]`                 | 中     | [实测] | 工程       |
| F6   | 公共 API 存在多条等价路径，文档与示例各教一套                  | 中     | [实测] | 可维护性   |
| F7   | prelude 缺少接入必需类型，示例被迫绕过 prelude             | 中     | [实测] | 可用性     |
| F8   | 概率掷骰早于判定，被拒绝的申请也消耗随机数列               | 中     | [读码] | 确定性     |
| F9   | `TargetingRequestQueue` 请求 ID 回绕复用                   | 中     | [读码] | 正确性     |
| F10  | 容量与热区划分全部为编译期常量，无调整出口                 | 中     | [读码] | 可配置性   |
| F11  | `AbilityChainContext` 深度上限两个谓词不一致               | 低     | [实测] | 正确性     |
| F12  | `ActiveGameplayEffects` 生成号饱和时槽位永久泄漏           | 低     | [读码] | 内存       |
| F13  | 收敛签名与转移历史无上限增长                               | 低     | [读码] | 内存/性能  |
| F14  | `cancel_active_abilities_with_tags` 存在冗余计算与重复查询 | 低     | [读码] | 性能       |
| F15  | 瞄准管线硬编码 `GlobalTransform` 与 `Targetable`           | 低     | [读码] | 扩展性     |
| F16  | `Changed<AttributeSet>` 过滤可能漏掉重算场景               | 低     | [推理] | 正确性     |
| F17  | 测试模块声明使用冗余 `#[path]`                             | 低     | [读码] | 整洁性     |
| F18  | `lib.rs` 巨型重导出与无意义的 `extern crate core;`         | 低     | [读码] | 整洁性     |
| F19  | tick 源硬编码为 `FixedUpdate`，无参数化入口                | 低     | [读码] | 扩展性     |

## 3. 高优先级

### F1 — 标签需求收敛每 tick 无条件全表扫描 4 次

**证据 [实测]。** `update_active_effect_tag_requirements_system`
（[requirements.rs:165](../src/gas/gameplay_effects/active_gameplay_effect/requirements.rs#L165)）
无条件调用 `resolve_active_effect_tag_requirements`，没有任何脏标记守卫；而它在
[runtime.rs](../src/gas/runtime_plugin/runtime.rs#L124) 中被注册了 **4 次**：

- `EffectTicks` 的链中第 1 次（[runtime.rs:125](../src/gas/runtime_plugin/runtime.rs#L125)）
- `EffectTicks` 的链中第 4 次（[runtime.rs:127](../src/gas/runtime_plugin/runtime.rs#L127)，与第 1 次是同一 tick 的重复调用）
- `PreGameplayConvergence`（[runtime.rs:144](../src/gas/runtime_plugin/runtime.rs#L144)）
- `UpdateEffectTagRequirements`（[runtime.rs:149](../src/gas/runtime_plugin/runtime.rs#L149)）

即使玩法状态完全静止，每 tick 仍要遍历所有带 `ActiveGameplayEffects` 的实体、构建并
`sort_unstable` 一个 `Vec<(u64, u32, u32, bool, bool, bool)>`（[requirements.rs:172](../src/gas/gameplay_effects/active_gameplay_effect/requirements.rs#L172)）。

运行仓库中被 `#[ignore]` 的测量用例（release）：

```text
cargo test --release --test gas_test -- --ignored --nocapture measure_requirement_convergence
effects=0     calls=64  elapsed_ms=0.017
effects=128   calls=64  elapsed_ms=0.349
effects=2048  calls=64  elapsed_ms=5.5    → 每趟约 86µs，单效果约 42ns
```

4 趟/tick 即约 0.34ms/tick（2048 个效果）。1 万效果量级会吃掉单核相当一块预算。

**建议：**

1. **先去重。** 确认 `EffectTicks` 链中的第 4 次调用是否必要。它与第 1 次同属一个 tick，
   中间只隔着 `tick_effect_period_system`；若引入它只是为了周期效果改变标签，应把需求收敛
   直接放进 `tick_effect_period_system` 的变更点，而不是整条链再跑一遍。
2. **给系统级注册加脏守卫。** `resolve_active_effect_tag_requirements_if_dirty`
   （[requirements.rs:158](../src/gas/gameplay_effects/active_gameplay_effect/requirements.rs#L158)）
   已经存在，但只在 resolver 内部路径被调用。建议为 `ActiveEffectRequirementSync` 增加只读
   脏查询，并提供 `run_if` 条件：

   ```rust
   // requirements.rs
   impl ActiveEffectRequirementSync {
       /// Returns whether pending changes still require a convergence pass.
       pub fn is_dirty(&self) -> bool { self.dirty }
   }

   /// Returns whether a convergence pass is required in this tick.
   pub fn active_effect_requirements_are_dirty(
       sync: Option<Res<ActiveEffectRequirementSync>>,
   ) -> bool {
       sync.is_some_and(|sync| sync.is_dirty())
   }
   ```

   把三处 `update_active_effect_tag_requirements_system` 注册改为
   `.run_if(active_effect_requirements_are_dirty)`。注意两处可见性变化：
   `ActiveEffectRequirementSync` 当前是 `pub(crate)`，而公开 `run_if` 的 `Res<T>` 参数会让 `T`
   出现在公开位置，因此它需要提升为 `pub`（`is_dirty` 可保持 `pub(crate)`，因为 `run_if`
   条件本身也定义在同一 crate 内）。若不愿公开该资源，替代做法是把脏标记换成
   `Res<EffectRequirementDirty>` 之类只含一个 `bool` 的公开薄包装。
3. **性能验收。** 改完必须用第 5 节的基准证明有效；本项是本审计中唯一有实测数据支撑的
   性能问题，也是唯一建议优先动手的性能改动。

**提交边界：** 去重与守卫是两个独立问题，按 [AGENTS.md](../AGENTS.md) 应分两次提交。

### F2 — 全局 `Random` 使概率效果的结果不确定

**证据 [读码]。** `EffectSystemParams` 暴露 `pub random_gen: ResMut<Random>`
（[effect_system_params.rs:17](../src/gas/gameplay_effects/effect_system_params.rs#L17)），
而 [`Random`](../src/randoms/random.rs#L10) 是全局 Resource。任何游戏系统都能取用，两个
各自掷骰的系统在 Bevy 中**只保证不并行，不保证相对顺序**；抽取顺序因此可能在不同运行间变化，
而抽取顺序直接决定 `probability < 1.0` 的效果是否命中。

受影响的是所有公开的未排序入口：`apply_gameplay_effect`、`execute_gameplay_effect_plan`、
`commit_ability`。只有经 FIFO 排空的
[`process_gameplay_execution_queue_system`](../src/gas/gameplay_execution/resolver.rs#L71)
路径顺序稳定。

这与 [01-overview.md:201](./01-overview.md#L201) 的"不依赖 Bevy 并行调度的偶然顺序"直接冲突；
[10-supporting-infrastructure.md:78](./10-supporting-infrastructure.md#L78) 承认了这一点并把
责任推给调用方，但 `random_gen` 的字段文档仍称其为"确定性随机源"。

**建议（按代价从低到高）：**

1. **立即：** 把 `random_gen` 的字段文档改为显式风险提示，说明"仅在使用统一 FIFO 入口时
   顺序确定；未排序的生产者系统必须以 SystemSet 固定顺序，或改用独立种子的随机源"。
   `Random` 的 `random_range`/`random_bool` 文档也应补同样的前提。
2. **中期：** 让随机流按来源派生，而不是共享一条流。可选做法：以
   `(source entity, effect spec handle, activation handle)` 的稳定元组派生每 tick 的子种子，
   或为每个 gameplay 实体挂一条独立的 RNG 状态 Component。
3. **验证方法：** 写一个测试，两个系统在**同一 system set 内无顺序约束**分别掷骰，断言结果
   与顺序无关。当前实现应当失败；修好后应通过。

### F3 — `give_ability` 句柄静默回绕并产生别名

**证据 [读码]。** [component.rs:41](../src/gas/ability_system/component.rs#L41)：

```rust
let handle = AbilitySpecHandle::new(self.next_ability_handle);
self.next_ability_handle = self.next_ability_handle.wrapping_add(1);
```

第 2³² 次授予后句柄重复，`ability_indices.insert` 覆盖旧条目（
[component.rs:46](../src/gas/ability_system/component.rs#L46)），于是 `find_ability_spec(旧句柄)`
返回**另一个技能**；一次 `clear_ability(旧句柄)` 因 `retain` 会**同时删除两条规格**
（[component.rs:59](../src/gas/ability_system/component.rs#L59)）。没有 `Err`、没有断言、没有测试。

这与本库其它三处的处理方式不一致：`GameplayExecutionQueue` 用 `checked_add` + 返回错误
（[queue.rs:79](../src/gas/gameplay_execution/queue.rs#L79)），`UniqueNamePool` 用 `try_from` +
返回错误（[unique_name.rs:106](../src/unique_names/unique_name.rs#L106)），
`ActiveGameplayEffects` 至少在生成号上限处停止分配（
[state.rs:225](../src/gas/gameplay_effects/active_gameplay_effect/state.rs#L225)）。

**建议：**

1. 为 `give_ability` 增加返回类型：`Result<AbilitySpecHandle, AbilityGrantError>`，用
   `checked_add`；`AbilityGrantError::HandleExhausted { max: u32 }`。这是**公共 API 变更**，
   按 AGENTS.md 需要单独提交并在 `.docs/07` 与 `.docs/11` 记录。
2. 若不希望改签名，至少加 `debug_assert!` 并在 `insert` 前检测碰撞（HashMap 返回
   `Some(旧值)` 即碰撞），release 下记录 `error!`。**不推荐**只加断言，因为外部输入导致
   的状态损坏不应依赖断言（AGENTS.md 错误处理原则）。
3. 补测试：句柄回绕边界（可把起点设到 `u32::MAX - 1` 后用公开构造器制造，避免真的循环 2³² 次）。

### F4 — `ActiveEffectHandle` 不含世界身份，跨 World 完全混叠

**证据 [实测]。** `ActiveEffectStorageRegistry` 是 **per-World Resource**，`Default = Some(1)`
（[lifecycle.rs:158](../src/gas/gameplay_effects/active_gameplay_effect/lifecycle.rs#L158)），
而句柄匹配只看 `(target, storage_id, slot, generation)`
（[state.rs:113](../src/gas/gameplay_effects/active_gameplay_effect/state.rs#L113)）。
实测两个独立 World 的第一个持续效果得到**完全相同**的身份：

```text
world A handles: [ActiveEffectHandle { target: 26v0, storage_id: 1, slot: 0, generation: 1 }]
world B handles: [ActiveEffectHandle { target: 26v0, storage_id: 1, slot: 0, generation: 1 }]
```

后果：把 A 世界的句柄连同 B 世界的 `params` 传给 `remove_active_effect`
（[removal.rs](../src/gas/gameplay_effects/active_gameplay_effect/removal.rs#L20)），
会静默修改 B 世界并返回"成功"，而该函数的文档承诺陈旧句柄返回 `Ok(false)`。

同一 World 内不受影响（`storage_id` 单调递增，实体重生后不会复用身份）。

**建议：**

1. 把 `ActiveEffectStorageRegistry.next_id` 换成进程级 `AtomicU64`（`Ordering::Relaxed` 足够，
   只需要唯一性）。这样 `storage_id` 全局唯一，跨 World 与跨测试都不再混叠，改动很小。
2. 若认为 per-World 是有意设计，则至少要在 `ActiveEffectHandle` 文档中写明"句柄只在创建它的
   World 内有效，跨 World 传递属于未定义行为"，并让 `remove_active_effect` 文档不再承诺
   陈旧句柄一定返回 `Ok(false)`。
3. 补测试：两个 App 各建一个效果，断言两次 `get_storage_id()` 不相等。

## 4. 中低优先级

### F5 — 缺少 CI 与基准，唯一测量用例被 `#[ignore]`

**证据 [实测]。** 仓库无 `.github/`、无任何 CI 配置、无 `benches/`；
[requirements_test.rs:822](../tests/gas_test/effects_test/requirements_test.rs#L822) 的
`measure_requirement_convergence` 带 `#[ignore = "manual requirement-convergence measurement"]`，
是唯一的性能测量。AGENTS.md 要求的 `fmt / clippy / test / build` 四步完全依赖人脑执行。

**建议：**

1. 增加最小 CI：`cargo fmt --check`、`cargo clippy --all-targets --all-features -- -D warnings`、
   `cargo test --all-features`、`cargo build`。前三步在本机已全部通过，可直接作为门禁。
2. 把 `measure_requirement_convergence` 改为 `benches/` 下的基准（criterion 会引入新依赖，
   若不希望新增依赖，可用 `--release` 下带 `Instant` 输出的测试并纳入 CI 的门槛断言）。
   **没有第 1 项基准，F1 的优化无法证明有效**——这是建议先做 F5 再动 F1 的原因。
3. 基准至少覆盖三个规模（0 / 128 / 2048 效果），与现有测量用例保持一致。

### F6 — 公共 API 存在多条等价路径

**证据 [实测]。** 以下三行同时编译通过（已用临时示例验证）：

```rust
bevy_gas::AttributeIdManager                    // crate root 重导出
bevy_gas::attributes::AttributeIdManager        // gas 域模块（gas.rs 的 pub mod）
bevy_gas::gas::attributes::AttributeIdManager   // 嵌套路径
```

而三处"教材"各用一条：[12-usage-patterns.md:13](./12-usage-patterns.md#L13) 用
`bevy_gas::gas::...`，[support_test.rs:3](../tests/gas_test/support_test.rs#L3) 用
`bevy_gas::attributes::...`，[ability_effect_flow.rs:10](../examples/ability_effect_flow.rs#L10)
用 `bevy_gas::gas::attributes::...`。

[README.md:117](./README.md#L117) 定义了三层约定（prelude / `gas::<domain>` / crate-root 兼容），
但 `src/gas.rs` 的 `pub mod` 又让第二、三层产生第四种拼写，实际是四条路径。

**建议：**

1. 选定 **`bevy_gas::gas::<domain>` 为唯一领域路径**，把 `src/gas.rs` 中除 `gas::<domain>` 之外
   的 `pub mod` 与重复 `pub use` 收敛掉；crate root 保留为显式兼容层并标注弃用计划。
2. 统一三处教材到同一路径。这是**纯文档/导入路径改动**，不要与功能修改混在同一次提交。
3. 该改动会让用户代码出现编译错误——属于破坏性变更，需要 CHANGELOG 或迁移说明。若暂无
   破坏性变更的意愿，则退一步：**只统一文档与示例**，并在这两处之外的路径上补
   `#[doc(hidden)]`，让 rustdoc 只展示推荐路径。

### F7 — prelude 缺少接入必需类型

**证据 [实测]。** [ability_effect_flow.rs:10](../examples/ability_effect_flow.rs#L10) 必须写
`bevy_gas::gas::attributes::AttributeIdManager`；`AbilityTaskDef`、`AbilityTaskOnFinishedDef`
（定义技能行为的核心类型）也不在 [prelude.rs](../src/gas/prelude.rs) 中。

**建议：** 按示例的实际需要补入 prelude，至少包括 `AttributeIdManager` 与 `AbilityTaskDef`、
`AbilityTaskOnFinishedDef`。prelude 的既定边界是"Plugin、核心 Component、常用定义与
SystemParam"，上述类型都属于"常用定义"，加入不违背该边界。改完让示例直接
`use bevy_gas::prelude::*;` 就能编译，这是最好的验收标准。

### F8 — 概率掷骰早于判定

**证据 [读码]。** [planning.rs:293](../src/gas/gameplay_effects/active_gameplay_effect/planning.rs#L293)：

```rust
if probability < 1.0 && !params.random_gen.random_bool(probability) {
    return Err(GameplayEffectApplicationError::ProbabilityRejected);
}
```

掷骰发生在标签需求（`passes_application_requirements`）、免疫
（`is_blocked_by_application_immunity`）、堆叠等判定**之前**，因此一次注定被拒绝的申请也会
消耗抽数，推移后续所有抽取。这放大了 F2：结果不仅依赖顺序，还依赖被拒绝申请的数量。

**建议：** 把掷骰移到确定性判定之后、`make_spec` 之前。这样被拒绝的申请不再扰动随机流，
是低成本、低风险的改善。注意保持"概率校验早于掷骰"的现有顺序
（[planning.rs:294](../src/gas/gameplay_effects/active_gameplay_effect/planning.rs#L294) 的
`InvalidProbability` 检查），否则会改变现有测试的预期。

### F9 — `TargetingRequestQueue` 请求 ID 回绕复用

**证据 [读码]。** [queue.rs:35](../src/gas/gameplay_targeting/targeting_queue/queue.rs#L35)：

```rust
self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
```

与 `GameplayExecutionQueue` 的 `checked_add` + 返回 `RequestIdExhausted` 相反。ID 复用后，
用 `TargetingRequestId` 关联 `TargetingResultEvent` 的代码会张冠李戴。

**建议：** 统一为 `checked_add`，并在耗尽时返回错误（需要 `push_request` 改签名）。若认为
回绕在实际运行时长内不可达，则至少要标注文档并加 `debug_assert!`；但两个同类队列对同一个
问题给出相反答案本身就是需要消除的不一致。

### F10 — 容量与热区划分无调整出口

**证据 [读码]。** [settings.rs](../src/gas/settings.rs) 的 5 个常量全是 `const`，
`AttributeSet` 由 `[Option<Attribute>; 32]` + `Box<[Option<Attribute>; 224]>` 组成
（约 1.9KB/实体）。接入方想按项目规模调整只能改源码。

[10-supporting-infrastructure.md:103](./10-supporting-infrastructure.md#L103) 已说明"不是运行时
配置，也不应在不同客户端之间出现不一致"——该理由成立，但当前没有中间选项。

**建议（择一，不要都做）：**

1. 保持现状，但在 [settings.rs](../src/gas/settings.rs) 补文档说明"调整需重新编译并全客户端
   一致，且会改变固定数组与位集布局"，让代价显式。
2. 若确有适配需求，把属性容量做成 `AttributeSet<const HOT: usize, const COLD: usize>` 或
   提供两个 feature 组合（小/大），而不是运行时配置。这会显著增加泛型复杂度，**只有在确实
   撞到上限时才值得做**。

### F11 — `AbilityChainContext` 深度上限两个谓词不一致

**证据 [实测]。** [ability_chain.rs:76](../src/gas/gameplay_abilities/ability_chain.rs#L76) 用
`depth >= MAX_DEPTH` 拒绝，[ability_chain.rs:122](../src/gas/gameplay_abilities/ability_chain.rs#L122)
用 `depth > MAX_DEPTH` 拒绝。实测：

```text
next() rejected edge 9: DepthExceeded { max_depth: 8 }
accepted edges=8, resulting depth accepted by validate=Ok(())
```

即同一个不变量有两个阈值，`ABILITY_CHAIN_MAX_DEPTH = 8` 实际允许 9 层技能。

**建议：** 抽出一个私有谓词（如 `fn depth_is_exceeded(depth: u8) -> bool`）供两处共用，然后
明确语义：若"最大嵌套数 8"指**层数**，`next()` 应在 `depth + 1 >= MAX_DEPTH` 时拒绝；若指
**边数**，则应把常量语义写清并让 `validate_for_handle` 用同一个谓词。无论选哪个，都要在
`.docs/07` 说明"深度"数的是层还是边。

### F12 — 生成号饱和时槽位永久泄漏

**证据 [读码]。** [state.rs:225](../src/gas/gameplay_effects/active_gameplay_effect/state.rs#L225)：

```rust
if slot.generation < u32::MAX {
    slot.generation += 1;
    self.free_slots.push(handle.slot);
}
```

生成号到 `u32::MAX` 后槽位既不递增也不回收，且不返回错误，与"超出容量返回 `Err`"的原则不符
（对比 `insert` 的 `CapacityExceeded`）。实际不可达。

**建议：** 二选一——在 `insert` 分配时若遇到饱和槽位返回新的
`ActiveEffectStorageError::GenerationExhausted { target, slot }`；或在 `remove` 中
`debug_assert!` 并记录 `error!`。前者与项目原则一致，代价是新增一个错误变体。

### F13 — 收敛签名与转移历史无上限增长

**证据 [读码]。** [requirements.rs:42](../src/gas/gameplay_effects/active_gameplay_effect/requirements.rs#L42)
的 `seen_states: Vec<...>` 与 `transitions: Vec<...>` 每轮追加；每轮还会把 handle 的 `Vec`
克隆进历史（[requirements.rs:56](../src/gas/gameplay_effects/active_gameplay_effect/requirements.rs#L56)）。
需求字段反复翻转的效果越多，内存与比较成本越高。

**建议：** 为 `seen_states` 设一个与 `ABILITY_CHAIN_MAX_DEPTH` 同级的显式上限常量（例如
`EFFECT_REQUIREMENT_MAX_PASSES`），达到上限即按现有 fail-closed 路径处理并 `error!` 记录。
这与 [16-gameplay-execution.md:402](./16-gameplay-execution.md#L402) 的"新请求类型必须证明
派生过程有限"是同一个原则，只是用在了收敛循环上。

### F14 — `cancel_active_abilities_with_tags` 冗余计算与重复查询

**证据 [读码]。** [transitions.rs:116](../src/gas/ability_system/lifecycle/transitions.rs#L116)
在循环内对每个匹配实例调用 `tag_bits_from_tags_with_manager` 并**丢弃结果**（只为校验标签）；
[transitions.rs:135](../src/gas/ability_system/lifecycle/transitions.rs#L135) 在取消循环里对每个
实例重复 `asc_query.get_mut(source)`。

**建议：** 把校验提到循环外做一次；把 `asc_query.get_mut(source)` 的在循环外的可变借用与该
循环内对 `params.pending_active_abilities` 的访问拆成两段，避免逐次查询。属于局部优化，
建议在 F1 之后按 profiling 结果决定是否要做。

### F15 — 瞄准管线硬编码 `GlobalTransform` 与 `Targetable`

**证据 [读码]。** [acquisition.rs:14](../src/gas/gameplay_targeting/acquisition.rs#L14) 的
`TargetingCandidateQuery` 固定为 `(Entity, &GlobalTransform, Option<&GameplayTagContainer>,
Option<&AttributeSet>, Option<&Targetable>)`。格子/六边形/自研物理的游戏必须给每个候选实体挂
`GlobalTransform` 并自己维护标记才能复用该管线。

**建议：** 记录为已知边界即可，不建议现在抽象化（会引入泛型参数并扩散到
`TargetingRequestQueue`）。若将来有第二种空间表示，再考虑把"候选收集"拆成一个可由游戏提供的
`SystemParam`，保持 `acquire_targets` 的排序与过滤逻辑不变。

### F16 — `Changed<AttributeSet>` 过滤可能漏掉重算场景

**证据 [推理]。** [recalculation.rs:115](../src/gas/attributes/attribute_set/recalculation.rs#L115)
用 `Query<&mut AttributeSet, Changed<AttributeSet>>`；
[recalculation.rs:26](../src/gas/attributes/attribute_set/recalculation.rs#L26) 的
`recalculate_dirty` 是公开的，任何系统都可以先清掉脏位，使后续 `Changed` 触发时无事可做。
另外 `AttributeIdRegister::request_or_register_attribute_id` 修改的是 Resource，
`initialize_attribute` 中的 `aggregators.remove(location)`
（[mutation.rs:27](../src/gas/attributes/attribute_set/mutation.rs#L27)）不会让任何实体的
`AttributeSet` 变"changed"。

**先说清楚这不是缺陷：** `base` 立即更新、`current` 惰性重算，而
`get_current_value` 会先重算再返回
（[recalculation.rs:59](../src/gas/attributes/attribute_set/recalculation.rs#L59)），
所以游玩逻辑读到的值始终正确；[16-gameplay-execution.md:325](./16-gameplay-execution.md#L325)
也把该行为写成了契约。**只有"直接读字段而不走读取 API"外加"运行时重新注册属性"的组合可能陈旧**，
我没有构造出可达路径。

**建议：** 先构造一个测试确认可达性（运行时注册新属性 + 保留旧效果 + 只读 `current`），
不可达就不要改。若可达，最小修法是让 `AttributeIdManager` 的变更通过一个
`AttributeRegistryVersion` 资源参与 `run_if`，而不是放宽 `Changed` 过滤。

### F17 — 测试模块声明使用冗余 `#[path]`

**证据 [读码]。** [gas_test.rs](../tests/gas_test.rs) 的 9 处 `#[path = "gas_test/xxx.rs"]` 全部
冗余：`tests/gas_test.rs` + `tests/gas_test/` 目录本就满足 Rust 模块解析规则，`mod x_test;`
即可。删除后行为不变（`tests/gas_test.rs` 声明的子模块默认在 `tests/gas_test/` 下查找）。

**建议：** 删除全部 `#[path]` 属性，跑一次 `cargo test` 验证。注意保留
`tests/gas_test/abilities_test.rs`、`effects_test.rs` 等中间门面文件的层级，本项只删属性。

### F18 — `lib.rs` 巨型重导出与无意义的 `extern crate core;`

**证据 [读码]。** [lib.rs:11](../src/lib.rs#L11) 的 `pub use gas::{...}` 列出 150+ 个类型名并
混杂模块名（`settings`、`prelude`、`modifiers`），末尾还有 `extern crate core;`
（[lib.rs:60](../src/lib.rs#L60)，edition 2024 下无意义）。这个列表让"公共 API 有多宽"无人可审。

**建议：** 与 F6 一起处理。删除 `extern crate core;` 是零风险改动，可单独提交。重导出列表
建议改为逐域 `pub use gas::<domain>::{...}` 或整体保留但加文档说明其为兼容层、新代码不应使用。

### F19 — tick 源硬编码为 `FixedUpdate`

**证据 [读码]。** 所有系统与 SystemSet 都直接注册到 `FixedUpdate`
（[runtime.rs](../src/gas/runtime_plugin/runtime.rs#L100)）。想做确定性回放（自己驱动 tick）只能
像 [ability_effect_flow.rs:77](../examples/ability_effect_flow.rs#L77) 那样手动
`run_schedule(FixedUpdate)`。

**建议：** 记录为已知边界。若将来需要，可提供
`GameplayAbilitySystemRuntimePlugin::in_schedule(ScheduleLabel)` 形式的入口，把现有注册代码
参数化。当前不必要，不要为此提前抽象。

## 5. 已排除的疑点（保留证据，以免重复排查）

### E1 — 标签引用计数在父子标签间"顺序相关"？**不成立**

一次对抗式审查提出：`GameplayTagContainer` 在"父标签显式引用已耗尽 + 多个子标签仍存活"时
会丢失父标签位，并在重复移除时 `u16` 下溢，debug 下触发
[container.rs:85](../src/gas/gameplay_tags/container.rs#L85) 的 `debug_assert!` panic，
release 下回绕成永久幽灵标签。

我用三个独立构造（含该审查给出的**精确算术轨迹**）复现，全部显示行为正确：

```text
after adds:        a=true  b=true  c=true
after remove(a):   a=true  b=true  c=true     ← 预测"a 存活"，一致
after remove(b):   a=true  b=false c=true     ← 预测"a 存活"，一致
after remove(c):   a=false b=false c=false    ← 预测"a 的位被错误清除"，实际正确清除
after re-add(b):   a=true  b=true             ← 父标签正确重建
```

原因：该审查算错了一步。`remove(A)` 在 `A.explicit > 0` 时是**空操作**，不会递减任何计数
（[container.rs:77](../src/gas/gameplay_tags/container.rs#L77) 的提前返回）；`add` 把自身算进
`total`（[container.rs:53](../src/gas/gameplay_tags/container.rs#L53)）而 `remove` 只在显式
引用 >0 时才递减自身，这个不对称是正确的。因此预测的下溢路径**不可达**，
`debug_assert!` 不会被触发。

**结论：`GameplayTagContainer` 的父子引用计数逻辑正确，无需修改。** 若将来改动该文件，
请把上述"父引用耗尽 + 多子存活 + 重加子标签"序列固化成一个回归测试
（可作为 [tests/gas_test/gameplay_tags_test.rs](../tests/gas_test/gameplay_tags_test.rs) 的用例），
因为这是最容易改坏的地方。

### E2 — `give_ability` 句柄别名是否只是理论问题？**不是，见 F3**

该审查补充了一个我先前遗漏的后果：一次 `clear_ability(句柄)` 会因 `retain` 同时删除两条规格。
已并入 F3。

### E3 — 收敛环检测的 fail-closed 强删是否过激？**保持现状**

[requirements.rs:64](../src/gas/gameplay_effects/active_gameplay_effect/requirements.rs#L64) 在检测到
非收敛环时移除参与效果，这是
[16-gameplay-execution.md:322](./16-gameplay-execution.md#L322) 明确规定的确定性 fail-closed
行为。该审查推测"合法两步链可能被误判为环"，但未给出可达构造。**不建议在没有反例前改动**；
若要收紧，应先用 F13 的轮次上限把"环"与"抖动"区分开。

## 6. 建议的执行顺序

1. **F5**（CI + 基准）——没有基准就无法验证 F1，且能防止后续改动引入回归。
2. **F1**（收敛去重 + 脏守卫）——唯一有实测数据支撑的性能问题，改完用 F5 的基准验收。
3. **F8**（掷骰时机）→ **F2**（随机流隔离）——低风险改善在前，架构性改动在后。
4. **F3 / F4 / F9 / F11 / F12**——批量的一处一提交，都是"边界未定义"，互不冲突。
5. **F6 / F7 / F17 / F18**——纯 API 路径与整洁性，统一到同一路径，合成一次破坏性变更提交。
6. **F10 / F15 / F19**——只在文档中记录边界，不动代码。
7. **F13 / F14 / F16**——按 profiling 与可达性验证结果决定，可能全部不做。

**每次提交后按 [AGENTS.md](../AGENTS.md) 更新受影响的 `.docs/` 条目，并删除本文对应行。**
