# 19 — Luban 配置工程与工具链

## 当前范围

项目在 `config/` 维护 XML 定义与 Excel 数据，在 `tools/luban/` 固定 Luban、Luban.Agent 与 Luban.Mcp 工具链。
日常通过 `config/export.ps1` 将配置导出为 Rust 代码和二进制数据；工具缓存与配置源文件分开存放。
Codex 通过 Luban.Mcp 查询表结构、校验和生成配置；Luban.Agent 保留为 MCP 的查询与校验后端。
当前配置包含七张 GAS 表，提供真实 Excel 火球配置包。
Safe Rust 解码和 GAS 适配代码位于根 `bevy_gas` 包的 `config` 模块，通过默认关闭的
`luban-config` feature 启用；加载后构造共享 GAS 定义，GAS 领域不反向依赖配置层。
仓库只维护根 `Cargo.toml`。业务结构与使用流程见
[20 — Excel 技能配置与 GAS 接入](./20-gas-configuration.md)。

## 配置路径与日常导表

| 路径 | 职责 | Git 管理 |
| --- | --- | --- |
| `config/tables/` | 七张 `gas.*.xlsx` 数据表 | 提交 |
| `config/defines/gas.xml`、`builtin.xml` | GAS 表登记、枚举及 Luban 内置定义 | 提交 |
| `config/luban.conf` | 输入目录、定义文件和导出目标 | 提交 |
| `config/export.ps1` | 项目导表入口，暂存生成、编译、玩法校验后发布 | 提交 |
| `config/templates/rust-bin/` | 项目维护的 Rust Result 解码模板 | 提交 |
| `src/config.rs`、`src/config/` | 可选配置模块：安全解码、配置包校验、GAS 编译与加载 | 提交 |
| `src/bin/gas_config.rs`、`examples/config_fireball.rs` | 由 `luban-config` 启用的 CLI 与火球示例 | 提交 |
| `config/LICENSE.Luban` | 初始示例文件的上游 MIT 许可证 | 提交 |
| `config/generated/` | 自动生成的 `mod.rs`、`gas.rs` 等 Rust 模块，无独立 Cargo 清单 | 提交，导表生成 |
| `config/bin/` | 七张 GAS 表的二进制数据及 `manifest.json` | 忽略，导表生成 |
| `tools/luban/.cache/` | 三种 Luban 工具的下载归档、已安装工具及临时验证产物 | 忽略 |

首次使用先准备工具链，再导表。以下命令在仓库根目录执行：

```powershell
pwsh -NoProfile -File tools/luban/setup.ps1
pwsh -NoProfile -File config/export.ps1
```

之后修改 Excel 或定义文件，只需重新执行 `config/export.ps1`。
该入口根据脚本自身位置解析路径，向 `tools/luban/run.ps1` 传递配置入口及输出目录的绝对路径，
不会受到调用者当前工作目录影响；初始入口不提供路径或生成参数覆盖选项。
生成参数固定为 `-t all -c rust-bin -d bin --strict`，并使用 `config/templates/` 自定义模板。
脚本先暂存生成结果并提取 Rust 模块，再复制根包的 `src/`、`examples/`、`Cargo.toml`、
`Cargo.lock` 和候选生成模块组成临时单包项目，格式化后以 `--features luban-config --offline --locked`
编译真实配置 CLI。新 CLI 生成清单、读取真实二进制并构建 GAS 定义。全部成功后发布代码与数据，
发布失败恢复原目录；并发导表由排他锁拒绝。原生进程失败保留非零退出码。
首次导表前可运行 `cargo build --features luban-config` 准备依赖缓存。发布不保证两个目录对并发读取者瞬时切换，
导表期间不要启动加载；运行中热更新不在首版范围内。

