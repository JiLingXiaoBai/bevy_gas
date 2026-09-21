# 19 — 外部配置集成

## 配置由游戏拥有

`bevy_gas` 提供运行时定义、ECS 组件与系统。游戏维护自己的数据表、结构、生成类型、
加载器、校验器、稳定配置 ID 和定义索引，再把数据转换为公开 GAS 类型。

配套游戏起始模板 [bevy_gas_template](../../bevy_gas_template/README.md) 展示完整的独立接入。
数据生产、资源部署和游戏测试以该工程文档为准；使用本库不要求采用特定配置格式或工具链。

## 公共接口衔接

1. 安装 `GameplayAbilitySystemPlugin`，准备名称、标签和属性注册资源。
2. 游戏读取并校验自己的数据，以确定顺序注册标签、属性和额外资源名称。
   常规系统可使用 `GameplayTagRegister`、`AttributeIdRegister` 与 `UniqueNamePool`。
3. 构造 `Modifier`、`GameplayEffect`、`GameplayAbility` 和 `TargetingDefinition`，
   通过 `Arc` 共享定义；游戏的 Resource 保存稳定配置 ID 到定义的映射。
4. 创建 `GameplayAbilitySystemBundle`，初始化属性，再通过
   `AbilitySystemComponent::give_ability()` 授予技能，保存返回的 `AbilitySpecHandle`。
5. 游戏系统在 `GameplayAbilitySystemSet::RequestProducers` 产生目标或激活请求，
   由公开 `TargetingRequestQueue`、`GameplayExecutionQueue` 进入固定 tick 运行时。

游戏配置 ID 与运行时 Handle 分开保存。名称、标签、属性和技能 Handle 属于当前 World，
不应直接当作跨存档的稳定编号。同一个逻辑效果应复用同一份 `Arc<GameplayEffect>`，
以保留以定义身份区分的堆叠和匹配语义。

如果配置编译需要失败时保持注册表不变，可克隆 `UniqueNamePool`、
`GameplayTagManager` 和 `AttributeIdManager`，在快照上完成注册与全部定义构造，
成功后再提交三个资源。当前公开的 `register_tag_internal()` 和 `register_id_internal()`
可用于该流程；游戏仍须校验既存标签的继承关系，并处理属性区域和容量错误。
应在启动阶段完成编译；此流程不提供运行中自动替换技能与效果定义的协议。

## 扩展边界

- 自定义数值公式实现 `ModifierMagnitudeCalculation`，由游戏编译层构造
  `ModifierMagnitude::Calculated`；只读取公开求值上下文。
- 背包、耐久等成本由 `AdditionalCost` 描述，通过游戏的 `AdditionalCostProvider`
  执行同步检查、支付和失败补偿。
- 游戏事件动作可构造 `AbilityTaskOnFinishedDef::EmitEvent`，通过
  `On<AbilityTaskEvent>` Observer 接入投射物、动画等游戏系统。Observer 延迟执行，
  不能充当启动阶段下一动作之前必须完成的同步结算。
- 新的原生任务生命周期或基础聚合操作仍需扩展库的枚举与执行逻辑；
  当前 API 没有任意任务种类的外部注册协议。

任务顺序与事件时机见 [08 — 技能任务](./08-ability-tasks.md)，
数值和成本扩展见 [14 — 扩展系统](./14-extending-the-system.md)。
配置结构变化由游戏更新自己的生成类型、编译映射与测试，不需要修改 GAS 核心。

## 升级验证

游戏可通过 Cargo 的 path、Git 或发布版本依赖本库。配套模板当前使用 `../bevy_gas`
本地 path 依赖，本地库的修改会在模板下次编译时参与构建。
修改依赖后，先运行本库测试，再运行游戏的配置验证和玩法回归。
旧版库内的配置模块和工具已经移出，既有调用方需把对应配置代码迁入游戏工程，
保留对上述 GAS 公共接口的调用。
