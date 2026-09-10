# 20 — Excel 技能配置与 GAS 接入

## 范围与目录

首版支持标签、属性、效果、修改器、技能、动作时间线和目标规则七张表。
配置使用固定 Luban 5.0.0 的 `rust-bin + bin + --strict`，经安全读取和业务编译后形成
`GameplayCatalog` Resource。配置接入位于同一个 `bevy_gas` 包的 `config` 模块，
由默认关闭的 `luban-config` feature 启用；GAS 领域代码不依赖生成的配置类型。
仓库只维护根 `Cargo.toml`，生成代码作为 Rust 模块参与编译。

| 路径 | 职责 |
| --- | --- |
| `config/tables/gas.*.xlsx` | 策划维护的数据、Luban 字段类型与说明 |
| `config/defines/gas.xml` | 七张 GAS 表的登记与普通枚举定义 |
| `config/defines/builtin.xml` | Luban 内置定义 |
| `config/templates/rust-bin/` | 项目维护的安全 Rust 模板 |
| `src/config.rs`、`src/config/` | 配置门面，以及解码、校验、GAS 编译、加载和包校验实现 |
| `src/config/decoding/` | 仅使用标准库的 Safe Rust、Result 二进制解码实现 |
| `config/generated/` | 自动生成的 `mod.rs`、`gas.rs` 等 Rust 模块，包含 DTO、表索引与结构描述 |
| `config/bin/` | 导出配置包，包括七张 GAS 表的 bytes 与 manifest.json；Git 忽略 |
| `src/bin/gas_config.rs` | 启用 `luban-config` 后可用的配置预览与导表校验 CLI |
| `examples/config_fireball.rs` | 启用 `luban-config` 后可用的无窗口火球示例 |
| `tools/luban/.cache/export/` | 每次导表的临时单包项目、验证产物与发布备份 |

完整配置包包含上述七张 GAS 表的七个数据文件和 `manifest.json`。
`src/config.rs` 通过外部路径加载 `config/generated/mod.rs`，公开为
`bevy_gas::config::generated`；生成目录不维护独立 Cargo 包。

## 日常使用

在仓库根目录运行：

```powershell
pwsh -NoProfile -File tools/luban/setup.ps1
pwsh -NoProfile -File config/export.ps1
cargo run --features luban-config --bin gas-config -- inspect config/bin 1001 3
cargo run --features luban-config --example config_fireball -- config/bin
```

首次导表需要已安装的 Rust 工具链及依赖缓存。导表使用锁文件和离线构建；
缺少依赖时先执行 `cargo build --features luban-config` 准备依赖，再重新导表。

导表入口先在独立暂存目录生成代码和数据，仅提取 Rust 模块作为待发布代码。
脚本复制根包的 `src/`、`examples/`、`Cargo.toml`、`Cargo.lock` 和候选生成模块，构建启用
`luban-config` 的临时单包项目；新 CLI 生成配置包清单，再实际读取整个包并在
无窗口 Bevy App 中编译 GAS 定义。
全部成功后才发布生成代码和数据目录；发布失败尝试恢复上一份目录。
GAS 转换或 Rust 编译失败也会使导表返回非零退出码。
发布过程通过排他锁防止并发导表，逐文件核对发布前后摘要，并在失败时尝试回滚。
两个目录的替换不是对并发读取者的瞬时切换，导表期间不要启动配置加载。

不要将 Luban MCP 的原始 generate 输出直接作为可发布包。MCP 的 get_schema 和 validate
适合查询表结构、定位 Excel 问题；完整发布入口始终为 export.ps1。
Luban 的结构校验和 Rust 的玩法校验是连续两层，不能互相替代。

## Excel 表与引用

| 表 | 主键 | 内容 |
| --- | --- | --- |
| gas.TbTag | name | 完整标签名及说明 |
| gas.TbAttribute | name | 属性名、Hot/Cold 区域及说明 |
| gas.TbEffect | id | 持续、周期、概率、asset/granted 标签 |
| gas.TbModifier | id | effect_id、order、属性、操作、Flat/LinearLevel 参数 |
| gas.TbAbility | id | 等级、标签、消耗、冷却、立即效果、目标规则、实例策略 |
| gas.TbAbilityAction | id | ability_id、at_tick、order、动作、目标范围、effect_id |
| gas.TbTargeting | id | 选择、标签/距离过滤、排序与数量上限 |

