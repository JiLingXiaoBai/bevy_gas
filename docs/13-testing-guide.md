# 13 — 测试指南

## 测试组织

```
tests/
├── gas_tests.rs                         # GAS 集成测试 crate 门面
├── gas_tests/
│   ├── support.rs                       # App、builder、tick 和查询 helper
│   ├── effects.rs                       # Effect 测试门面
│   ├── effects/
│   │   ├── application.rs               # 应用校验、错误和回滚
│   │   ├── stacking.rs                  # 堆叠、上限和刷新策略
│   │   ├── requirements.rs              # 免疫、抑制和条件收敛
│   │   ├── ticking.rs                   # Duration 与 Period tick
│   │   └── removal.rs                   # 显式与标签驱动移除
│   ├── abilities.rs                     # Ability 测试门面
│   ├── abilities/
│   │   ├── activation.rs                # 激活条件和实例策略
│   │   ├── commit.rs                    # Cost 与 Cooldown
│   │   ├── lifecycle.rs                 # 取消、结束和清理
│   │   ├── tasks.rs                     # WaitTicks 与任务结束
│   │   └── chaining.rs                  # 链式上下文和深度限制
│   ├── gameplay_tags_test.rs            # 标签注册、位集、容器
│   ├── attributes_test.rs               # 属性初始化、重算、聚合器
│   ├── gameplay_targeting_test.rs       # 目标管线、确定性、技能集成
│   ├── queues_test.rs                   # 队列处理、运行条件
│   └── runtime_paths_test.rs            # 端到端运行时路径
├── randoms_tests.rs                     # RNG 确定性
└── unique_names_tests.rs                # 字符串驻留、冲突检测
```

Effect 与 Ability 测试按行为拆分，而不是按实现文件逐一镜像。这样移动私有函数不会引起测试
目录抖动；新增行为时应选择它验证的外部语义，例如 Requirement 收敛测试放在
`effects/requirements.rs`。

## 运行测试

```bash
# 运行所有测试
cargo test

# 运行特定测试模块
cargo test --test gas_tests

# 只运行 Effect Requirement 测试
cargo test --test gas_tests effects::requirements

# 带输出运行
cargo test -- --nocapture

# 运行特定测试
cargo test test_name
```

## 提交前检查清单

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
```

所有代码应无编译警告，并通过所有测试。

运行时代码按照项目约定不使用 `unwrap()`、`expect()` 和 `panic!()`。测试中的
`unwrap()` / `expect()` 可作为“此步骤必须成功”的显式断言使用；生产代码必须通过
`Result`、`?`、`match` 或 `let ... else` 处理失败路径。

## 编写新测试

### 测试结构

测试使用 `#[cfg(test)] mod tests { ... }` 放在各文件中，或作为独立测试文件放在 `tests/` 目录下。

### 通用测试模式

```rust
use bevy::app::App;
use bevy_tools::prelude::*;

#[test]
fn test_my_feature() {
    let mut app = App::new();
    app.add_plugins(GameplayAbilitySystemPlugin);

    // 在 Startup 系统中注册标签、属性等
    // 完整 GAS Actor 使用 GameplayAbilitySystemBundle；纯领域测试只生成所需 Component
    // 应用效果或激活技能
    // 推进 FixedUpdate tick
    // 断言预期状态
}
```

### 推进时间

由于所有计时基于 tick，推进 `FixedUpdate` 来推进时间：

```rust
// 推进 N 个 tick
for _ in 0..n_ticks {
    app.world_mut().run_schedule(FixedUpdate);
}
```

不要在测试中继续依赖插入 `AttributeSet` 或 `GameplayTagContainer` 时隐式产生
`ActiveGameplayEffects`。需要持续/无限效果存储的 fixture 应显式加入该 Component，或直接使用
`GameplayAbilitySystemBundle`；纯 Tags/Attributes 测试应验证它们能够独立存在。

集成测试直接运行 `FixedUpdate` schedule，从而保证每次循环恰好推进一个 Gameplay tick；
`app.update()` 会受 Bevy 固定时间累积影响，不适合断言精确 tick 边界。

### 关键测试领域

| 领域       | 应测试的内容                                 |
| ---------- | -------------------------------------------- |
| 标签注册   | 父标签自动注册、容量限制                     |
| 标签容器   | 添加/移除、引用计数、has_tag/has_all/has_any |
| 属性初始化 | 基础值、重算、聚合器                        |
| 修饰器聚合 | 顺序：Override → Add → PercentAdd → Multiply |
| 即时效果   | Base 值修改、post_execute 回调               |
| 持续效果   | 修饰器应用、过期清理                         |
| 周期效果   | Tick 计数、execute_on_applied                |
| 堆叠       | 堆叠上限、幅度缩放、持续时间刷新             |
| 抑制       | 持续标签要求、修饰器/标签恢复                |
| 免疫       | 阻止应用、免疫查询匹配                       |
| 技能激活   | 冷却、消耗、阻止标签、激活要求               |
| 技能任务   | WaitTicks 倒计时、on_finished 动作           |
| 技能链式   | 深度限制、循环检测                           |
| 目标抓取   | 管线验证、过滤、排序、锥形、批量请求、多目标效果 |
| 队列       | Push/pop、FIFO、单 tick 全量消费、运行条件       |
| 清理       | Ending/Cancelled 技能销毁、索引清理          |
