# C 批次新构建真实渠道验收条件 — #57

本清单承接 [规格 #9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9) 的
T08–T11、T14、T15，以及 #39–50 的逐组合完成条件。它是验收准备，未执行真实推理，
不改变能力声明、不扩大权限、不授权新账号、配置发布、服务启停或部署。

既有“已有账号渠道真实验证、无调用或费用上限”的授权保留在其原目标和范围内，
不重新解释成新环境部署或账号恢复授权。本轮选择先收敛软件及整理条件，真实 Provider 调用为零。

## 新构建共同前置条件

| 条件 | 必需证据与不满足时的状态 |
|---|---|
| 不可变候选 | 实现 source SHA、适用 Full/Fast 与两轴评审、操作系统/工具链、产物 SHA256、构建时间。产物尚未构建为 NOT_RUN；不能以本地 HEAD 或旧构建实测代表运行中的新源码。 |
| 实际运行构建 | 从目标实例的实际进程/可执行文件或等价发布清单确认运行 SHA 和产物校验值，记录实例、观察时间与 API 地址。仅给现有脚本传入 `--runtime-commit` 不构成构建核实。新部署需其具体授权，本清单不执行。 |
| 当前配置与权限 | 活动 Config Version ID/revision、明确的 Provider/channel/endpoint/route/candidate、连接 API 格式/路径、实际模型 ID、Client-Key 可见模型和授权组。查询 `/models` 只证明此权限视图，不证明推理。不得用别的 endpoint 自动替补来填本组合 PASS。 |
| 当前账号 | 脱敏 account/credential 标识、类型、revision、启用意图、到期/授权状态及其观察时间；native pool 与普通 binding 分开。缺 revision 保持 null 并阻断精确身份验收，不能推定为零。目录成功不等于推理成功。 |
| 出口和租约 | 明确 Direct/固定代理/池及其 revision；Console/Web 会话、Web clearance、Build reasoning owner 保留各自绑定。每请求核对实际成功 attempt 的 endpoint/credential/revision；池中一份账号成功不能接受整个池。 |
| 参数与能力冻结 | 记录模型、协议、流模式、实际提交的输出上限、reasoning/parallel 控制，以及 stored continuity/compaction/WS 等实际声明。不能关闭已声明能力后标通过，不能从渠道名推断未声明能力。 |
| 软件阻断先决条件 | Kiro 原生硬输出上限、Web 原生连续性/必填输出上限尚未有完整契约和实现的组合继续 BLOCKED；账号补齐不解除它们。原生富元数据不能表示的 Chat/Messages 模型组合也必须独立记录 BLOCKED/FAIL。 |
| 调用范围与停机条件 | 沿用适用授权中的已有账号、目标和预算；新目标/费用/恢复/发布操作需其授权。单次请求固定协议和精确目标；发现身份漂移、假终态、语义损失或非预期重发立即终止该组合。调用产生正常 Usage/lease/health/quota 记录须在回执中说明。 |

## 11 渠道条件与声明扩展

下表引用本轮从 GitHub 读取的 #40–50；具体运行配置仍需在真实执行前冻结。
“2026-09-30 历史观察”全部来自旧运行 `b0cf36e` 的既有报告，本轮没有重读生产 inventory，
不表示现在仍是该状态。历史缺账号与新构建的软件阻断分别保存。

| 渠道／工单 | 必需的原生身份与连接条件 | 必须保留的声明扩展 | 2026-09-30 历史观察与当前独立缺口 |
|---|---|---|---|
| openai-compatible #40 | 自定义地址/API key、实际 Responses 或 Chat adapter、模型与代理配置、精确 serving binding | reasoning/parallel/WS；Responses 明确启用的续接/压缩 | Krill 旧构建六项 pin FAIL；需新构建的同一目标归属与文本/连续工具闭环，不能以合成 Moonshot/兼容 endpoint 证明厂商真实 API 存在。 |
| anthropic-compatible #41 | Messages API key、配置目录与认证头；不附会 OAuth 身份或额度 | reasoning/parallel/WS；续接/压缩按实际声明 | 历史 serving inventory 未见独立账号；需可用的授权账号与 route。 |
| codex #42 | OAuth account binding、原生请求头、身份/revision、目录/额度来源，精确 Responses 连接 | reasoning/parallel/WS；明确配置的续接/压缩 | 历史 CredentialUnavailable；授权恢复与新推理验收是不同动作，不以目录或旧令牌存在接受新构建。 |
| claude #43 | 实际 OAuth 或 API-key 认证、Messages endpoint、独立身份/目录/额度来源 | reasoning/parallel/WS；续接/压缩按实际声明 | 历史未见对应账号；缺账号不能借 anthropic-compatible 的 fixture 代替。 |
| grok.official #44 | 官方 API key 与 `api.x.ai/v1/responses`，独立模型、限流/计费元数据及 quota owner | reasoning/parallel/WS；续接/压缩按实际声明 | 历史未见账号；本轮原生元数据为本地证据，仍需实际模型输出/引用/错误与额度归属。 |
| kimi-coding #45 | 独立 OAuth/device 材料、设备身份、明确 Coding Responses/Chat 目标与启用 route | reasoning/parallel/WS；明确配置的续接/压缩 | 历史有账号但无 enabled route；发布生产 route 需要其授权，不能改用 Moonshot API key。 |
| kimi-api #46 | Moonshot API key、所选兼容 endpoint 的实际 API 格式与能力证据 | 实际 endpoint 声明的 WS 及其他扩展 | 历史未见账号；不能从渠道名或合成 Responses peer 推断真实厂商接口/能力。 |
| grok.build #47 | 原生设备 OAuth、可用池账号/revision、CLI 目标、专有连续性、出口/额度、encrypted reasoning owner | reasoning/stored continuity/compaction/WS；未声明 parallel 时拒绝 | 旧构建仅指定账号的 Responses/Messages 代表性文本与一次工具回传 PASS，Chat FAIL；本轮删除 reasoning 静默过滤，但新构建、第二工具周期及声明扩展均未真实验收。 |
| grok.console #48 | native SSO、原生 console 目标、DPoP/bootstrap、实际模型、额度/出口绑定 | reasoning/WS；parallel/续接/压缩按实际声明 | 历史 unauthorized/unavailable，部分账号 revision 缺失；不能推定每份 SSO 的失效原因，不能用 Build/Web 账号替补。 |
| grok.web #49 | native SSO、exact account/revision、出口会话/clearance、conversation 和 parent response 的可信原生绑定 | stored continuity/WS；工具为既定不支持，reasoning/parallel/compact 不自动承诺 | 历史未见账号；当前生产构造只接受单个 user 消息及 temporary new conversation，多轮和 Messages 必填输出上限仍 SOFTWARE BLOCKED。 |
| kiro #50，装配 #39 | 分别核对凭据类型、地区、IDE/CLI endpoint/origin、动态目录和精确 profile ARN/revision；API key 不冒充 OAuth 目录 | reasoning/WS；未声明 parallel/续接/压缩不自动承诺 | 历史未见账号；当前 Messages 及其他协议带上限的请求仍 SOFTWARE BLOCKED。Social IDE/CLI 合成回归不证明全部 credential/地区组合。 |

各渠道基础组合继续按 Chat/Messages/Responses × JSON/SSE 单独记录。每组合至少完成独立
多轮文本，以及非 Web 的单工具/结果回传和连续两个工具周期（第二个工具调用及其结果后的回答）。
冻结角色、顺序、模型、工具 ID/name/参数/结果/错误历史，核对唯一成功终态、原生 Usage 和实际 attempt。
原有两个限制单列：Web 三协议工具不支持；Messages 精确 token counting 不支持。
不把拒绝用例通过记成该能力正例通过。

## 未解决的软件／协议前置条件

### Kiro 输出硬上限

`provider-kiro::conversation_request::reject_unsupported_request_fields` 仍拒绝 unscoped
Canonical extensions；目前没有经过核实的原生输出上限字段。下一步需要明确字段/单位/模型与
IDE/CLI、凭据、地区的适用性，保留下游请求上限，验证原生 outbound 及实际截断终态/Usage。
提示词 token budget、删除 max_tokens、只截断下游、少量样本恰好小于上限均不能证明等价硬上限。
Messages 必填 `max_tokens` 不得取消；Chat 的两个上限别名和 Responses `max_output_tokens`
也不能静默删除。本轮 16 个拒绝断言只证明零新增 attempt/发送，正例仍未完成。

### Web 原生连续性与输出上限

`provider-grok::web_production::normalized_message` 仍要求一个 user 消息；生产 body 保留
`temporary:true`、`disableMemory:true` 的既有 new-conversation 契约。现有
`GrokWebConversationState` 类型不构成公共运行路径已接入原生续接的证据。
下一步需可核实的 continuation 目标/请求 schema、原生 conversation/response/parent ID、
可信客户端续接载体及 exact account/revision/egress/session 绑定，并覆盖 Chat、Messages、
Responses 的多轮运行路径和 WS/存储历史，不凭拼接角色标签、重放成单条提示或工具模拟获得 PASS。
Messages 上限还需独立原生映射。现有参考源码只提供线索，未填补这些契约和真实证据。

### Grok 富元数据与扩展

本轮支持的原生 summary/content/phase/citations 及显式 Responses 历史回放不等于 stored
continuity、compaction 或 WS 的逐渠道验收。非空 logprobs、未审定签名、未知 semantic
event 字段继续失败；Official/Console 非 null encrypted reasoning 没有所有权路径，不能透传。
若实际模型必然产生无法等价表示的元数据，记录该模型/协议/参数组合及具体字段边界，保留受阻，
不自动调整 spec #9 或把它登记成已接受的新限制。

## 真实回执与 #57 归集

每条真实组合回执至少包含：

1. `layer=LIVE_NEW_BUILD`、运行/产物/source SHA、实例/连接、安全时间窗口、配置 ID/revision。
2. Provider/channel/endpoint/route/candidate、原生模型、下游协议、JSON/SSE、场景/轮次、
   完整参数键和值的脱敏表示、实际声明扩展、可用账号与实际使用 credential 的脱敏标识/revision。
3. 公共 HTTP 状态、request ID（缺失保持 null）、实际 attempt 的目标/次数/结果及关联方法。
   独占时间窗口关联须注明推断与歧义，不能冒称 request-ID 直接关联。
4. 角色/工具/终态/参数/原生 Usage 的行为断言，响应侧错误的唯一错误终态与后续零重发；
   Usage 不可得保持 nullable/provenance，不补造计数。
5. PASS/FAIL/NOT_RUN/BLOCKED、失败类别与精确阻断、适用既定限制、关联证据文件/校验值。
   不保存 API key、SSO/OAuth token、DPoP/private material、敏感正文或账本原始身份。

既有 [cpar-batch-c-live.py](../../../../scripts/acceptance/cpar-batch-c-live.py) 仍是旧运行
代表性 smoke runner：运行 SHA 由调用者输入、工具最多一次回传、未验证硬上限兑现、
未覆盖声明扩展/完整 Usage/唯一终态与实际 account revision，也没有全部逐渠道精确归属。
它的回执应保持原有 `LIVE_EXISTING_RUNTIME` 与约束未核实标签，不能仅改标签就接受新构建。
正式验收需补足上述回执与第二工具周期、故障/取消/重试、声明扩展的独立 oracle。

#57 只汇集对应层级的证据和未完成清单；C 的本地 Gate/评审即使通过，#39–50、#57 和父规格
也不能自动关闭。11 渠道/实际声明扩展之外的其他批次、88 故事/F01–F08/T01–T15 全范围结论
不由本清单重新验收或宣布完成。
