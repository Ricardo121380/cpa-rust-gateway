# CPAR 现有渠道与三协议证据基线

审计日期：2026-09-29（Asia/Shanghai）。源码冻结为
`9fc4ceb4e12aab9c60658869706828a7d5d8312f`（2026-09-28 的文档提交）。
任务：[建立 CPAR 现有渠道与三协议的证据基线](https://github.com/Ricardo121380/cpa-rust-gateway/issues/3)。
所属地图：[CPAR 自用可靠性与渠道操作体验收敛](https://github.com/Ricardo121380/cpa-rust-gateway/issues/1)。

## 结论与证据规则

三种公共入口及九种协议配对已登记，11 个渠道入口也都能定位到管理或 serving 实现；这不构成 11 × 3 全面可用的结论。当前主要缺口是接入步骤仍暴露底层资源、协议和能力限制未形成统一承诺、状态与验收证据分散。Kiro 的跨协议装配限制、Reasoning 对 Chat 的准入限制，以及多种明确的转换损失需要进入下一轮兼容性决策。

本轮只读核对跟踪源码、契约、测试源码和既有脱敏报告。业务代码相对冻结提交无差异；工作区原有 AGENTS.md 修改和未跟踪文件不作为能力证据。没有读取凭据、生产数据库、私有截图或原始生产回执，没有启动服务、执行行为测试、调用真实 Provider 或操作生产。报告是唯一新增的跟踪文件。

以下状态必须连同层级、日期及版本阅读：

| 状态 | 本报告中的含义 |
| --- | --- |
| PASS | 指定层级的具体事实得到证据支持；源码 PASS 只证明分支/约束存在，历史 PASS 只转述当次报告结论 |
| FAIL | 指定目标未满足，或报告记录实际失败；分别标明源码可证实缺陷、历史上游拒绝，不能混为软件故障 |
| NOT_RUN | 本轮未执行行为验证，或没有定位到相应验收证据；未定位不等于从未实现或从未运行 |
| BLOCKED | 明确的前置条件或准入规则阻断；区分源码拒绝、历史凭据失效、当时缺账号 |

层级为 **S 源码静态核对、L 本地/loopback 测试、B 模拟浏览器、R 真实渠道、P 生产只读/部署**。所有 S 均为 2026-09-29 / 上述 SHA；本轮 L/B/R/P 行为验证均为 NOT_RUN。历史证据另列日期和版本。`TEST_PRESENT` 是测试源码索引，不是运行结果。未知单元格保留未知，不用健康检查、模型目录或保存回执替代推理验收。

文中 S/T/H 引用均指向冻结 SHA 的文件和行号，避免后续分支变化改变出处。最新共同生产报告记载的运行代码为 `b0cf36e`，不是本报告源码 HEAD；此刻部署和账号状态没有重新核验。[H03]

## 1. 11 渠道 × 三协议矩阵

表内“路径”表示静态装配和准入条件可定位，**运行状态仍为 NOT_RUN**，不承诺任意模型、参数或上游响应可用。表中能力基准为文本、JSON/SSE 和允许的 function-tool 子集；图片、thinking、连续性等另见第 2 节。

- **原生**：候选声明协议与下游相同，仍经过公共解码及 provider 构造/响应转换。
- **桥接**：必须显式配置正确 transform mode，并满足请求及响应语义限制；不自动回退。
- **收窄**：默认 Reasoning capability 会阻断 Chat。当前形状门禁允许通用 Responses、Grok Build/Console 通过明确 `allow_unlisted_model:true, reasoning:false` 收窄；通用 Responses 另有 stored/compact opt-in 组合。收窄声明不保证上游实际不会返回 reasoning。[S03][S04]
- **BLOCKED/S**：当前源码的组合约束已明确阻断，未发真实请求。不是凭据失效判断。

| 渠道入口 | 实际上游适配方式 | Chat Completions | Messages | Responses | 核心出处 |
| --- | --- | --- | --- | --- | --- |
| `openai-compatible` | 通用 Chat 或 Responses adapter，地址来自配置 | Chat 原生；Responses 需收窄后桥接 / NOT_RUN | 向 Chat 或 Responses 桥接；各自限制不同 / NOT_RUN | Responses 原生，或向 Chat 的有限桥接 / NOT_RUN | [S01][S02][S03][S04][S05] |
| `anthropic-compatible` | `anthropic-compatible.messages` | BLOCKED/S：Reasoning profile；当前 override 形状不允许该 adapter 收窄 | 原生 / NOT_RUN | 向 Messages 桥接 / NOT_RUN | [S02][S03][S04] |
| `codex` | 通用 Responses adapter + 官方 URL/account/header 专属路径 | 需收窄后桥接 / NOT_RUN | 向 Responses 桥接；不推定支持任意私有 reasoning 历史 / NOT_RUN | 原生 / NOT_RUN | [S05][S06][S03][S04] |
| `claude` | 通用 Messages adapter + Claude 凭据/授权 | BLOCKED/S：同 Anthropic adapter 限制 | 原生 / NOT_RUN | 向 Messages 桥接 / NOT_RUN | [S02][S05][S06][S03][S04] |
| `grok.official` | `grok.official.responses`，固定 xAI target | BLOCKED/S：Reasoning profile；当前 override 不允许收窄 | `canonical_bridge` 有限路径 / NOT_RUN | 原生 typed Canonical / NOT_RUN | [S01][S02][S03][S04][S07] |
| `kimi-coding` | 通用 Responses adapter + Kimi OAuth/device headers | 需收窄后桥接 / NOT_RUN | 向 Responses 桥接 / NOT_RUN | 原生 / NOT_RUN | [S06][S08][S03][S04] |
| `kimi-api` | UI 默认 Moonshot Chat；复用通用 Chat adapter，无独立 provider adapter | 默认 Chat 原生 / NOT_RUN | 向 Chat 桥接，仅可表达子集 / NOT_RUN | 向 Chat 桥接，仅可表达子集 / NOT_RUN | [S05][S09][S03] |
| `grok.build` | `grok.build.responses`，独立 native OAuth pool | 需收窄 + `canonical_bridge`；含 native reasoning 输出仍有限制 / NOT_RUN | `canonical_bridge` 有限路径；native reasoning/连续性不保证可转 / NOT_RUN | 原生，含受约束工具/原生 reasoning 路径 / NOT_RUN | [S02][S04][S07][T07] |
| `grok.console` | `grok.console.responses`，独立 native SSO pool | 需收窄 + `canonical_bridge` / NOT_RUN | `canonical_bridge` 有限路径 / NOT_RUN | 原生 typed Canonical / NOT_RUN | [S02][S04][S07][S22] |
| `grok.web` | 声明 Responses 语义，实际 Web conversation 原生 wire | 文本 `canonical_bridge` 路径；若收到 reasoning，Chat 响应仍可能拒绝 / NOT_RUN | 文本 `canonical_bridge` 路径 / NOT_RUN | 文本原生语义路径 / NOT_RUN | [S02][S07][S23][S24][S43] |
| `kiro` | 声明 Messages 语义，实际 Kiro request + AWS EventStream | BLOCKED/S：Canonical-only 且 Reasoning profile | 原生 typed Canonical / NOT_RUN | BLOCKED/S：装配仅允许同协议 `Canonical` | [S02][S03][S10] |

**共同装配事实（PASS/S）**：三个 POST handler 使用共同 executor；Responses WebSocket、stored lifecycle、compact 和 Messages count_tokens 也有 HTTP 路由。[S11] Registry 固定绑定 8 种 adapter，多个目录渠道共用 adapter；native Grok 管理服务在部署组合根中接入，目录的初始 `import_available=false` 会随服务存在而打开，不能把初始值当成整渠道禁用。[S01][S12]

**能调用还依赖什么**：活动配置 → enabled Upstream/Endpoint → 合法 Candidate/transform → 目录硬资格或明确模型例外 → 有效凭据容量 → Public Model/Route → Client Key/Access Group。无活动配置会使用不可发送 executor；同一上游不同协议的 endpoint 不能互相替代。native pool 保存后还要有对应 endpoint/route，不能从账号列表反推可调度。[S13][S14][S15]

Grok Web 已有 production adapter 和实际执行分支，不应沿用早期“仅 fixture/无 transport”的描述。检查 `apps/gateway/src` 和其 Cargo.toml 未找到整渠道编译 feature 开关；是否具备生产配置、出口、签名/relay、有效 SSO 和准入仍未知，真实验收见历史表。[S23][S24]

## 2. 能力矩阵与协议损失

下表是运行时 capability 上限，**不是账号授权或真实模型能力**。第 1 节的协议准入规则作用于每一行；例如 Kiro 即便声明 WS，Messages-only 限制仍阻断其下游 Responses WebSocket。`JsonSchema` 用于工具输入 schema，不能据此声称结构化回答已支持。[S02][S10]

| 渠道/adapter 范围 | 流式 / function tools | 并行工具 | reasoning / thinking | Responses stored / compact / WS | 图片 |
| --- | --- | --- | --- | --- | --- |
| OpenAI Chat；Kimi API 默认 | 两者有声明与编解码路径 | 有声明，运行准入疑点见下文 | 没有 Reasoning 声明 | 否 / 否 / 有声明但需桥接条件 | 未声明 |
| OpenAI Responses；Codex；Kimi Coding | 两者有路径 | 有声明，准入疑点同下 | 有声明，跨协议及 native 历史限制 | 显式 opt-in / 显式 opt-in / 有声明 | 未声明 |
| Anthropic compatible；Claude | 两者有路径 | 有声明，准入疑点同下 | 有声明，签名/引用损失见下文 | 否 / 否 / 有声明但需桥接条件 | 未声明 |
| Grok Official | 两者有路径 | 有声明，准入疑点同下 | effort 子集；native search 无完整 Canonical 契约 | 否 / 否 / 有声明 | 未声明 |
| Grok Build | 两者有路径 | 未声明 ParallelTools | native reasoning 路径，受 ownership/历史约束 | 是 / 是 / 是，均受精确 lineage 限制 | 未声明 |
| Grok Console | 两者有路径 | 未声明 ParallelTools | effort 子集；不能从内置搜索推定可无损交付搜索结果 | 否 / 否 / 有声明 | 未声明 |
| Grok Web | 流式文本；生产 builder 拒绝 tools | 未声明 | 拒绝显式 thinking；decoder 仍可能读到上游 reasoning | 是 / 否 / 有声明 | 生产 builder 拒绝非文本 |
| Kiro | 两者有路径 | 未声明 ParallelTools | typed thinking 路径，当前只准 Messages | 否 / 否 / 有声明但被装配限制 | 未声明 |

当前具体边界（均 PASS/S 为“行为/限制存在”，实际请求验证 NOT_RUN）：

1. **入口和模式**：passthrough 仅同协议且保有 exact request body；canonical 仅同协议；lossless_bridge 仅跨协议；canonical_bridge 同/跨均可。passthrough 仍先经过 ingress decoder，响应也不能自动当成原文透传。[S03][S11]
2. **文本与工具历史**：九对投影已有有序 Text/ToolCall/ToolResult；Chat/Messages assistant 仅允许可选首个 text 后接 calls；Chat/Responses tool result 需单个字符串输出，且不能带 `is_error=true`。不是旧 ADR 所写的“历史工具一律不支持”。未知扩展、opaque、不能表达的内容仍拒绝。[S16][T02]
3. **Chat 子集**：消息文本只收字符串，不接受 content 数组/图片；`n=1`；legacy functions/function_call 拒绝；tool_choice 仅 auto/required；parallel_tool_calls 若提供须为 true。跨协议 tool_choice 另有限制，不能以入口解码成功推定出站成功。[S17][S16]
4. **结构化输出/多模态**：Responses 的 `text`、background、conversation、top_logprobs、stream_options 在解码阶段拒绝；Chat response_format 等保留为未知扩展，在严格 canonical 白名单下拒绝。Responses/Messages 的部分非文本输入可保留 opaque，但 canonical/bridge 拒绝；同协议保留不证明视觉推理或多模态输出通过。[S18][S16]
5. **Thinking 和 cache**：Responses 同协议支持特定 reasoning history/effort/summary/include/cache 参数；Messages thinking history 为 opaque，块级 cache 的跨协议投影仍受限。Responses↔Messages thinking 参数有映射，但 budget↔effort 是分段量化，例如 1000→low→1024；不能称数值无损。[S16][S19]
6. **已确认的输出损失**：Anthropic SSE `signature_delta`、`citations_delta` 被消费但不发出；buffered thinking 只保留文字。文件头的“signature fail closed”与该分支描述不一致。多轮 signed-thinking/citations 的实际客户端影响未验证；这是兼容性决策必须覆盖的已知损失，不是已完成的无损保证。[S20][S42]
7. **并行准入证据缺口**：实际 runtime 投影传 `requires_parallel_tools:false` 和 `requires_json_schema:false`；工具定义另会要求 Tools+JsonSchema，因此不能说 schema 完全不校验。Chat decoder 又移除 parallel_tool_calls。缺少证明“显式并行请求被严格按能力准入”的当前闭环，尚不把所有并行工具行为判为故障。[S07][S17][S16]
8. **usage 投影损失**：Chat/Responses 输出删去 Anthropic cache_read/cache_creation 细项；Messages 输出删去 reasoning/cached_tokens 细项。总量及计费证据需独立验证，不能把投影后的未知记为零。[S21]
9. **stored/compact/WS**：store 默认 false，持久化由网关负责，发给上游仍为 false；previous_response_id 需 client ownership 和精确 lineage，禁止隐式换账号/第二 attempt。compact 仅支持规定的 previous_response_id 形状与固定摘要请求。WS 仅 response.create，不支持 Realtime/binary；其下游 WS 不证明上游 WS。[T04][T05][S18]
10. **count_tokens**：HTTP 已注册，默认 `UnsupportedCountTokensExecutor`；检查全部 `apps/gateway/src` 的 count_tokens/TokenCount/ExactToken/token_counter 未发现生产替换，构建 state 时沿用默认。当前精确计数为 BLOCKED/S，返回不支持而不是估算；mock exact-counter 测试不代表 11 渠道有真实 tokenizer。[S11][S25][T06]

## 3. 接入、管理与恢复矩阵

本节 S 核对日期/版本同页首，真实运行状态均 NOT_RUN。backend prepare/import/binding 是独立步骤；前端负责组合。`beginConfigurationTask.finish()` 在已有草稿时保留草稿，在活动配置开始的 automatic 任务中才校验并发布。因而“已保存”“已连接”“已应用”“已验证调用”必须分开。[S26][S27][S28]

| 渠道 | 导入/授权与目标 | 绑定/生效 | 身份、额度、模型来源 | 续期与失效恢复 | 证据 |
| --- | --- | --- | --- | --- | --- |
| openai-compatible | API Key；必须先选已有服务，空时另开创建服务/接口流程 | 接口可稍后连接；当前草稿不自动发布 | key 无人类身份；配置 models_path 才建目录读取；无专属 quota reader 证据 | 更换 key / 禁用 / 修改绑定走配置；不属普通 OAuth refresh 集合 | [S09][S26][S29][S30] |
| anthropic-compatible | 同上，Messages preset | 同上 | 通用目录；官方 quota reader 仅收 OAuth，不把 API Key 当 OAuth | API Key 无到期 refresh | [S05][S26][S29][S30] |
| codex | JSON 或 OAuth；prepare 固定官方 target | 前端连接返回 credential ID，finish 决定草稿/应用 | 导入 claims 身份为显示信息；官方 models 与 wham usage 独立观测 | 后台有 enabled/active 精确绑定准入、CAS；另有手工 OAuth refresh、同身份重授权约束 | [S06][S26][S27][S30][S31] |
| claude | JSON/API Key 或 OAuth；prepare 固定 target | 与 Codex 同组合方式 | 导入/授权身份；官方 models；OAuth usage。quota parser 不借窗口刷新身份/套餐 | OAuth 后台 refresh；raw API Key 无 expiry、不 refresh | [S06][S27][S30][S31] |
| kimi-coding | OAuth 文件或设备码；prepare 支持多个专用服务显式选择，未知目标 conflict | 首次前端补缺失 binding，续授权保留；再 finish | models、me、usages 分别观测；身份成功不代表额度/推理成功；成功空额度不等于零余额 | 后台 OAuth refresh；pending/slow_down/denied/expired/failed 分开；不确定结果停止自动重放 | [S08][S32][S31][H04] |
| kimi-api | Moonshot Key；共用 API 流程，owner kind 为 kimi；不属 Kimi Coding OAuth | 服务前置，接口可延后；不是自动 prepare | 可配置兼容 models_path；Kimi OAuth 的 me/usages 不适用于此类 key，专属 quota/identity 缺证据 | Key 维护；不属普通 OAuth refresh | [S05][S09][S26][S29][S32] |
| grok.official | API Key；共用 API 流程 | 普通配置 credential/binding；不是 native pool | 专属 models 与推理 header quota；账号余额/身份不因导入而存在 | exact key unauthorized / quota / endpoint failure 分开 | [S26][S33] |
| grok.build | native JSON 或 device OAuth，无需预建普通服务 | native 保存后独立 rebuild，返回 runtime_applied；仍需对应 endpoint/route/model | identity/userinfo；动态 Build models；billing credits；过期 reader 不暗中 refresh | 实际 native refresh worker 为 Build；同 subject + revision 重授权；失败区分暂时/需重授权 | [S12][S15][S34][S35][S44] |
| grok.console | native SSO，无 device flow | 与 Build 同 native 边界 | session 身份；DPoP usage；六个 adapter model 名不是账号目录或 grant | 手动换 SSO + 身份/revision 校验；未在 apps/provider 范围证实自动 SSO reauth 接线 | [S15][S22][S35][S36] |
| grok.web | native SSO，无 device flow | 与 Build 同 native 边界 | session 身份；REST auto/fast 与可选 gRPC 局部 quota；四个静态 adapter model；推理 token usage 为估算 | 账号、出口、Statsig/clearance 失败分开；出口恢复不等于账号重授权 | [S15][S23][S24][S35][S36] |
| kiro | JSON/ksk_ 或 Builder ID；按 region prepare，混合 region 导入拒绝 | 前端补 binding，再 finish | IDE/CLI 动态目录，CLI 要唯一 ACTIVE profile；API Key catalog builder 明确拒绝；usage 可读部分 identity/plan/quota | Social/Enterprise 后台 refresh；API Key 不 refresh；设备授权失败不自动重试 | [S10][S26][S31][S37][S45][S46] |

共同恢复边界（PASS/S）：普通 OAuth 只有明确 grant 错误才进入持久 Unauthorized，HTTP 403/WAF/429/5xx 不能直接当作撤销授权；短暂失败有退避。quota 查询失败只保留安全错误/缓存，不自动等价于账号被禁用。目录刷新失败保留上次成功观测；UI 应区分“旧目录仍可见”和“刚刷新成功”。native enable/reset 只改变调度或请求恢复，不证明上游身份或余额恢复。[S30][S31][S36][S38]

## 4. 历史验收台账

**以下是历史报告的结果，不是当前现网复验。** 2026-09-27 的 11 入口 EgoLite fixture 和本地管理测试有 PASS；2026-09-28 的生产页面检查仅打开/取消表单，没有完整导入/授权/推理。所有渠道当前完整接入和真实三协议矩阵仍 NOT_RUN。[H01][H03]

| 渠道 | 最新相关 R 证据（限定已查报告） | 日期 / 版本 | 状态与不能外推的范围 |
| --- | --- | --- | --- |
| openai-compatible | Krill 的 Chat/Responses 目录和公开模型修正 | 2026-09-27；报告记录在当次生产修改，未给独立验收代码 SHA | 目录 PASS/R（该实例）；推理 NOT_RUN。归到此入口为按协议做的分类推断 [H05] |
| anthropic-compatible | 本次已查报告未定位独立真实推理证据 | 当前审计；历史具体 revision 未知 | R NOT_RUN/未知；不能移用 Claude fixture 结果 [T08] |
| codex | 旧隔离 staging 三协议 × JSON/SSE × text/tool 12/12；新 metadata 快照反而有 busy/authentication | 2026-08-09 staging，报告未记精确源码 SHA；2026-09-27 `333eae5` | 旧 staging PASS/R；后次目录 FAIL/R，quota BLOCKED/R。无最新成功重授权证据，不能用旧通过覆盖新失效 [H06][H02] |
| claude | metadata 发布验收当时无对应账号 | 2026-09-27 `333eae5` | BLOCKED/R（当时缺账号），不是源码“未实现”；推理 NOT_RUN [H02] |
| grok.official | 旧授权 probe 报告只运行默认安全 harness，live 延后；近期报告无补充成功 | 2026-07-23，独立 SHA 未记 | L 历史 PASS；R NOT_RUN。否定仅限已查报告 [T09] |
| kimi-coding | profile/身份、绑定、保存的 fresh 模型观测正常；quota 成功空对象 | 2026-09-27 `f28349406b7645572e85c95d411a769720092751` | PASS/R 元数据与空态语义；数值额度未观测，推理 NOT_RUN [H04] |
| kimi-api | 本次范围未定位独立真实元数据/推理证据 | 当前审计；历史具体 revision 未知 | R NOT_RUN/未知；Kimi Coding 结果不移植给 Moonshot Key [H01][T10] |
| grok.build | 两组“工具调用→结果回传→文本回答”四轮 Responses SSE；后次 quota 有成功亦有 unauthorized | 2026-09-27 `a715d6b952042d2e7c2775c0c7ab91895ee26e59`；metadata `333eae5` | 指定模型/账号/参数 PASS/R；不是所有账号或三协议 PASS。后次目录 stale，额度混合结果 [H07][H02] |
| grok.console | Responses JSON HTTP502 / CredentialUnauthorized；后次 quota unauthorized | 2026-09-26～27，M4 当次与后续 `333eae5` | FAIL/R（上游凭据拒绝）；不能直接归因为软件缺陷，当前是否恢复未知 [H07][H02] |
| grok.web | 旧受控 canary 有一次 canonical 文本生命周期；近期 metadata 验收缺账号 | 2026-07-23 G9，报告未记精确 closeout SHA；2026-09-27 `333eae5` | 旧文本 PASS/R；近期 BLOCKED/R（缺账号）。无当前三协议/工具成功证明 [T11][H02] |
| kiro | 旧替代凭据 probe 无成功样本；近期验收无对应账号 | 2026-08-02 历史，精确 SHA 未记；2026-09-27 `333eae5` | 旧 FAIL/R、原生凭据阶段 BLOCKED；近期仍 BLOCKED/R（缺账号），当前能力不以旧失败定性 [T12][H02] |

跨渠道历史层级：

- **L/B，2026-09-27**：接入报告以 `b6876e5` 为开发基线，修复提交 `5497f92` 后随新版发布。61 前端相关测试、41 管理 HTTP 测试、类型/构建通过；11 入口 fixture 使用合成凭据。Playwright runner 当次 NOT_RUN。不是 11 次真实 OAuth/推理成功。[H01][H03]
- **L/P，2026-09-28 / b0cf36e**：报告记前端 443、Rust 1389 passed/12 ignored、正式门槛和发布通过；生产 48 页面状态及接入表单打开/取消通过。完整接入、生产草稿发布及真实推理明确 NOT_RUN。此证据不自动重跑，也不代表今天运行状态。[H03]
- **L，2026-08-02**：已有三协议 × 四类渠道 loopback 矩阵，7 支持单元格 × 4 合成请求，以及 5 拒绝单元格 × 2，非 11 命名渠道完整矩阵。其 Grok Canonical-only 结论已被后续 canonical_bridge 实现扩展；保留为当次证据。[T01][S07]

已被后续证据覆盖的旧问题：Kimi not_connected 已有绑定修复，quota unknown 已明确为成功空态并发布；Codex/Claude/Kimi/Kiro 初次导入 binding、Claude models、Kiro CLI metadata 等早期未接线描述不能作为当前源码结论。对这些项目保留“真实完整接入仍未验收”，不重新列成已确认未修复缺陷。[H02][H04][H08]

## 5. 可交给后续决策的发现

| 编号 | 分类 / 状态 | 当前可证实的事实 | 决策归属 |
| --- | --- | --- | --- |
| F01 | 操作目标不满足，FAIL/S | 四类 API Key 入口必须选服务，无服务时要求先创建；接口可以稍后连接。用户拒绝的 Kimi API 前置流程仍存在，属于共用流程 [S26] | [无需预建服务与接口的接入及生效流程](https://github.com/Ricardo121380/cpa-rust-gateway/issues/4) |
| F02 | 反馈缺陷，FAIL/S；运行复现 NOT_RUN | prepare/import 前已有 workingId；失败或全拒绝仍 completed；关闭时不核对保存数量即显示“账号修改已保存”。逐条结果与顶层回执可能冲突 [S26] | [日常管理的状态解释与恢复操作](https://github.com/Ricardo121380/cpa-rust-gateway/issues/6) |
| F03 | 状态完整性风险，NOT_RUN | 首次 Codex/Claude callback 请求报错后没有“不确定已保存”专用状态；关闭仍可能 cancel pending。后端最终状态/损害未验证，不能直接称授权丢失 [S27] | 同上 |
| F04 | 反馈/流程缺口，PASS/S；体验验收 NOT_RUN | OAuth 完成文案未总是区分 draft/active；列表额度把多类失败压成未观测，详情才能解释。账号保存→目录→模型开放→Key授权→调用测试分散，缺统一完成回执 [S28][S39] | 接入流程及状态解释 |
| F05 | 支持范围限制，BLOCKED/S | Kiro 当前 Messages-only；Chat 受 Reasoning profile 与有限收窄规则约束；Web 生产仅文本子集。不能承诺 33 组合无差别支持 [S02][S03][S04][S10] | [三协议兼容与可靠性标准](https://github.com/Ricardo121380/cpa-rust-gateway/issues/5) |
| F06 | 已知转换损失 + 待验证风险 | signature/citations 被消费、thinking budget 量化、usage 细项丢弃已确认（PASS/S）；并行能力准入闭环及客户端多轮影响 NOT_RUN [S19][S20][S21][S07] | 同上：决定保留、拒绝或明示有损的精确语义 |
| F07 | 验收覆盖不足，NOT_RUN | 未定位 11 渠道 × 三协议的统一逐格验收；已通过的 fixture/历史协议类矩阵不能补成真实全部通过 [T01][H01] | 三协议标准与最终完成门槛 |
| F08 | 文档/测试入口漂移，FAIL/S | ADR-0037/0038 仍有早期协议范围文字；部分 native/channel authorization E2E 仍查旧 dialog/select，当前 chooser 是按钮。fixture 默认未开放 Codex/Claude 官方授权，不能用导入测试代替授权 [S40][S41][S47][S48] | 验收门槛；实现前修正规格/测试入口 |

F01/F02 是源码直接支持的目标或反馈缺陷；F05 是现有实现边界；F06 混合已知损失与待验证影响；历史账号拒绝另在第 4 节。这样后续修复不会把失效凭据当程序 bug，也不会把“暂未验收”写成“已支持”。

这些发现均落在已有接入、兼容性、状态解释决策内，本轮不另建重复实施工单，不改变 ADR、底层架构或产品承诺。下一步按地图顺序处理接入及生效流程；具体修复顺序和完整验收门槛由后续工单确定。

## 6. 复核范围与测试证据索引

采用七个独立只读检索范围：runtime 装配、公共协议语义、普通账号 lifecycle、native Grok lifecycle、前端流程、近期历史报告、协议/Provider 测试与契约。主线程亲读架构/相关 ADR，并抽查能力表、Reasoning gate/override、Kiro 装配、输出损失、错误回执、draft/active 分支及关键历史报告。未修改业务源码。

测试索引（均 TEST_PRESENT，本轮 NOT_RUN）：

| 证据 | 证明的范围 | 不证明的范围 |
| --- | --- | --- |
| 三协议九对请求/响应、四类渠道 HTTP loopback [T01][T02][T16] | typed 投影、JSON/SSE、拒绝前零 attempt | 当前 11 命名渠道全部真实成功 |
| Tool 分片/adversarial/cancellation [T03][T15] | 合成分片、终态、取消不变量 | Provider 真实事件完整性 |
| stored/continuity/compact/WS [T04][T05] | owner/lineage/持久化/下游 WS 的局部契约 | 任意 OpenAI 扩展、上游 WS、全部渠道连续性 |
| exact counter [T06] | 注入 exact counter、默认不支持、禁止估算 | 当前生产已接真实计数器 |
| Grok Build native reasoning [T07] | fixture 多轮、opaque ownership、JSON/SSE | Chat/Messages 的 native reasoning 无损透传 |
| Claude/Anthropic runtime [T08] | API Key/OAuth、loopback Tools/Thinking/错误 | 官方账号可用性 |
| Kimi 管理/目标分离 [T10] | 合成授权、拒绝重放、Kimi API/Coding 分离 | 两个渠道三协议真实推理 |
| Kiro adapter [T12][T14] | FixtureTransport/EventStream/tool lifecycle | 真实账号、任意 region/profile 可用 |
| Official/Console/Web [T09][T11][T13] | provider 子集、合成 decoder、单探针边界 | 全部账号/搜索工具/三协议完整成功 |

“未定位完整矩阵”的负面结论限于：已跟踪 `tests/`、`crates/*/tests/`、`docs/contracts/`、`docs/reports/` 文本及单独核查的 2026-09-26～28 渠道报告；没有全量读取报告附件或所有实现内测试。检索组合包含 `three_protocols_by`、11/eleven/33/三协议、渠道 ID、协议路径、count_tokens/websocket/stored/image/vision。来源不足的 cell 保留 NOT_RUN/未知。

本报告静态验证：`./scripts/check.sh docs` PASS（781 Markdown 文件、107 契约测试引用、131 计划任务及 canary/Caddy 静态检查、跟踪文件秘密扫描）；固定 SHA 引用/行号和 11 × 3 表格完整性检查 PASS，最终 diff 仅此报告。业务行为、浏览器、真实 Provider 和生产验证保持 NOT_RUN。

## 固定版本证据

[S01]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/runtime.rs#L2051-L2150
[S02]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/runtime.rs#L584-L631
[S03]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-router/src/protocol_transform.rs#L203-L295
[S04]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/runtime.rs#L3476-L3509
[S05]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/web/prism/src/features/upstreams/connectionPresets.ts#L1-L14
[S06]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-http-actix/src/management_resources/account_channels.rs#L142-L171
[S07]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/runtime.rs#L4345-L4403
[S08]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-http-actix/src/management_resources/account_channels.rs#L199-L324
[S09]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-http-actix/src/management_resources/account_channels.rs#L960-L1102
[S10]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/runtime.rs#L3397-L3419
[S11]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-http-actix/src/lib.rs#L1068-L1081
[S12]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/deployment.rs#L548-L589
[S13]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/runtime/reload.rs#L140-L191
[S14]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-control/src/route_compiler.rs#L870-L965
[S15]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/provider-grok/src/account_pool.rs#L1043-L1113
[S16]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-router/src/protocol_transform.rs#L346-L1006
[S17]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/protocol-openai-chat/src/lib.rs#L63-L174
[S18]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/protocol-openai-responses/src/lib.rs#L207-L727
[S19]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-router/src/protocol_transform.rs#L564-L659
[S20]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/protocol-anthropic/src/upstream_response.rs#L737-L755
[S21]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-router/src/response_transform.rs#L121-L155
[S22]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/provider-grok/src/console_responses.rs#L419-L492
[S23]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/provider-grok/src/web_production.rs#L283-L408
[S24]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/runtime.rs#L4962-L5053
[S25]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-http-actix/src/lib.rs#L425-L510
[S26]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/web/prism/src/features/accounts/AddAccountDialog.tsx#L24-L269
[S27]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/web/prism/src/features/accounts/AuthorizationCodeDialog.tsx#L26-L89
[S28]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/web/prism/src/features/config-versions/configurationTask.ts#L15-L66
[S29]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/runtime.rs#L3658-L3683
[S30]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/runtime/account_quota.rs#L18-L155
[S31]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/credential_refresh/ordinary.rs#L21-L470
[S32]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/runtime/kimi_metadata.rs#L12-L119
[S33]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/provider-grok/src/official_runtime.rs#L1-L95
[S34]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-http-actix/src/management_resources/native_accounts.rs#L331-L443
[S35]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/native_account_usage.rs#L123-L475
[S36]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/provider-grok/src/account_pool/maintenance.rs#L13-L157
[S37]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/provider-kiro/src/catalog.rs#L23-L154
[S38]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/runtime/catalog_refresh.rs#L34-L66
[S39]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/web/prism/src/features/accounts/AccountsPage.tsx#L56-L150
[S40]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/adr/ADR-0037-protocol-transform-admission.md#L20-L56
[S41]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/web/prism/e2e/native-accounts.spec.ts#L14-L50
[S42]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/protocol-anthropic/src/upstream_response.rs#L1-L165
[S43]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/provider-grok/src/web_live.rs#L214-L227
[S44]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/apps/gateway/src/credential_refresh.rs#L726-L760
[S45]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/provider-kiro/src/account_usage.rs#L25-L50
[S46]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-http-actix/src/management_resources/account_quota.rs#L96-L109
[S47]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/web/prism/src/dev/fixtures.ts#L1668-L1681
[S48]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/adr/ADR-0038-endpoint-format-isolated-protocol-routing.md#L21-L39
[T01]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/p12-08f2-three-protocol-four-channel-loopback.md#L5-L59
[T02]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/p12-08d1-three-protocol-request-projection.md#L25-L63
[T03]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/protocol-anthropic/tests/p5_08_adversarial_properties.rs#L91-L128
[T04]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/p13-09c-exact-continuity-and-compaction.md#L25-L86
[T05]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/contracts/BC-RESP-004-public-responses-websocket.md#L14-L105
[T06]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/p5-02-exact-token-count-capability.md#L7-L63
[T07]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/provider-grok/tests/native_output_fidelity.rs#L61-L147
[T08]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/p12-08e2-anthropic-compatible-runtime.md#L30-L85
[T09]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/p8-07-authorized-official-probe.md#L7-L45
[T10]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/gateway-http-actix/tests/managed_resource_inventory.rs#L2993-L3300
[T11]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/g9-gate-report.md#L7-L42
[T12]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/p12-06-kiro-channel-evidence.md#L7-L150
[T13]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/provider-grok/tests/p12_10e_console_web_runtime.rs#L204-L335
[T14]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/crates/provider-kiro/tests/p7_09_native_inference_adapter.rs#L32-L173
[T15]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/g5-gate-report.md#L30-L42
[T16]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/p12-08d2-three-protocol-response-projection.md#L25-L67
[H01]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/cpar-channel-onboarding-repair-20260927.md#L3-L50
[H02]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/cpar-channel-metadata-production-20260927.md#L5-L38
[H03]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/prism-polish-production-20260928.md#L5-L62
[H04]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/cpar-kimi-quota-release-20260927.md#L5-L43
[H05]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/cpar-krill-alignment-20260927.md#L5-L31
[H06]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/p12-codex-runtime-final-review.md#L3-L55
[H07]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/cpar-reliability-m4-20260926.md#L67-L119
[H08]: https://github.com/Ricardo121380/cpa-rust-gateway/blob/9fc4ceb4e12aab9c60658869706828a7d5d8312f/docs/reports/cpar-channel-repair-20260927.md#L3-L89
