# 19 — Luban 配置工程与工具链

## 当前范围

项目在 `config/` 维护 Excel 配置工程，在 `tools/luban/` 固定 Luban 导表工具链。
日常通过 `config/export.ps1` 将配置导出为 Rust 代码和二进制数据；工具缓存与配置源文件分开存放。
当前配置沿用官方 MiniTemplate 的 `demo` 示例，用于验证目录和导表流程。

GAS 配置结构、Rust 读取库的 Safe Rust / Result 适配，以及配置到运行时定义的转换，
属于后续接入工作。生成的 crate 尚未作为本库依赖，也未接入游戏运行时。

## 配置路径与日常导表

| 路径 | 职责 | Git 管理 |
| --- | --- | --- |
| `config/tables/` | Excel 数据表，当前为 `#demo.item.xlsx` | 提交 |
| `config/defines/` | 表、Bean、枚举定义及 `builtin.xml` | 提交 |
| `config/luban.conf` | 输入目录、定义文件和导出目标 | 提交 |
| `config/export.ps1` | 项目导表入口，固定参数与输出路径 | 提交 |
| `config/LICENSE.Luban` | 初始示例文件的上游 MIT 许可证 | 提交 |
| `config/generated/` | 自动生成的 `cfg` 和 `macros` Rust 源码及 Cargo 清单 | 提交，导表生成 |
| `config/bin/` | 导出的二进制数据，当前为 `demo_tbitem.bytes` | 忽略，导表生成 |
| `tools/luban/.cache/` | Luban 下载归档、已安装工具及临时验证产物 | 忽略 |

首次使用先准备工具链，再导表。以下命令在仓库根目录执行：

```powershell
pwsh -NoProfile -File tools/luban/setup.ps1
pwsh -NoProfile -File config/export.ps1
```

之后修改 Excel 或定义文件，只需重新执行 `config/export.ps1`。
该入口根据脚本自身位置解析路径，向 `tools/luban/run.ps1` 传递配置入口及输出目录的绝对路径，
不会受到调用者当前工作目录影响；初始入口不提供路径或生成参数覆盖选项。
生成参数固定为 `-t all -c rust-bin -d bin --strict`，导表失败会保留非零退出码。

`luban.conf` 的路径相对于 `config/`：`dataDir` 指向 `tables`，`schemaFiles` 显式列出
`defines/builtin.xml`、`defines/__tables__.xlsx`、`defines/__beans__.xlsx` 和
`defines/__enums__.xlsx`；三个 Excel 定义文件分别使用 `table`、`bean` 和 `enum` 类型。
当前只有 `all` 目标，包含 `c`、`s`、`e` 分组，管理器为 `Tables`，顶层模块为 `cfg`。

`config/tables/`、`config/defines/` 是人工维护的源文件。生成代码与二进制必须由同一次
导表产生，不手动修改；Luban 会清理输出目录，因此生成目录内只能放生成产物。
手写读取库、GAS 适配代码或自定义模板应单独维护，不能放入 `config/generated/` 或 `config/bin/`。
配置源文件、结构定义或模板变更后，应重新导表，并将相关生成代码与源文件变更放在同一次提交中。
所有层级的 `target/` 构建目录均由 Git 忽略，避免提交生成子 crate 的编译产物；
`config/bin/` 和工具 `.cache/` 继续忽略。
删除 `.cache` 不会删除配置源文件，重新执行准备和导表即可恢复工具及产物。

本仓库是 GAS 库，二进制先输出到 `config/bin/`。具体游戏负责将需要的数据部署到自身的
资源目录，例如 `assets/config/`；当前导表脚本不承担游戏资源部署。

## 初始示例来源