表名、主键、输入文件和枚举统一维护在 `config/defines/gas.xml`。
表字段仍从数据 Excel 表头读取；因此修改技能数值只改数据表，新增字段修改相应表头，
新增表或枚举修改 XML 定义。新增数据文件的路径相对于 `config/tables/`。
无需额外维护定义 Excel；未独立定义的行 Bean 由 Luban 根据表头生成。

第一行 `##var` 为字段名，`##type` 为类型，`##` 为说明；数据行 A 列留空。
普通整数、浮点和布尔值使用 Excel 原生值。可空字段留空，不填字符串 null；
集合使用表头指定的分号分隔，不自行更换分隔符。

示例类型：

- 必须存在的效果引用：`int#ref=gas.TbEffect`。
- 可选效果引用：`int?#ref=gas.TbEffect`。
- 标签列表：`(list#sep=;),(string#ref=gas.TbTag)`。
- 属性引用：`string#ref=gas.TbAttribute`。

标签与属性表是配置的权威名单。效果和技能引用的父标签也需在 TbTag 明确登记，
否则 ref 校验会拒绝；运行时登记子标签时会自动建立祖先。
容量统计包含全部祖先，当前上限为 512 个标签、32 个 Hot 属性和 224 个 Cold 属性。
TbAttribute 定义属性身份与存储区域，不定义角色当前数值；角色创建时仍需初始化属性。

SelectionKind 使用 `SelfTarget`，Excel 别名可以填写 Self，避免 Rust 的 Self 关键字冲突。

## Fireball 样例

- Ability 1001：Fireball，最大等级 5，成本 2001，冷却 2002，目标规则 3001。
- Effect 2001：Instant，Mana Add -20。
- Effect 2002：DurationTicks 180，无修改器，授予 Cooldown.Fireball。
- Effect 2003：Instant，Health Add LinearLevel，base=-100、per_level=-20。
- Targeting 3001：显式实体 → 排除来源 → 要求 AttributeSet → 最大距离 20 → 保留一个目标。
- Action 10011：at_tick=12、order=1，向主目标施加 2003。
- Action 10012：at_tick=12、order=2，EndAbility。
- 激活阻止标签 State.Stunned；end_on_activation=false，不允许同规格多实例。

第五级示例从 Health=500、Mana=100 开始，成功激活后 Mana=80，十二次后续
AbilityTasks 推进后目标 Health=320，技能在同一 tick 结束；冷却仍按自身生命周期保留。
示例不创建弹体，也不执行动画。时间只承诺 tick，秒数取决于游戏设置的 Time<Fixed>。

## 数值、时间与动作

Flat 使用 base，per_level 必须为零。LinearLevel 使用
`base + per_level * (level - 1)`，等级从 1 开始。
编译器、预览与运行时计算器共用求值规则，验证允许等级范围内的结果可表示为有限 f32。
适配器授予技能时检查等级；独立使用导出的 Effect 定义时，调用者仍须遵循其等级契约。

首版效果统一 non_stacking。Instant 不允许周期或保留 granted tags；
DurationTicks 必须提供正整数持续时间，Infinite 不填写 duration_ticks。
period_ticks 留空表示无周期，有值时必须为正；execute_on_applied 只用于周期效果。

成本必须 Instant、Add、概率 1、非周期，每个属性只能有一条修改器，且全等级成本为负。
不消耗资源使用空 cost_effect_id。冷却必须正持续时间、非空 granted tags、概率 1、
无周期且无属性修改器。冷却检查使用 granted tags，不使用 asset tags。

动作按 `(at_tick, order)` 排序，三元组 `(ability_id, at_tick, order)` 必须唯一。
同 tick 动作编译为一个 `AbilityTaskOnFinishedDef::Batch`，只创建一个等待任务。
at_tick=0 使用 Instant；正值使用 WaitTicks，所有时间都相对于同一次激活。

有限技能二选一：

1. end_on_activation=true，只配置即时动作，不再写 EndAbility；
2. end_on_activation=false，恰好一条 EndAbility，且必须为最后一个有序动作。

Batch 按定义顺序派发，遇到第一个 EndAbility 后停止，已入队效果继续结算。
它不提供事务回滚，不会让普通 sibling WaitTicks 自动变成串行任务，也不会让
EmitEvent Observer 在 startup resolver 内同步回写。完整边界见 [08 — 技能任务](./08-ability-tasks.md)。

ApplyEffect 的 Primary/AllCaptured 分别使用激活时捕获的主目标/全部目标。
命中时重新抓取、独立 Self 动作、地面点空目标、弹体和跨技能配置动作尚未开放。
真实弹体可由游戏层通过现有 EmitEvent、Targeting 和统一效果队列扩展。