`luban.conf` 的路径相对于 `config/`：`dataDir` 指向 `tables`，`schemaFiles` 显式列出
`defines/builtin.xml` 和 `defines/gas.xml`，两者的 `type` 均为空字符串，按 XML 解析。
`gas.xml` 统一登记七张 GAS 表与普通枚举；表的行结构继续从对应 Excel 的 `##var`、`##type`
表头读取。原三个定义 Excel 已移除，原 `__beans__.xlsx` 没有业务 Bean 定义，不保留空的替代文件。
当前只有 `all` 目标，包含 `c`、`s`、`e` 分组，管理器为 `Tables`，生成器顶层模块为 `cfg`。
上游生成器的内置模板仍会在暂存区产生 Cargo 清单与宏包；导表脚本只提取需要的 Rust 模块，
将生成入口发布为 `config/generated/mod.rs`，由 `src/config.rs` 通过外部路径声明
`generated` 模块。仓库不维护生成包的 Cargo 清单，也不需要自定义 `toml.sbn`。

`config/tables/`、`config/defines/` 是人工维护的源文件。生成代码与二进制必须由同一次
导表产生，不手动修改；Luban 会清理输出目录，因此生成目录内只能放生成产物。
手写读取库、GAS 适配代码或自定义模板应单独维护，不能放入 `config/generated/` 或 `config/bin/`。
配置源文件、结构定义或模板变更后，应重新导表，并将相关生成代码与源文件变更放在同一次提交中。
Cargo 的 `target/` 构建缓存、`config/bin/` 和工具 `.cache/` 均由 Git 忽略。
删除根 `target/` 后，下次 Cargo 命令会重新创建构建缓存。
删除 `.cache` 不会删除配置源文件，重新执行准备和导表即可恢复工具及产物。

本仓库是 GAS 库，二进制先输出到 `config/bin/`。具体游戏负责将需要的数据部署到自身的
资源目录，例如 `assets/config/`；当前导表脚本不承担游戏资源部署。

## 初始示例来源

