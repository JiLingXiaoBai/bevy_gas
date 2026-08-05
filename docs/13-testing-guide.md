# 13 — 测试指南

## 测试组织

```
tests/
├── gas_tests/
│   ├── common_test.rs                   # 共享测试工具
│   ├── gameplay_tags_test.rs            # 标签注册、位集、容器
│   ├── attributes_test.rs               # 属性初始化、重算、聚合器
│   ├── gameplay_effects_test.rs         # 效果应用、堆叠、抑制
│   ├── gameplay_abilities_test.rs       # 技能激活、任务、链式
│   ├── gameplay_targeting_test.rs       # 目标管线、确定性、技能集成
│   ├── active_gameplay_effect_test.rs   # 活跃效果生命周期
│   ├── queues_test.rs                   # 队列处理、运行条件
│   └── runtime_paths_test.rs            # 端到端运行时路径
├── randoms_tests.rs                     # RNG 确定性
└── unique_names_tests.rs                # 字符串驻留、冲突检测
```

## 运行测试

```bash
# 运行所有测试
cargo test

# 运行特定测试模块
cargo test --test gas_tests

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
use bevy_tools::*;

#[test]
fn test_my_feature() {
    let mut app = App::new();
    app.add_plugins(GameplayAbilitySystemPlugin);

    // 在 Startup 系统中注册标签、属性等
    // 生成带 ASC、AttributeSet、GameplayTagContainer 的实体
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
