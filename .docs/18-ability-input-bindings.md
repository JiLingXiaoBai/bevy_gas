# 18 — 输入绑定迁移与游戏输入边界

## 当前所有权

逻辑动作与技能槽映射已迁移至游戏模板。GAS 不再包含 `ability_input` 模块，
也不从 prelude、GAS 聚合门面或 crate root 重导出输入绑定类型。
完整行为、输入缓冲、重绑与生命周期说明见
[模板 — 输入绑定与空间随机](../../bevy_gas_template/.docs/06-input-and-spatial-random.md)。

| 旧入口 | 新入口 |
| --- | --- |
| GAS `AbilityInputBindings<Action>` | `bevy_gas_template::gameplay::ability_input::AbilityInputBindings<Action>` |
| GAS `AbilityInputBindingError` | `bevy_gas_template::gameplay::ability_input::AbilityInputBindingError` |
| `examples/ability_input_bindings.rs` | 模板 `examples/ability_input_bindings.rs` |
| `tests/gas_test/ability_input_test.rs` | 模板 `tests/gameplay_test/ability_input_test.rs` |

名称、错误变体和行为保持不变；旧路径没有兼容重导出。调用方显式导入模板类型：

```rust
use bevy_gas_template::gameplay::ability_input::{
    AbilityInputBindingError, AbilityInputBindings,
};
```

输入绑定仍是可选 Component，不进入 `GameplayAbilitySystemBundle`，也不自动安装系统。
组件应与拥有对应技能的 ASC 位于同一实体；移除技能后清理该 Handle 的绑定，替换 ASC 时同步
清空或重建绑定。游戏也可直接使用自己的动作映射，不必使用模板组件。

## GAS 保留的契约

- `give_ability(ability, level)` 返回 `AbilitySpecHandle`，规格不持有输入编号或按下状态。
- 游戏将动作解析为 Handle，组装目标及 `AbilityActivationContext::input(source, chain)`。
- 请求生产系统在 `GameplayAbilitySystemSet::RequestProducers` 写入统一执行队列。
- GAS resolver 验证技能要求、成本和冷却，再执行技能；不读取键盘、技能栏或输入缓冲。
- `AbilityActivationReason::Input` 不携带动作编号；动作与设备语义仍由游戏维护。

绑定的 `resolve()` 只检查指定 ASC 中的规格是否存在，无法识别其他 ASC 的同值 Handle，
也不替代激活校验。重绑不会改写已入队请求，取消和输入释放策略由游戏显式实现。

## 验证

在模板仓库根目录运行：

```bash
cargo run --example ability_input_bindings
cargo test --test gameplay_test ability_input_test
```

GAS 保留技能授予、激活、队列和生命周期测试；输入与缓冲测试由模板维护。
空间随机的同时迁移见 [10 — 支撑基础设施](./10-supporting-infrastructure.md#随机数)。
