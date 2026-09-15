# 20 — Excel 技能配置与 GAS 接入

## 范围与目录

首版支持标签、属性、效果、修改器、技能、动作时间线和目标规则七张表。
配置使用固定 Luban 5.0.0 的 `rust-bin + bin + --strict`，经安全读取和业务编译后形成
`GameplayCatalog` Resource。配置接入位于同一个 `bevy_gas` 包的 `config` 模块，
运行时接口和生成代码始终参与编译；GAS 领域代码不依赖生成的配置类型。
默认关闭的 `config-validation` feature 启用配置包校验、完整业务校验和离线工具，仓库只维护根 `Cargo.toml`。

| 路径                           | 职责                                                                   |
| ------------------------------ | ---------------------------------------------------------------------- |
| `config/tables/gas.*.xlsx`     | 策划维护的数据、Luban 字段类型与说明                                   |
| `config/defines/gas.xml`       | 七张 GAS 表的登记与普通枚举定义                                        |
| `config/defines/builtin.xml`   | Luban 内置定义                                                         |
| `config/templates/rust-bin/`   | 项目维护的安全 Rust 模板                                               |
| `src/config.rs`、`src/config/` | 配置门面，以及解码、校验、GAS 编译、加载和包校验实现                   |
| `src/config/decoding/`         | 仅使用标准库的 Safe Rust、Result 二进制解码实现                        |
| `config/generated/`            | 自动生成的 `mod.rs`、`gas.rs` 等 Rust 模块，包含 DTO、表索引与结构描述 |
| `config/bin/`                  | 导出配置包，包括七张 GAS 表的 bytes 与 manifest.json；Git 忽略         |
| `src/bin/gas_config.rs`        | 启用 `config-validation` 后可用的配置预览与导表校验 CLI                |
| `examples/config_fireball.rs`  | 默认可编译运行的无窗口火球示例                                         |
| `tools/luban/.cache/export/`   | 每次导表的临时单包项目、验证产物与发布备份                             |

导表产物包含上述七张 GAS 表的七个 `.bytes` 文件和 `manifest.json`。
默认运行时只需部署七个 `.bytes` 文件；启用 `config-validation` 的加载器还要求匹配的清单。
`src/config.rs` 通过外部路径加载 `config/generated/mod.rs`，公开为
`bevy_gas::config::generated`；生成目录不维护独立 Cargo 包。

