# 20 — 审计复核与后续项

> 原审计以 commit `c6700b8` 为快照。复核后，已移除误报和不适合直接执行的修改建议，
> 不再把原来的严重度、测量数据和执行顺序作为当前结论。
> 本页只保留需要后续决策的观察与维护边界；实际行为以 rustdoc、领域文档和回归测试为准。
> 后续项解决后删除对应条目；全部关闭后可删除本页，并更新 [知识库目录](./README.md)。

## 待需求或测量支撑的后续项

### F8 — 被拒绝的应用可能消耗随机数

[prepare_gameplay_effect](../src/gas/gameplay_effects/active_gameplay_effect/planning.rs)
先验证概率范围并掷骰，再检查标签、免疫、规格与堆叠等条件。因此，某些最终无法应用的请求也会
消耗随机数，后续结果依赖完整请求序列。这是可观察的顺序语义，不能仅凭这一点判断确定性失效：
固定种子、初始状态和请求顺序仍应产生相同结果。

**当前不改判定顺序。** 如果玩法需要“确定性条件拒绝时不抽数”，先明确错误优先级、规格求值
与抽数的边界，再补覆盖随机数序列和拒绝原因的回归测试。堆叠等检查依赖已构造的规格，不能
简单把抽数移动到 `make_spec` 之前便声称所有拒绝均不消耗随机数。

### F9 — Targeting 请求 ID 在 `u64` 耗尽后回绕

[TargetingRequestQueue::push_request](../src/gas/gameplay_targeting/targeting_queue/queue.rs)
使用 `wrapping_add(1).max(1)`，ID 在耗尽后复用；与统一执行队列的可返回错误策略不同。
这一边界真实存在，但容量是 `u64`，不应与 `u32` 技能句柄耗尽列为同等优先级。

**低优先级保留。** 如果需要统一所有 ID 的耗尽策略，应单独变更其返回类型，补边界测试和调用方
迁移说明。不要以 `debug_assert!` 或日志代替对外可处理的错误。

### F13 — 复杂标签依赖的收敛成本需要有代表性的场景

[Requirement 收敛](../src/gas/gameplay_effects/active_gameplay_effect/requirements.rs)
保存每轮状态签名和转移历史，成本随活跃效果数与收敛轮数增加；重复状态已有环检测和确定性的
fail-closed 处理。这是待评估的最坏情况成本，不能直接表述为已复现的无界内存泄漏。

**先测量再决定。** 若实际配置出现长链或较大的历史开销，先构造代表性场景并证明允许的边界。
不要直接增加任意轮数上限后删除效果：合法的长依赖链也可能被截断，这会改变 Gameplay 结果。

### F14 — 取消技能路径有局部优化空间

[cancel_active_abilities_with_tags](../src/gas/ability_system/lifecycle/transitions.rs)
会重复构造匹配位集，并在取消实例时重复访问 ASC。可以在 profiling 证明重要后缓存请求标签的
位集，评估减少重复查询。

循环中校验的是每个匹配技能各自的 `block_abilities_with_tags`，内容可能不同，不能直接提到
循环外只校验一次。现有流程先完成所有相关校验，再开始取消；优化时应保留这个错误处理边界。

## 已明确的设计边界

以下是接入与维护约束，不作为本轮新增架构工作的依据：

| 原编号 | 复核结论与依据 |
| --- | --- |
| F2 | 共享 RNG 的复现要求种子、初始状态与抽取顺序一致。FIFO 只保持实际入队顺序，生产者仍须显式排序；见 [10 — 支撑基础设施](./10-supporting-infrastructure.md)。没有实际需求前不改为按实体派生随机流。 |
| F4 | Active Effect 句柄属于创建它的 World；同一 World 内的陈旧句柄保护不等于跨 World 唯一性。接入方不得混用不同 World 的句柄；见 [06 — 效果](./06-gameplay-effects.md)。不引入影响分配顺序的进程全局计数器。 |
| F6/F7/F18 | 领域路径、GAS 聚合门面和 crate-root 兼容重导出属于既定公开 API；prelude 有意保持精简。新用法优先推荐路径，不为减少导入拼写破坏兼容；见 [17 — 源码布局](./17-source-layout-and-maintenance.md)。 |
| F10 | 容量与冷热划分是编译期布局，调整需重新编译并保持客户端一致。存在实际容量需求后再讨论配置方式；见 [10 — 支撑基础设施](./10-supporting-infrastructure.md)。 |
| F15 | 内建空间抓取使用 `GlobalTransform`，大部分 selection 要求 `Targetable`；游戏仍可自行构造 `AbilityTargetData` 后提交激活。无需为假设的第二种空间表示提前抽象；见 [15 — 目标抓取](./15-gameplay-targeting.md)。 |
| F19 | Runtime Plugin 注册到 `FixedUpdate`，调用方可以显式运行该 schedule 推进 tick。自定义 schedule 参数化需要实际用例；见 [02 — 插件系统](./02-plugins-and-lifecycle.md)。 |

## 后续修改的验证要求

- 正确性修改先给出可达路径与回归测试，区分实际缺陷、契约选择和理论耗尽边界。
- 性能修改比较相同机器、构建配置和场景下的工作量与耗时；保持既有阶段可见性和结算结果。
- 手动性能测量保留 `#[ignore]`，不添加依赖 CI 机器负载的固定耗时阈值。运行方式与 CI 检查见
  [13 — 测试指南](./13-testing-guide.md)。
- 公共 API 变化同步更新领域文档、示例、测试与配套游戏调用方；不要把无关清理混入修复。
