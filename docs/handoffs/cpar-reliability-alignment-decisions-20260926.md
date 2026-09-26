# CPAR 可靠性与 CPA 行为对齐：讨论决策记录

日期：2026-09-26。决策确认时状态：用户已确认方案，交付计划文档。其后用户已指令实施 M0/M1；动态进度以关联开发计划与验收清单为准，本页保留讨论原始决定。

关联：[开发计划](cpar-reliability-alignment-plan-20260926.md) · [验收清单](cpar-reliability-alignment-acceptance-20260926.md) · [当前状态](../reports/cpar-current-status-20260926.md)。

## 1. 最终有效决策

| 编号 | 问题与实际回复 | 最终解释 |
| --- | --- | --- |
| Q1 | 个人与可信小团队 | 保留 Rust 重构 CPA、参考 CPAMP 日常管理的定位，不建设充值、订阅或商业运营平台。 |
| Q2 | 记录可靠优先 | 无法保障必需请求记录时，拒绝新请求，接受短时不可用。已接收请求须可追溯；上游结果不明不能伪造成功或费用。 |
| Q3 | 初选金额硬预算 | 已被 Q11、Q13 替代，**不是本轮实施要求**。 |
| Q4 | 真实链路闭环 | 本地故障/协议回归、生产复查、受控真实客户端多轮分开验收，不用模拟交换冒充真实授权。 |
| Q5 | USD，人工确认换算 | 随金额预算延期；仅作为未来方案输入，不据此修改本轮价格契约或推断历史币种。 |
| Q6 | 每个 Key 独立预算 | 随金额预算延期；本轮不引入全局或 Key 金额预算。 |
| Q7 | 缺价拒绝、未知用量保留预留 | 随金额预算延期；本轮保留现有 unpriced/unknown/partial 统计语义，不静默将缺价模型变成不可调用。 |
| Q8–Q10 | 周期、重试计费、旧 Key 迁移均“参考 CPA 即可” | 先核查参考源码，不把上游套餐额度解释成客户端美元预算；由 Q11 消除两者差异。 |
| Q11 | 本轮严格跟随 CPA，暂不新增金额硬预算 | 先补可靠性、使用统计与账号额度观测。硬预算及其预留/结算规则进入后续计划。 |
| Q12 | 最多 12 次上游推理尝试，每次最多 512 输出 token，无自动重试 | 仅限本计划验收；失败也计数、到额即停。旧任务额度不能借用或重置。当前新额度使用为 0/12。 |
| Q13 | 确认 M0–M4，并落盘 | 五阶段计划与边界成立；独立客户端 RPM 限额等新增使用控制亦暂缓。本次交付止于计划、决策和验收文档，不执行开发或上线。 |

本记录中的初选项保留讨论历史，但不得与最终决定并行执行。用户已确认的日常实现授权与安全约束继续按实际任务范围解释，不因旧文档措辞扩展范围。

## 2. 为什么不能直接套 CPA 的金额预算

固定参考代码：

- CLIProxyAPI 提交 ed980be34b9981735eaa16941956b1e8d9abfb7c 的 [SDKConfig.APIKeys](https://github.com/router-for-me/CLIProxyAPI/blob/ed980be34b9981735eaa16941956b1e8d9abfb7c/internal/config/sdk_config.go) 是认证字符串列表；[内置认证路径](https://github.com/router-for-me/CLIProxyAPI/blob/ed980be34b9981735eaa16941956b1e8d9abfb7c/internal/access/config_access/provider.go) 不提供所讨论的美元预算周期。
- [凭据并发](https://github.com/router-for-me/CLIProxyAPI/blob/ed980be34b9981735eaa16941956b1e8d9abfb7c/internal/config/credential_concurrency.go) 与 [重试配置](https://github.com/router-for-me/CLIProxyAPI/blob/ed980be34b9981735eaa16941956b1e8d9abfb7c/internal/config/config.go) 管理上游凭据和请求重试，不是客户端金额结算。
- CPA-Manager-Plus 提交 e19d8267a52ca146c43bdb86d185bb7baed7ff38 的 [Key 别名模型](https://github.com/seakee/CPA-Manager-Plus/blob/e19d8267a52ca146c43bdb86d185bb7baed7ff38/apps/manager-server/internal/model/api_key_alias.go) 与 [费用计算](https://github.com/seakee/CPA-Manager-Plus/blob/e19d8267a52ca146c43bdb86d185bb7baed7ff38/apps/manager-server/internal/service/pricing/cost.go) 支持管理和估算；不能由此推导硬预算周期、余额预留或故障时的扣减规则。

否定结论仅覆盖所核查固定版本的配置、内置认证、管理、quota、用量和成本路径，不代表所有插件或整个生态均无预算能力。行为参考不等于复制实现，也不照搬参考项目缺价返回零的计算方式。

## 3. 持续有效的约束

- 覆盖既有 API、Codex/ChatGPT、Claude、Kimi、Kiro、Grok Web/Console/Build；不新增渠道和媒体协议。
- 保留 V2 / Apple Liquid Glass、八个工作区、旧路由和 query 深链；重点修复栏目内部操作与恢复，不重新更换设计方向。
- 需要设计推敲时，可在既有 OpenDesign 项目 prism-gateway-console-redesign-a735 使用已授权的 Kimi for Coding K3 路由；现有 high 已获允许，不把 high 写成 max。只发送合成数据。本次文档工作未调用设计模型。
- 本轮不使用 Codex with ChatGPT；Codex 负责实施、取舍和最终验证。子代理仅一轮只读探索，显式 fork_turns=none，不追派。
- 保留账号、历史请求与账本；11 条歧义事件继续隔离并停止无效重试。没有逐次调用证据，不补造费用或改写历史。
- 模型保留 exact ID；目录、可服务模型、Key 权限分开。目录发现不自动开放权限，套餐不生成模型名单。
- 认证、调度、额度、权益、目录及记录持久化健康分别表达；null 与未知不补零。
- 现有 Oracle 服务和域名为未来验收目标，不改 DNS、Caddy、Autoreg 或其他服务；不清理生产历史数据，不发布新的公共镜像/安装包。
- 浏览器测试使用 EgoLite；官方登录/条款确认由本人完成。工具不可用时记录实际阻碍，不静默切换浏览器或模型。

## 4. 新增真实调用授权登记

| 项目 | 约束 |
| --- | --- |
| 归属 | 本计划的 M4 真实验收；与历史 CPAR/OMP 额度隔离 |
| 总上限 | 12 次真正发往上游的推理尝试，不是 12 次客户端请求 |
| 输出上限 | 每次最多 512 输出 token；若渠道不能明确执行该限制，先停止该项，不静默放宽 |
| 重试 | 关闭客户端、网关和 harness 自动推理重试/自动换号重放；人工安排的另一次尝试仍占总数 |
| 计数 | 发送失败、超时、失败响应、取消以及发送结果不确定均按已消耗尝试保守登记 |
| 内容与工具 | 合成内容；工具读写限隔离测试目录，不发送现有私有文件，不执行模型生成的任意命令 |
| 优先级 | 可用现有渠道冒烟、Grok 代表性客户端多轮及其持久记录回读；不是承诺所有渠道已获真实验收 |
| 停止 | 达额、异常扩大范围、输出限制不可保证或登录未完成时停止依赖步骤；不靠无限重跑取得成功 |
| 当前使用 | 0/12；本次只做讨论、源码核查和文档，没有新增推理 |

认证、目录、额度等必要元数据请求不计入“推理 12 次”，但仍需有界、遵守渠道访问约束，不能借此执行推理或无节制轮询。输出次数上限不等同固定美元费用承诺。