## 加载、注册与授予

启用 `luban-config` 后，从 `bevy_gas::config` 导入配置 API。
`load_tables(directory)` 先校验 manifest，再将同一次读取并验过摘要的 bytes 交给
`generated::Tables::new`。不在校验后重新读取文件，不提供忽略包校验的默认路径。

`validate_tables(&tables)` 检查名称、引用、未使用参数、成本/冷却、动作顺序与公式。
`compile_catalog(&tables, &mut world)` 复用这些检查，再登记名称并构建 Arc 定义。
World 应先安装 GameplayAbilitySystemPlugin。

```rust
use bevy_gas::config::{compile_catalog, load_tables};

let tables = load_tables(data_directory)?;
let catalog = compile_catalog(&tables, app.world_mut())?;
app.world_mut().insert_resource(catalog);
```

配置语义校验在注册前完成；名称注册采用追加语义。
同名现存标签必须具有与配置一致的完整继承位图，同名属性必须属于一致的 Hot/Cold 区域。
如果现存注册表容量不足，已成功登记的名字可能保留，但不会自动发布不完整 catalog。
调用方只在 compile_catalog 成功后插入 Resource。首版在启动加载，不替换战斗中的 catalog。

- 按名字排序登记 Tag/Attribute，祖先先于子标签，避免依赖 HashMap 或 Excel 行顺序。
- 固定注册顺序只保证相同配置的重复结果，不保证不同配置版本的内部编号相同。
- 按 EffectId 创建唯一 Arc，成本、冷却、动作和立即效果复用它。
- 不按内容去重两个不同 EffectId；内容相同不代表叠层身份相同。
- 存档保存稳定配置 ID/名字，不保存 GameplayTag 位编号、AttributeId 或 AbilitySpecHandle。
- `grant_ability` 将共享定义授予 ASC，并写入同一角色的 ConfiguredAbilities。
- `revoke_ability` 成对清除规格和映射；活动中的技能拒绝撤销并保留映射，已由底层清除的
  陈旧映射可以清理。需要再次授予时，先使用该接口完成撤销。
- 输入系统通过 ConfiguredAbilities 查 Handle，通过 CompiledAbility 的 targeting 发起目标请求，
  continuation 再进入统一 GameplayExecutionQueue。

成本与冷却在激活时提交；技能结束不会自动移除已应用效果。取消后自动退款和延迟 Commit
不属于本配置层的行为。

## 包版本与生成模板

manifest.json 包含格式/模板版本、结构摘要、整包内容摘要与每个文件的大小及摘要。
结构描述包含生成字段顺序、字段类型、普通枚举判别值、表主键及输出名称；
因此字段顺序或枚举编码变化不能被旧生成代码悄悄接受。
数据内容摘要标识具体配置版本，普通数值变动允许由兼容的同一份读取代码加载。

BLAKE3 与 serde_json 复用现有 Bevy 依赖树，分别承担标准内容摘要和清单 JSON 解析，
由 `luban-config` feature 启用；二进制解码模块本身只使用 Rust 标准库。

读取限制为单文件 64 MiB、整包 256 MiB、清单 1 MiB；二进制集合和字符串另有上限。
清单必须包含恰好全部预期表，拒绝重复/未知路径、结构不兼容和摘要不一致。
内容摘要用于一致性检查，不承担发布者签名认证。

模板支持基础类型、Option、Vec、普通枚举和非继承 map 表。
所有 Decode 返回 Result，非法枚举、截断、长度错误、重复主键、尾随字节均返回错误。
不调用上游旧的不可失败 deserialize helper，不使用 flags、抽象 Bean 或 dyn Any。
变更模板后重新导表，禁止手改生成 Rust 来修复当前结果。

## 验证与维护

所有源码属于根 `bevy_gas` 包。常规检查在仓库根目录执行：

```powershell
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
```

`cargo test` 运行现有测试；默认构建不启用配置模块，`--all-features` 检查会包含它。
配置变更使用完整导表验证候选 Rust 模块、真实二进制包和 GAS 语义，再运行上面的
配置预览与火球示例检查实际行为。修改 Excel、schema 或模板后，必须重新导表并同步提交
源文件和生成 Rust 模块；不手动修改 manifest 来掩盖结构或数据变化。
根目录 `target/` 是可删除的构建缓存，后续 Cargo 命令会重新创建。

有关工具安装和 MCP 的细节见 [19 — Luban 工具链](./19-luban-toolchain.md)。