初始文件来自 [Luban 官方示例](https://github.com/focus-creative-games/luban_examples/tree/8e1727d5a466682684ecc081fd89551665f2e117/MiniTemplate)，
固定提交为 `8e1727d5a466682684ecc081fd89551665f2e117`：

- `MiniTemplate/Data/#demo.item.xlsx` 原样复制到 `config/tables/`；
- `MiniTemplate/Data/__tables__.xlsx`、`__beans__.xlsx`、`__enums__.xlsx` 原样复制到 `config/defines/`；
- `MiniTemplate/Defines/builtin.xml` 原样复制到 `config/defines/`；
- 上游 MIT 许可证保留为 [`config/LICENSE.Luban`](../config/LICENSE.Luban)。

`config/luban.conf` 与 `config/export.ps1` 是本项目的入口，目录约定在本页维护。
这些示例只提供可运行的起点，尚未定义正式的 GAS 业务配置结构。

## 固定版本与运行时要求

唯一的机器可读版本来源是
[`toolchain.lock.json`](../tools/luban/toolchain.lock.json)：

| 组件 | 固定值或要求 |
| --- | --- |
| Luban | `5.0.0`，源码提交 `52d329fb93be79810ed090f489ba4bf3821c4e4c` |
| 本机 .NET Runtime | PATH 中第一个 `dotnet.exe` 可用的 `Microsoft.NETCore.App >= 8.0.0` 正式版 |

Luban 发行包使用 GitHub 官方 release asset 的 SHA-256 校验值。
.NET 运行时使用本机安装，锁文件以 `source: system`、`minimumVersion: 8.0.0` 和
`rollForward: LatestMajor` 记录要求。

Luban 原版 `runtimeconfig.json` 的目标框架是 `net8.0`，请求的框架版本为 `8.0.0`。
这是上游编译目标；本项目通过启动参数允许使用本机更高版本的正式版运行时，保留上游配置文件原样。
`10.0.9` 已通过实际生成与产物比较，后续更换运行时建议重新执行生成验证。

来源：[Luban v5.0.0 发行版](https://github.com/focus-creative-games/luban/releases/tag/v5.0.0)。

## 脚本职责

| 文件 | 职责 | 使用时机 |
| --- | --- | --- |
| [`config/export.ps1`](../config/export.ps1) | 使用固定项目路径和严格校验参数导出 Rust 代码与二进制 | 日常导表 |
| [`setup.ps1`](../tools/luban/setup.ps1) | 检查本机运行时，下载、校验并安装固定的 Luban，验证生成器版本 | 首次使用、清理缓存后或升级工具时 |
| [`run.ps1`](../tools/luban/run.ps1) | 使用本机运行时启动 Luban，传递参数并保留退出码 | 由项目入口调用，也可手动查询帮助或诊断 |
| [`resolve-dotnet.ps1`](../tools/luban/resolve-dotnet.ps1) | 从 PATH 查找 dotnet，检查最低运行时版本，返回可执行文件路径 | 由工具脚本共用，通常无需手动执行 |

## 准备工具链与缓存

当前准备脚本支持 Windows x64，需要 PowerShell 7.2 或更高版本、PATH 中可调用的
`dotnet.exe`，以及 `7z.exe` 或 `7za.exe`。7-Zip 仅用于解压官方 `.7z` 发行包。
该 dotnet 的运行时列表必须包含至少一个 `Microsoft.NETCore.App >= 8.0.0` 正式版，
`8.x`、`9.x`、`10.x` 及后续正式版均满足要求。可先执行：

```powershell
dotnet --list-runtimes
```

`setup.ps1` 和 `run.ps1` 共用 [`resolve-dotnet.ps1`](../tools/luban/resolve-dotnet.ps1)，
从 PATH 定位第一个 `dotnet.exe`，并检查它列出的框架名称、正式版版本号和最低版本。
找不到命令、无法列出运行时或缺少满足要求的运行时时会明确报错，
需要安装运行时或调整 PATH 选择正确的 dotnet。
检查依据是 `dotnet --list-runtimes`，不是 `dotnet --version` 输出的 SDK 版本；
仅有预览版不满足要求。脚本不会自动下载或安装 .NET。

通过本机检查后，准备脚本只下载固定的 Luban 归档，先校验归档，再解压到暂存目录并完成安装。
下载归档、已安装工具及临时生成验证产物均位于 `tools/luban/.cache/`，已由 Git 忽略。
锁文件和工具脚本需要提交；工具二进制和缓存不提交。
`config/` 中的初始示例已作为项目输入提交，后续可在其中维护自己的表格；正常导表不依赖
官方示例仓库。准备脚本不再下载官方示例，也不会清理此前可能保留的示例缓存。

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

## 工具验证与后续接入

工具准备阶段已完成归档摘要、生成器版本、重复准备以及官方 MiniTemplate 的
`rust-bin + bin + --strict` 真实生成验证。本机 .NET `10.0.9` 与原 .NET `8.0.31`
基线的 6 个输出文件逐一一致。

`tools/luban/.cache/smoke/code/` 和 `tools/luban/.cache/smoke/data/` 是此前工具验证的
临时产物，清理缓存后不要求保留。日常项目产物统一使用 `config/generated/` 和 `config/bin/`。
生成成功只确认导表流程可用，不代表 Rust 配置读取或 GAS 接入已经完成。

当前生成的 `cfg` crate 仍按上游模板依赖 `../../luban_lib`，其路径对应 `config/luban_lib/`。
该读取库尚未建立，生成 crate 也未加入根项目的依赖；需要完成读取库与模板适配后再编译接入。
后续适配可按需查阅上述来源提交中的
[`Projects/Rust_bin/`](https://github.com/focus-creative-games/luban_examples/tree/8e1727d5a466682684ecc081fd89551665f2e117/Projects/Rust_bin)
及其 `luban_lib/`，不依赖准备脚本下载参考快照。

下一步可设计一张瞬时属性加减效果表，验证 Excel 数值经导出、读取和适配后改变 GAS
结算结果，再扩展到冷却、消耗和任务。涉及 `read_* -> Result` 的改动需要同步调整生成的
反序列化表达式；自定义模板应在生成目录之外单独维护。

## 升级约定

升级生成器时，核对生成器版本、源码提交、现有配置兼容性和需要的 .NET 运行时，
更新锁文件中的 Luban 固定 URL 与校验值，然后重新执行准备及项目导表验证。
`config/` 中已复制的输入不会被 `setup.ps1` 自动更新；如需引用新的上游示例，
应单独合并所需变更，并维护本页的来源记录和 `config/LICENSE.Luban`。
本机运行时升级补丁或主版本时，只要正式版仍满足 `>= 8.0.0`，就不需要修改锁文件；
建议升级后重新执行准备及导表。
涉及 Rust 模板或编码规则变化时，还要验证实际读取端。
不要通过改用 `latest` URL 或跳过摘要检查更新工具。

官方参考：[安装](https://www.datable.cn/docs/guide/install)、
[生成参数](https://www.datable.cn/docs/reference/cli)、
[新增数据表](https://www.datable.cn/docs/guide/add-table)。