初始文件来自 [Luban 官方示例](https://github.com/focus-creative-games/luban_examples/tree/8e1727d5a466682684ecc081fd89551665f2e117/MiniTemplate)，
固定提交为 `8e1727d5a466682684ecc081fd89551665f2e117`：

- 最初曾将 `MiniTemplate/Data/#demo.item.xlsx` 原样复制到 `config/tables/`，该 demo 源文件和对应生成产物现已移除；
- 最初使用 `MiniTemplate/Data/__tables__.xlsx`、`__beans__.xlsx`、`__enums__.xlsx` 作为定义起点；
  当前 GAS 表登记与枚举已迁入 `config/defines/gas.xml`，三个定义 Excel 已移除；
- `MiniTemplate/Defines/builtin.xml` 原样复制到 `config/defines/`；
- 上游 MIT 许可证保留为 [`config/LICENSE.Luban`](../config/LICENSE.Luban)。

`config/luban.conf` 与 `config/export.ps1` 是本项目的入口，目录约定在本页维护。
这些示例提供最初起点；当前定义文件维护 GAS 表与枚举，导出包不再包含 demo 数据或类型。

## 固定版本与运行时要求

唯一的机器可读版本来源是
[`toolchain.lock.json`](../tools/luban/toolchain.lock.json)：

| 组件 | 固定值或要求 |
| --- | --- |
| Luban | `5.0.0`，源码提交 `52d329fb93be79810ed090f489ba4bf3821c4e4c` |
| Luban.Agent | `5.0.0`，与 Luban 相同源码提交，独立发行包与 SHA-256 |
| Luban.Mcp | 发行版本 `5.0.0`，与 Luban 相同源码提交，独立发行包与 SHA-256 |
| 本机 .NET Runtime | PATH 中第一个 `dotnet.exe` 可用的 `Microsoft.NETCore.App >= 8.0.0` 正式版 |

锁文件的 `luban`、`agent` 和 `mcp` 分别记录三个发行包的固定 URL、安装路径及 GitHub 官方
release asset 的 SHA-256 校验值；三者使用同一发行版本。
Luban.Mcp 的上游 DLL 产品版本为 `1.0.0+52d329fb93be79810ed090f489ba4bf3821c4e4c`，
锁文件用 `assemblyVersion: 1.0.0` 单独记录；这与发行版本 `5.0.0` 含义不同，保留官方 DLL 原样。
.NET 运行时使用本机安装，锁文件以 `source: system`、`minimumVersion: 8.0.0` 和
`rollForward: LatestMajor` 记录要求。

三种工具原版 `runtimeconfig.json` 的目标框架是 `net8.0`，请求的框架版本为 `8.0.0`。
这是上游编译目标；本项目通过启动参数允许使用本机更高版本的正式版运行时，保留上游配置文件原样。
`10.0.9` 已通过实际生成与产物比较，后续更换运行时建议重新执行生成验证。

来源：[Luban v5.0.0 发行版](https://github.com/focus-creative-games/luban/releases/tag/v5.0.0)。

## 脚本职责

| 文件 | 职责 | 使用时机 |
| --- | --- | --- |
| [`config/export.ps1`](../config/export.ps1) | 暂存生成、编译、完整校验和发布，并负责子进程退出码、受限路径操作和双目录回滚 | 日常导表 |
| [`setup.ps1`](../tools/luban/setup.ps1) | 检查本机运行时，下载、校验并安装三种工具，验证版本与 Agent 能力查询 | 首次使用、清理缓存后或升级工具时 |
| [`run.ps1`](../tools/luban/run.ps1) | 使用本机运行时启动 Luban，传递参数并保留退出码 | 由项目入口调用，也可手动查询帮助或诊断 |
| [`mcp.ps1`](../tools/luban/mcp.ps1) | 启动固定版本的 Luban.Mcp stdio 服务，为其指定 Luban 与 Agent DLL | 由 Codex MCP 客户端启动 |
| [`resolve-dotnet.ps1`](../tools/luban/resolve-dotnet.ps1) | 从 PATH 查找 dotnet，检查最低运行时版本，返回可执行文件路径 | 由工具脚本共用，通常无需手动执行 |

## 准备工具链与缓存

当前准备脚本支持 Windows x64，需要 PowerShell 7.2 或更高版本、PATH 中可调用的
`dotnet.exe`，以及 `7z.exe` 或 `7za.exe`。7-Zip 仅用于解压官方 `.7z` 发行包。
该 dotnet 的运行时列表必须包含至少一个 `Microsoft.NETCore.App >= 8.0.0` 正式版，
`8.x`、`9.x`、`10.x` 及后续正式版均满足要求。可先执行：

```powershell
dotnet --list-runtimes
```

`setup.ps1`、`run.ps1` 和 `mcp.ps1` 共用 [`resolve-dotnet.ps1`](../tools/luban/resolve-dotnet.ps1)，
从 PATH 定位第一个 `dotnet.exe`，并检查它列出的框架名称、正式版版本号和最低版本。
找不到命令、无法列出运行时或缺少满足要求的运行时时会明确报错，
需要安装运行时或调整 PATH 选择正确的 dotnet。
检查依据是 `dotnet --list-runtimes`，不是 `dotnet --version` 输出的 SDK 版本；
仅有预览版不满足要求。脚本不会自动下载或安装 .NET。

通过本机检查后，准备脚本下载三个固定归档，分别校验，再解压到各自的暂存目录并完成安装。
下载归档、已安装工具及临时生成验证产物均位于 `tools/luban/.cache/`，已由 Git 忽略。
锁文件和工具脚本需要提交；工具二进制和缓存不提交。
`config/` 中的配置源文件已作为项目输入提交，后续可在其中维护自己的表格；正常导表不依赖
官方示例仓库。准备脚本不再下载官方示例，也不会清理此前可能保留的示例缓存。

安装后，准备脚本检查 Luban 的完整版本字符串，以及 Agent、Mcp 入口 DLL 的 `ProductVersion`，
要求版本号和源码提交与锁文件一致；Mcp 使用锁定的 `assemblyVersion` 核对。
随后执行 Agent 的 `capabilities`，检查 JSON 结果和支持的模式。

再次执行准备脚本会校验下载归档并复用已完成安装。复用检查包括安装凭据和入口文件，
不对全部已解压文件逐个重新计算摘要，因此应将已安装工具视为只读。
中断的安装不会被标记为成功；报错指出不完整目录时，可将对应目录移走后重新准备。

## 使用固定生成器进行诊断

```powershell
pwsh -NoProfile -File tools/luban/run.ps1 --version
pwsh -NoProfile -File tools/luban/run.ps1 --help
```

`run.ps1` 将参数逐项传递给固定的 Luban，并保留调用者工作目录与生成器退出码。
直接调用时，`--conf` 和输出路径的相对路径均相对于当前工作目录解析；
日常使用 `config/export.ps1` 即可避免手动处理这些路径。
启动器调用已检查的本机 dotnet，使用 `exec --roll-forward LatestMajor`。
以上游原版运行时配置中的 `8.0.0` 为起点，默认选择满足要求的最高正式版运行时。
准备脚本的版本查询使用相同参数；脚本不覆盖 `DOTNET_ROOT`。

Luban 5.0.0 的 `--version` / `--help` 会通过 stderr 输出并返回退出码 1，这是该版本
上游 CLI 的行为；启动器保留这个退出码。准备脚本对版本查询做了专门处理，
同时要求完整版本字符串与锁定的源码提交一致。正式生成仍要求退出码为 0。

## 在 Codex 中使用 Luban MCP

项目的 [`.codex/config.toml`](../.codex/config.toml) 注册 `luban` MCP 服务。
Codex 加载该项目配置后，通过 `pwsh` 启动 [`mcp.ps1`](../tools/luban/mcp.ps1)，
使用 stdin/stdout 传输 MCP 消息。该脚本是服务入口，不是交互式查表命令，无需日常手动启动。
连接配置通过 `git rev-parse --show-toplevel` 定位仓库根目录，因此从仓库子目录启动也能找到服务入口。

首次使用先执行 `setup.ps1`，并在 Codex 中信任本项目，以允许加载项目级配置。
配置保存后，已有会话可能需要重启 MCP 服务或重新打开任务才能加载；
应以 Codex 显示 `luban` 服务及其工具可用为准。

当前启用以下工具：

| 工具 | 用途 |
| --- | --- |
| `list_tables` | 列出配置表 |
| `get_schema` | 查询完整结构 |
| `describe` | 查询指定表或类型 |
| `validate` | 加载并校验配置 |
| `generate` | 导出配置代码和二进制 |

启动器将服务的工作目录固定为仓库根目录，因此查询和校验可统一传入
`conf: "config/luban.conf"`、`target: "all"`；查询技能表时，向 `describe`
再传入 `name: "gas.TbAbility"`。也可将 `conf` 替换为配置文件的绝对路径。
本项目未准备上游本地文档目录，因此当前不启用 `search_docs`。

MCP 工具响应的文本内容为 JSON；外层 `ok`、`exitCode` 表示执行状态，
`report` 保存工具报告，`stderr`、`stdout` 保留进程输出。
Agent 的结构化查询数据位于 `report.result`，自动化调用应检查状态并读取报告中的诊断。

`mcp.ps1` 为服务设置 `LUBAN_AGENT_DLL` 和 `LUBAN_DLL`，分别指向缓存中的固定入口。
MCP 调用 Agent 完成查询与校验，调用 Luban 完成生成；因此 Agent 安装包仍需保留。
启动器复用本机 .NET 检查，并向服务及其子进程设置 `DOTNET_ROLL_FORWARD=LatestMajor`，
使下游调用继续遵循本机 Runtime 8+ 的约定。MCP 和 Agent 均用于开发阶段，不参与游戏运行时。

日常严格导表使用 `config/export.ps1`。MCP 查询和校验不会更新生成产物。
MCP 的原始 `generate` 仅用于诊断，不能替代 Rust 编译、包清单和玩法校验；
需要直接生成时输出到缓存目录，例如：

```json
{
  "args": "--conf config/luban.conf -t all -c rust-bin -d bin --strict --customTemplateDir config/templates -x outputCodeDir=tools/luban/.cache/diagnostic/code -x outputDataDir=tools/luban/.cache/diagnostic/data"
}
```

## AI skills

本项目在 `.agents/skills/` 安装六个 Luban 官方 skill，供 Codex 按任务选择使用：

| Skill | 适用任务 |
| --- | --- |
| [`luban-excel-fill`](../.agents/skills/luban-excel-fill/SKILL.md) | 填写或修改 Excel，保留表头、类型、主键和分组语义 |
| [`luban-add-table`](../.agents/skills/luban-add-table/SKILL.md) | 新增数据表、登记结构并导表 |
| [`luban-schema-design`](../.agents/skills/luban-schema-design/SKILL.md) | 设计 Bean、枚举、集合和多态配置 |
| [`luban-validator`](../.agents/skills/luban-validator/SKILL.md) | 添加引用、范围、大小等校验规则 |
| [`luban-generate-debug`](../.agents/skills/luban-generate-debug/SKILL.md) | 根据结构化错误定位表格或生成问题 |
| [`luban-runtime-load`](../.agents/skills/luban-runtime-load/SKILL.md) | 对接生成代码和配置读取端 |

来源为 [Luban 官方 ai/skills](https://github.com/focus-creative-games/luban/tree/3b6641410dcdfdbe4143b12313ff30c2e69d3d6a/ai/skills)，
固定提交 `3b6641410dcdfdbe4143b12313ff30c2e69d3d6a`。已逐文件核对六个上游 `SKILL.md` 的
Git blob，与工具链 v5.0.0 的提交 `52d329fb93be79810ed090f489ba4bf3821c4e4c` 完全一致。
保留上游正文，在每个 skill 的元数据之后添加指向本节的项目上下文说明；
新增表、Schema 设计和生成排错的上下文同时明确本仓库使用 `config/defines/gas.xml`，
上游 `__tables__` / `__beans__` 等 Excel Schema 示例不代表本项目的维护入口；
为 `luban-excel-fill` 和 `luban-validator` 的 description 添加 YAML 引号，避免 `##` 被解析成注释而截断触发描述。
上游 MIT 许可证保留在 `.agents/skills/LICENSE.Luban`。

### 本项目使用约定

- 配置入口使用 `config/luban.conf`，当前目标是 `all`；Excel 数据在 `config/tables/`。
  新增 GAS 表与枚举在 `config/defines/gas.xml` 登记，不重建 `__tables__.xlsx`、
  `__beans__.xlsx` 或 `__enums__.xlsx`。表字段默认继续从数据 Excel 表头读取，不重复声明同名 Bean。
  需要共享 Bean 时可在该 XML 中定义。`builtin.xml` 保留 Luban 内置定义，不放业务表。
  上游 `Data/`、`Defines/` 和 Excel Schema 文件均为泛用示例。
- 修改前按需使用原生 MCP `list_tables`、`get_schema` 或 `describe` 确认实际结构；
  上游 `ListTables` / `GetSchema` 在本项目对应这两个 snake_case 工具名。
  修改后使用 `validate` 检查；检查返回文本 JSON 的 `ok`、`exitCode` 和 `report`，
  结构查询结果位于 `report.result`。不要假设存在 `search_docs` 或改单元格的 MCP 工具。
- Excel 文件的实际读取和编辑仍由电子表格工具完成；skill 提供规则，MCP 提供结构查询、
  校验和生成。只改数值时保留已有类型、主键、分组、样式和非目标单元格。
- 日常生成使用 `pwsh -NoProfile -File config/export.ps1`，保持 `all + rust-bin + bin + --strict`。
  上游 `gen.bat`、`gen.sh`、直接启动 DLL 的命令需要替换为本项目入口；诊断优先使用原生 MCP，
  需要生成器 CLI 时使用 `tools/luban/run.ps1`。Agent 保留为 MCP 后端，无需恢复 `agent.ps1`。
  使用 MCP `generate` 时仅写诊断缓存，正式发布走完整导表入口。生成产物规则见本页“配置路径与日常导表”。
- 上游 Schema 的继承/多态建议只用于评估配置表达方式；运行时仍遵循 Bevy ECS、
  Component/Resource/System 与现有 GAS 架构，不据此改造为 OOP。`luban-runtime-load` 的
  C#/Unity 示例只供概念参考，实际使用 Rust；解码和适配实现位于 `src/config/`，通过
  `luban-config` feature 启用。当前模板不开放继承、多态和 flags 枚举。
- 安装或解释 skill 本身不需要修改表格或生成代码；实际任务涉及配置源文件变更时，
  按已有导表流程更新对应产物，不能通过削弱校验掩盖错误数据。

### 发现、知识库与维护

Codex 会从项目 `.agents/skills/` 发现 skill 的名称和描述，在任务匹配或显式指定时读取正文。
安装后从下一轮消息即可使用，例如 `$luban-excel-fill 修改 gas.TbAbility 中 Fireball 的 max_level`。
如果技能列表未刷新，再重启 Codex。机制参考 [Codex 官方说明](https://learn.chatgpt.com/docs/build-skills)。

`.docs/` 是按需阅读的知识库，单独复制 `SKILL.md` 到其中不会注册 skill，也不表示每轮都会
自动读取全文。本项目由各 skill 显式链接本节，使相关任务能找到实际路径和工具约定；
通用规则正文只保留在 skill 中，避免在知识库维护重复副本。

`.agents/` 不再被 Git 忽略；六个 skill、项目上下文说明和许可证应一起纳入版本管理。
提交后，其他工作目录或电脑克隆仓库即可获得这些 skill，无需单独安装。
升级时对比上游规则与固定工具链的兼容性，保留本地项目说明，并更新本节来源记录。


## 工具验证与运行时接入

初始工具准备阶段已完成归档摘要、生成器版本、重复准备以及官方 MiniTemplate 的
`rust-bin + bin + --strict` 真实生成验证。本机 .NET `10.0.9` 与原 .NET `8.0.31`
基线的 6 个输出文件逐一一致。
Luban.Agent 5.0.0 已在本机 .NET `10.0.9` 下通过五种模式验证，以及错误用法、名称不存在
的退出码和 JSON 输出检查。从其他工作目录使用绝对配置路径查询成功；查询、校验前后
`config/` 文件摘要一致，重新导表的产物内容也保持一致。
Luban.Mcp 5.0.0 已在本机 .NET `10.0.9` 下通过真实 stdio 握手、工具发现、上述五种工具
及名称不存在时的错误报告检查。从仓库外启动服务成功；查询、校验及严格导表后，
`config/` 文件摘要保持一致，关闭 stdin 后服务正常退出。此验证确认服务可用；
客户端仍需加载受信任项目的 MCP 配置，工具才会出现在会话中。

`tools/luban/.cache/smoke/code/` 和 `tools/luban/.cache/smoke/data/` 是此前工具验证的
临时产物，清理缓存后不要求保留。日常项目产物统一使用 `config/generated/` 和 `config/bin/`。
项目使用 `config/templates/rust-bin/` 与 `src/config/` 中的安全解码实现，解码返回 Result。
生成 Rust 模块与配置适配代码通过 `luban-config` 编译进同一个包；真实 Excel 火球示例包含
消耗、冷却、目标过滤、按等级伤害及同 tick 动作结束。导表入口为每次候选产物建立临时
单包项目，重新编译并执行完整校验后才发布。发布流程失败时恢复上一套代码和数据；
检查回滚行为时必须使用隔离临时目录，不能以正式产物作为故障测试对象。
配置包验证、运行示例与测试维护见 [20 — 技能配置](./20-gas-configuration.md)。

## 升级约定

升级工具链时，核对 Luban、Luban.Agent 与 Luban.Mcp 的版本、源码提交、现有配置兼容性和需要的
.NET 运行时；同步更新锁文件中的三个固定版本、URL 与校验值，并核对 Mcp 的程序集版本。
重新执行准备、MCP 查询与校验，以及项目导表验证，确认工具间对配置结构的解释一致。
`config/` 中已复制的输入不会被 `setup.ps1` 自动更新；如需引用新的上游示例，
应单独合并所需变更，并维护本页的来源记录和 `config/LICENSE.Luban`。
本机运行时升级补丁或主版本时，只要正式版仍满足 `>= 8.0.0`，就不需要修改锁文件；
建议升级后重新执行准备及导表。
涉及 Rust 模板或编码规则变化时，还要验证实际读取端。
不要通过改用 `latest` URL 或跳过摘要检查更新工具。

官方参考：[安装](https://www.datable.cn/docs/guide/install)、
[生成参数](https://www.datable.cn/docs/reference/cli)、
[新增数据表](https://www.datable.cn/docs/guide/add-table)、
[Luban MCP](https://www.datable.cn/docs/ai/mcp)。
