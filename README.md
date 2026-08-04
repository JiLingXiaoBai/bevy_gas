# bevy_tools

`bevy_tools` 是一个基于 Bevy ECS 的轻量 Gameplay Ability System 实验项目。当前重点是搭建一套可组合的战斗/技能运行时，包括 GameplayTag、Attribute、GameplayEffect、GameplayAbility、AbilityTask 和 AbilitySystemComponent。

项目依赖：

- Rust edition 2024
- Bevy `0.19.0`
- rand `0.10.2`

## 核心目标

这个项目试图把常见 GAS 流程拆成几个清晰层级：

- `GameplayTag`：描述状态、分类、阻挡、免疫、需求等标签语义
- `AttributeSet`：保存生命、攻击力、资源等属性，并支持 modifier 聚合
- `GameplayEffect`：描述属性修改、持续时间、周期效果、堆叠、标签授予和免疫
- `GameplayAbility`：描述技能定义、消耗、冷却、启动任务和标签规则
- `AbilityTask`：承担技能执行行为，例如等待、触发事件、应用 effect
- `AbilitySystemComponent`：挂在角色实体上，管理技能授予、激活和生命周期

项目整体思路是：Ability 负责启动和组织行为，Effect 负责真正修改属性或授予状态，Task 负责把行为拆成可 tick、可取消、可扩展的执行单元。

## 快速检查

常用检查命令：

```bash
cargo fmt
cargo clippy
cargo test
cargo build
```

## TODO

- 目标抓取流程
- 玩家通过输入激活技能流程
- 序列化与反序列化
- 技能消耗支持非 ModifierOperation::Add 的其他消耗类型