加载与预览分属 `loading` 和 `inspection`：前者负责读包解码，后者仅在 `config-validation`
启用时执行完整校验和文本报告。`ConfigError`、`ConfigErrorKind`、`ConfigLocation`
由配置领域共用的 `error` 模块拥有。编译器按准备、注册、效果、目标、技能时间线拆分：
`PreparedTables` 借用生成行，统一解析实际使用的动作和幅度参数，建立已排序的技能动作、
效果修改器索引；编译、完整校验和预览共用这一视图，不重复扫描和解释同一关系。
生成 DTO 与 GAS 运行时类型仍然分离，feature 行为与配置协议保持不变。文件归属见
[17 — 源码布局与维护边界](./17-source-layout-and-maintenance.md#配置模块内部职责)。

## Feature 边界

`default = []` 保持不变。`config-validation` 不控制配置读取、编译和授予能力，也不控制生成模块的可见性。

| 能力                                                                                                                            | 默认构建             | 启用 `config-validation`                         |
| ------------------------------------------------------------------------------------------------------------------------------- | -------------------- | ------------------------------------------------ |
| 生成 DTO、`load_tables`、`read_package`、`compile_catalog`、技能授予与撤销                                                      | 可用                 | 可用                                             |
| manifest、schema、文件与整包摘要校验                                                                                            | 不编译，不读取清单   | 必须通过校验                                     |
| 文件与解码大小限制，非法枚举、重复主键及二进制合法性                                                                            | 始终检查             | 始终检查                                         |
| 构建所需引用、运行时注册表容量与状态、基本数值合法性                                                                            | 始终检查             | 始终检查                                         |
| 命名规范、未使用字段、跨表业务约束、时间线、成本与冷却策略                                                                      | 跳过完整业务校验     | `compile_catalog` 在注册前对准备结果执行完整校验 |
| `package_schema_hash`、`MAX_MANIFEST_BYTES`、`validate_tables`、`describe_ability*`、`write_package_manifest`、`gas-config` CLI | 不编译               | 可用                                             |
| `config_fireball` 示例                                                                                                          | 可用，直接读取表数据 | 可用，并执行包校验和完整业务校验                 |

默认加载器按生成的 `TABLE_FILES` 直接读取 `.bytes`，不要求 `manifest.json`，已有清单即使无效也会忽略。
I/O、分配边界、解码和必要构建错误仍返回 `Result`。发布前的导表流程始终启用 `config-validation`，
完成清单、结构、摘要与业务校验后才发布；默认运行时加载这批已验证的表数据。

基本数值检查包含正数最大等级、有限且位于 `[0, 1]` 的应用概率、实际使用的有限幅度参数，
以及正数持续时间/周期和转换为运行时 f32 tick 时的无损性。使用的球形/锥形半径与最大距离
也必须具有有限平方。最大等级 100、tick 上限 1,000,000 等制作限制属于完整业务校验。

在 Zed 中打开仓库根目录时，`src/config/` 和 `config/generated/` 已属于默认 Cargo 模块图，
rust-analyzer 无需额外启用 feature 即可分析运行时接口的定义与引用。
需要编辑或导航完整校验和离线工具代码时，仍需让 rust-analyzer 启用 `config-validation`。

## 日常使用

在仓库根目录运行：

```powershell
pwsh -NoProfile -File tools/luban/setup.ps1
pwsh -NoProfile -File config/export.ps1
cargo run --features config-validation --bin gas-config -- inspect config/bin 1001 3
cargo run --example config_fireball -- config/bin
```

首次导表需要已安装的 Rust 工具链及依赖缓存。导表使用锁文件和离线构建；
缺少依赖时先执行 `cargo build --features config-validation` 准备依赖，再重新导表。

导表入口先在独立暂存目录生成代码和数据，仅提取 Rust 模块作为待发布代码。
脚本复制根包的 `src/`、`examples/`、`Cargo.toml`、`Cargo.lock` 和候选生成模块，构建启用
`config-validation` 的临时单包项目；新 CLI 生成配置包清单，再实际读取整个包并在
无窗口 Bevy App 中编译 GAS 定义。
全部成功后才发布生成代码和数据目录；发布失败尝试恢复上一份目录。
GAS 转换或 Rust 编译失败也会使导表返回非零退出码。
发布过程通过排他锁防止并发导表，逐文件核对发布前后摘要，并在失败时尝试回滚。
两个目录的替换不是对并发读取者的瞬时切换，导表期间不要启动配置加载。

不要将 Luban MCP 的原始 generate 输出直接作为可发布包。MCP 的 get_schema 和 validate
适合查询表结构、定位 Excel 问题；完整发布入口始终为 export.ps1。
Luban 的结构校验和 Rust 的玩法校验是连续两层，不能互相替代。

## Excel 表与引用

| 表                  | 主键 | 内容                                                  |
| ------------------- | ---- | ----------------------------------------------------- |
| gas.TbTag           | name | 完整标签名及说明                                      |
| gas.TbAttribute     | name | 属性名、Hot/Cold 区域及说明                           |
| gas.TbEffect        | id   | 持续、周期、概率、asset/granted 标签                  |
| gas.TbModifier      | id   | effect_id、order、属性、操作、Flat/LinearLevel 参数   |
| gas.TbAbility       | id   | 等级、标签、消耗、冷却、立即效果、目标规则、实例策略  |
| gas.TbAbilityTask   | id   | ability_id、at_tick、order、动作、目标范围、effect_id |
| gas.TbTargeting     | id   | 选择、标签/距离过滤、排序与数量上限                   |

技能动作时间线的源文件为 `config/tables/gas.ability_task.xlsx`，在 XML 中登记为 `gas.TbAbilityTask`。
生成的 `config::generated::gas::AbilityTask` 表示一行配置，每行描述一个动作，由编译器按 tick 分组并合并为任务定义。
它与运行时的 `AbilityTask` Component 分属配置数据和 ECS 任务状态两个层次。

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

以下规则描述配置制作约定，由启用 `config-validation` 的完整业务校验执行；
默认构建仍检查构建定义所需的基本数值与引用合法性，但不重复执行全部策略检查。

Flat 使用 base，per_level 必须为零。LinearLevel 使用
`base + per_level * (level - 1)`，等级从 1 开始。
编译器、预览与运行时计算器共用求值规则；完整业务校验检查允许等级范围内的结果
可表示为有限 f32。默认构建检查实际使用的 base 与 LinearLevel 的 per_level 有限，
不检查 Flat 未使用的 per_level，也不预演全部等级；动态公式溢出时沿用计算器返回零的行为。
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

直接从 `bevy_gas::config` 导入运行时配置 API，无需启用 feature。
`load_tables(directory)` 始终通过 `read_package` 读取表数据，再交给 `generated::Tables::new`。
默认构建使用标准库按 `TABLE_FILES` 读取受大小限制的 `.bytes`，不解析清单、计算摘要或检查 schema。
启用 `config-validation` 时，先检查 manifest、schema、大小和摘要，再将同一次读取并验证的 bytes
交给解码器，不在校验后重新读取文件。

启用 `config-validation` 后，`validate_tables(&tables)` 检查名称、引用、未使用参数、
成本/冷却、动作顺序与公式；`compile_catalog(&tables, &mut world)` 在注册前使用相同校验，
直接复用已经完成的 `PreparedTables`，不重复准备。默认构建跳过完整制作规则，
两种构建均保留动作载荷、实际使用的幅度参数、所需引用和运行时注册状态检查。
未被任何现存技能或效果使用的孤立动作/修改器仍由完整制作校验拒绝；默认构建不使用它们。
World 应先安装 GameplayAbilitySystemPlugin。

```rust
use bevy_gas::config::{compile_catalog, load_tables};

let tables = load_tables(data_directory)?;
let catalog = compile_catalog(&tables, app.world_mut())?;
app.world_mut().insert_resource(catalog);
```

`compile_catalog` 返回 `Err` 时，World 中的名称池、标签表、属性表及已有 catalog 均保持原状。
实现先克隆三个注册表，在私有快照上完成名称登记与全部 Arc 定义构建，成功后才统一提交
三个 Resource；准备期间不会从 World 移除资源。快照同时承担注册预检和最终提交内容，
因此不需要维护两套容量/冲突判断，也没有失败回滚分支。

同名现存标签必须具有与配置一致的完整继承位图，同名属性必须属于一致的 Hot/Cold 区域。
容量不足、继承冲突、区域冲突或后期构建错误均丢弃快照，调用方可修正配置或加载备用配置，
失败尝试不会消耗后续名称与内部 ID。成功注册仍保留已有条目并追加新名称。

快照会复制启动时的注册表数据；这是为失败原子性付出的冷路径成本，不进入战斗热路径。
调用方只在 compile_catalog 成功后插入 catalog Resource。首版仍只支持启动加载，不替换战斗中的 catalog。

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

## 结构化错误

所有配置层失败使用 `ConfigError`：`kind()` 返回可匹配的 `ConfigErrorKind`，
`location()` 返回 `ConfigLocation`，`message()` 和 `Display` 提供可读说明。
旧的字符串 `context()` 已移除，调用方不应解析文案来决定恢复流程。

| 类别 | 含义 |
| --- | --- |
| `Io`、`Capacity`、`Decode`、`Package` | 文件读写、边界限制、二进制解码、清单/schema/摘要或生成表契约 |
| `Validation`、`Reference`、`InvalidValue` | 制作规则、构建所需引用、运行时不可表示的值 |
| `MissingResource`、`Registration` | ECS 注册表缺失、名称/继承/区域注册失败 |
| `UnknownAbility`、`UnsupportedLevel`、`AlreadyGranted`、`ActiveAbility` | 查询/授予/撤销时可分别处理的调用状态 |
| `Report` | 文本报告格式化失败 |

位置用枚举明确区分文件、表、Resource 和操作。表位置包含表名、可选的行 ID/稳定名、
字段路径；文件位置包含完整读取路径、可选字段路径，以及解码失败的字节偏移。
例如加载失败可匹配 `ConfigLocation::File` 并定位实际出错的 `gas_tb*.bytes`，
不再只得到目录；校验等级可匹配 `ConfigLocation::Table` 的 Ability 行和 level 字段。
完整校验仍可在编译前拒绝制作规则；运行时所需检查不会依赖 feature 是否启用。

## 包版本与生成模板

导表生成的 manifest.json 包含格式/模板版本、结构摘要、整包内容摘要与每个文件的大小及摘要。
结构描述包含生成字段顺序、字段类型、普通枚举判别值、表主键及输出名称；
启用 `config-validation` 的加载器据此拒绝结构不兼容的数据。默认加载器不执行这一兼容性检查，
部署时应配套使用同次导表生成的 Rust 模块和表数据。
数据内容摘要标识具体配置版本，普通数值变动允许由兼容的同一份读取代码加载；包协议和模板不因 feature 改变。

BLAKE3 与 serde_json 是由 `config-validation` 启用的可选直接依赖，分别负责摘要与清单 JSON。
关闭 feature 时，本库不编译相关哈希、清单解析和校验代码；二进制读取与解码使用标准库。
Bevy 的默认 feature 可能独立引入同名传递依赖，因此整个依赖树中仍可能出现这些 crate。

两种构建均限制单文件 64 MiB、表数据合计 256 MiB；二进制集合和字符串另有上限。
启用 `config-validation` 时，另限制清单 1 MiB，要求恰好包含全部预期表，
拒绝重复/未知路径、结构不兼容和摘要不一致。内容摘要用于一致性检查，不承担发布者签名认证。

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
cargo test --all-features
cargo build
```

`cargo test` 包含默认运行时配置测试；`cargo test --all-features` 同时验证包校验和完整业务校验，
两种构建均通过 `tests/config_test.rs` 加载 `tests/config_test/` 中的测试。
这些测试自建表数据和临时配置包，不依赖导表生成的 `config/bin/`。
配置变更使用完整导表验证候选 Rust 模块、真实二进制包和 GAS 语义，再运行上面的
配置预览与火球示例检查实际行为。修改 Excel、schema 或模板后，必须重新导表并同步提交
源文件和生成 Rust 模块；不手动修改 manifest 来掩盖结构或数据变化。
根目录 `target/` 是可删除的构建缓存，后续 Cargo 命令会重新创建。

有关工具安装和 MCP 的细节见 [19 — Luban 工具链](./19-luban-toolchain.md)。

配置编译回归测试位于 `tests/config_test/compilation_test.rs`：覆盖已初始化注册表冲突后的
资源保持与重试、后期构建失败不影响已有 catalog、缺失 Resource 分类，以及输入行序变化时
编译任务与预览时间线顺序一致。默认构建和启用 feature 的构建均需执行这些外部行为测试。
