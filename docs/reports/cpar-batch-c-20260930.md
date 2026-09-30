# CPAR C 批次修复、渠道实测与阻断台账 — 2026-09-30

## 结论与范围

**整体 C：BLOCKED。** 已完成可独立验证的运行装配和语义修复，取得受控公共入口证据及现有账号的真实调用证据；Kiro 的原生输出上限、Grok Web 的原生多轮连续性、部分原生响应 metadata 和逐渠道已声明扩展仍未达到冻结规格。没有关闭 CPAR-30–41 / #39–50，也没有调整其必过线。

- 来源：[实施规格 #9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)，本地冻结副本 `output/cpar-reliability-spec-20260929/spec.md`；工单副本 `output/cpar-reliability-tickets-20260929/published/30.md`–`41.md`。
- Checkout：`codex/prism-v4-delivery`；固定比较起点 `45a315a6809eff5ba75ad47c53fe1825a88f7da9`（B 批次）。实现源码检查点及 Full 结果在下方门禁段固定。
- 用户授权已有账号渠道真实验证、无调用或费用上限，并要求参考 kiro-rs / grok2api 源码。此授权用于已有账户推理，不包含生产部署、配置发布、服务启停、新账号或扩大客户端权限。
- 本地公共工厂：**PASS，6 个行为测试，298 次成功请求及 15 次拒绝／不重发请求**。受控上游、合成凭据和当前源码不证明真实 Provider 可用。
- 当前线上旧构建：56 次公共 HTTP 验收请求及 6 次受保护 channel-pin，合计 **62 次验收请求**。重复验证包含在计数内；准入拒绝没有上游调用，不能把 62 全部称为 Provider 推理次数。
- EgoLite 产品验收：**NOT_RUN / 不适用**，此切片未改变浏览器产品流程。公开协议由 Actix 公共 HTTP 入口、实际 runtime factory 和受控原生传输验证；未声称浏览器或新构建生产验收通过。
- 新构建真实渠道、Linux 发布产物和部署：**NOT_RUN**。线上进程与活动配置未变；不以旧构建实测接受新源码。

## 实现变化

| 行为 | 修改与验证 |
|---|---|
| Kiro 运行装配 | 接受现有 CanonicalBridge，使用客户端源协议作 Canonical 投影边界；不再因装配与执行支持模式不一致提前拒绝。纯 Kiro 请求构造在租约前校验，删除曾静默丢弃 `max_tokens` 的投影。 |
| Kiro 工具、思考与终态 | `tool` 角色仅允许已关联 ToolResult，保持 ID、值、错误状态和次序；IDE/CLI 保留各自定性 effort 字段，仅允许 low/medium/high/xhigh/max。数值 budget 和 enabled/disabled 不近似映射。完整 CRC/EventStream 和语义 EOF 后产生 tool_use/end_turn；不完整工具仍失败。 |
| Kiro CLI profile | 完整目录观测提供 OAuth、地区校验的 profile；内存快照固定 endpoint、credential ID、revision，过期时间不超过凭据到期及一小时。轮换或旧 generation 的观测不能附着到新材料；API key 不冒充 OAuth 目录。无观测时保留原有已审定 fallback，已有但失效的观测拒绝。 |
| 普通渠道与 Codex | Codex 公共 JSON 响应独立于其原生 SSE 传输；删除收到 400 后去掉客户端输出上限、用同一租约重发的兼容路径。Kimi Chat 使用共同 transport headers，保持原始已准入目标和设备身份；Codex account identity 与 Kimi device identity 不合并。 |
| Grok Console 请求控制 | 保留且校验 max_output_tokens、tool_choice、parallel_tool_calls 和关联工具历史；移除未经客户端请求的默认 web_search/x_search 工具及默认工具选择。 |
| Grok Build / Official / Console 工具参数 | JSON 或仅有最终参数的响应补齐参数 delta；零输入归一为对象的既有窄契约保持一致。非空参数原字节有序保留；Chat 最终 JSON 比较只忽略外层 JSON 空白，不改变非空参数值。 |
| Grok Official quota | 原生适配器与运行时共用精确 endpoint/credential 的 quota registry；额度耗尽后的第二次请求不再次发送。 |
| Grok Official / Console 响应保真 | 非空 citations/annotations、logprobs、summary、签名及 encrypted content 无完整表示时明确失败。完整最终 item 快照必须匹配已完成 item；既有仅含 id 的稀疏终态只确认已完成 item。失败不产生成功终态。拒绝通过不等于这些 metadata 已支持。 |
| Grok Web 已知请求问题 | 仅一个 user 文本消息可直接构造；顺序文本保留空白，不拼接角色标签或模拟工具。多角色／多轮、输出上限和工具等已知不可表示请求在租用 native 账号、DNS 或发送前拒绝。原生多轮尚未实现，继续阻断。 |

实现沿用既有管理、编译、调度、lease、Canonical 和 Provider 模块；测试端口只在 `cfg(test)` 生效，生产固定 URL 策略未改。没有 schema、管理 OpenAPI、客户端权限或前端业务变更。Kiro 的 [ADR-0048](../adr/ADR-0048-kiro-profile-arn-lifecycle.md)、[ADR-0052](../adr/ADR-0052-kiro-semantic-tool-thinking-mapping.md) 及对应 BC-PROVIDER-006/010 已补充 C 行为；没有新增能力例外。

## 确定性本地证据

方法：`CARGO_NET_OFFLINE=true cargo test --locked -p gateway runtime::batch_c --all-features`，2026-09-30 macOS 本 checkout；**6 passed / 0 failed**。严格四 crate Clippy（gateway、provider-grok、provider-kiro、protocol-openai-chat，all-targets/all-features，`-D warnings`）**PASS**。

入口：[factory fixtures](../../apps/gateway/src/runtime/batch_c.rs)、[ordinary HTTP peers](../../apps/gateway/src/runtime/batch_c/http.rs)、[Grok native peers](../../apps/gateway/src/runtime/batch_c/grok.rs)。每个 fixture 使用独立临时 SQLite 和合成账户，经实际 ManagementService bootstrap、deployment compiler、P12 runtime factory、Client-Key 鉴权、实际 scheduler/lease 和公共三协议 codec 执行。公开模型 `p12-test-model`；合成配置由 `p12_configuration_for(..., "batch-c")` 生成；所有 native 账号与普通凭据分别沿用其真实存储/装配路径。

评审后补强了 [public response oracle](../../apps/gateway/src/runtime/batch_c/public_response.rs)：所有 298 次正例都完整解析 JSON/SSE，校验线上的唯一成功终态、无错误／截断、工具 ID/name、参数对象与完成状态，及回传轮次后的精确文本。Responses/Chat 的原生字段使用严格 wire codec 校验；公开 `cpar_usage` 的六个 nullable 源计数和两个 evidence 枚举独立校验后，只在测试用解码视图中处理。Messages 使用符合公开缺失计数契约的内容／流序校验；未能精确转换的输入 aggregate 保持 absent，不填造计数。该工具／终态断言不等于完整 Usage→ledger 的逐渠道验收。

每个正例的四轮依次为工具 call-c、回传 result-c 后文本回答、新 user 轮次产生 call-d、回传 result-d 后文本回答；保留完整历史。普通和 Grok 正例每请求上限 19，校验原生 outbound 字段与工具历史。此断言证明字段保留，不证明真实上游硬上限兑现。

| 实际工厂／连接策略 | 模型／合成凭据 | 下游与本地结果 | 请求数 |
|---|---|---|---:|
| openai-compatible Responses，gateway.example.test/v1 | upstream-c / API key | Chat、Messages、Responses × JSON/SSE × 四轮 PASS | 24 |
| anthropic-compatible Messages，api.anthropic.com/v1 | upstream-c / API key | 同上 PASS | 24 |
| Codex Responses，chatgpt.com/backend-api/codex | upstream-c / Codex OAuth | 同上 PASS；原生 SSE 可投影公共 JSON | 24 |
| Claude Messages，api.anthropic.com/v1 | upstream-c / Claude OAuth | 同上 PASS | 24 |
| Kimi Coding Responses，api.kimi.com/coding/v1/responses | upstream-c / Kimi OAuth | 同上 PASS | 24 |
| Kimi Coding Chat，api.kimi.com/coding/v1/chat/completions | upstream-c / Kimi OAuth | 同上 PASS | 24 |
| Kimi API 配置 Chat，api.moonshot.cn/v1/chat/completions | upstream-c / API key | 同上 PASS | 24 |
| Kimi API 配置 Responses，api.moonshot.cn/v1/responses | upstream-c / API key | 同上 PASS；合成端点不能证明厂商有此 API | 24 |
| Grok Official，api.x.ai/v1/responses | grok-4 / API key | 同上 PASS | 24 |
| Grok Build，cli-chat-proxy.grok.com/v1/responses | grok-4.5 / native Build OAuth | 同上 PASS | 24 |
| Grok Console，console.x.ai/v1/responses | grok-4.3 / native SSO，合成 DPoP bootstrap | 同上 PASS | 24 |
| Kiro IDE，q.eu-west-1.amazonaws.com/generateAssistantResponse | claude-sonnet-4.5 / Social OAuth | Chat、Responses × JSON/SSE × 四轮、high effort PASS；不带输出上限 | 16 |
| Kiro CLI，runtime.us-east-1.kiro.dev/ | 同上；注入合法、精确绑定的目录 profile 快照 | 同上 PASS；不是完整真实 CLI 目录→推理闭环 | 16 |
| Grok Official 精确额度状态 | grok-4 / API key | Responses JSON/SSE 各首次成功 | 2 |
| Kiro 不能表达的输出上限 | 同上两个地区／IDE/CLI | 8 次租约前拒绝、零新增 attempt/发送 PASS | 8（负例） |
| Grok Web 已知不可表示请求 | grok-chat-fast / synthetic native SSO | 4 次拒绝，零 lease、DNS/发送 PASS | 4（负例） |
| Codex 上限重发边界 | upstream-c / Codex OAuth | 受控 400 仅一次发送，仍带 max_output_tokens=19 PASS | 1（负例） |
| Grok Official 额度耗尽 | 同上 | 2 次再次请求不成功、零额外发送 PASS | 2（负例） |

普通 HTTP peer 只在已完成原始目标准入后，通过测试端口接到 loopback；记录原始 URL/body/headers，实际 HTTP client 与公开 codec 未替换。Kiro/Grok native 则替换现有注入 transport；不等于完整部署进程或真实 TLS 对端验收。新 factory 覆盖 Kiro Social OAuth；其他凭据种类及全部地区的 factory 闭环 **NOT_RUN**，既有 Provider 单测只作回归。

先失败再修复的集中回归包括 Console controls、Kiro ToolResult/effort、Codex 保留上限且不重发、native SSE→公共 JSON、native 工具最终参数 delta、Chat 外层 JSON 空白、Official/Console metadata 和快照。见 [Console tests](../../crates/provider-grok/tests/cpar_c_console_controls.rs)、[Kiro tests](../../crates/provider-kiro/tests/cpar_c_conversation.rs) 及 factory 测试；最后全量门禁再次运行这些测试。历史失败日志未作为独立附件保存，不能宣称附有完整红绿日志档案。

## 真实渠道证据：旧线上构建

实际环境 `new-vps` / instance-20260726-2136；现有 systemd gateway PID 499070，`/proc/499070/exe` 指向 `/opt/cpa-rust-gateway/releases/b0cf36e387b774439acc23830221389eb3ac74d0/gateway`。活动配置 `kimi-binding-a65aff19-302e-471c-846c-2ed3a6a4af75`，`rev-1`。公开 API `https://cpar.142857142.xyz/v1`；保护管理端口为既有 loopback 18181。推理使用既有 Pi CPAR Client-Key；管理回读及 pin 使用既有机器认证，凭据只在内存中使用，附件没有凭据或请求／响应正文。

| 回执／时间 UTC | 验收请求及结果 | 允许的结论 |
|---|---|---|
| [首次公共 HTTP](assets/cpar-batch-c-20260930/live-existing-runtime.json)，08:32:04–08:33:37 | 40 请求，12 PASS / 28 FAIL；Build 16、Codex 12、Console 12 | Build Responses/Messages 文本与工具回传可用；初版工具断言不检查具体值，因此精确工具值结论采用后续独立重跑。 |
| [Build 严格重跑](assets/cpar-batch-c-20260930/live-build-attribution.json)，08:46:36–08:47:20 | 16 请求，12 PASS / 4 FAIL；核验唯一非空 call ID、lookup 参数精确为 `{value:1}`、对应结果回传后文本及成功终态 | Responses/Messages × JSON/SSE 的代表性文本、工具调用和一次结果回传闭环 PASS。Chat 文本／工具 × JSON/SSE 均 400 ClientRequestError。没有连续两个工具周期或全部扩展验收。 |
| [运行 inventory 与请求归属](assets/cpar-batch-c-20260930/live-inventory-and-attribution.json) | 同一重跑窗口 16 个 ledger 请求：12 succeeded、各一 attempt；4 Chat failed、零 attempt；无后续页 | 成功实际 endpoint 为 Build，credential 脱敏标识 319bde2905a6。HTTP 未提供 x-request-id，以独占窗口、模型／协议／stream、时间和顺序关联；不是响应 request-ID 的直接关联。 |
| [Krill exact channel-pin](assets/cpar-batch-c-20260930/live-krill-channel-pin.json)，09:13:48–09:13:54 | 6 请求，全部 FAIL。Responses JSON/SSE 各一次 upstream_sent，在 decoder 失败；Chat/Messages JSON/SSE 均零 attempt 准入拒绝。管理 HTTP 均 200。 | HTTP 200 仅是返回诊断回执。固定短文本、上限 8 的 probe 不是工具闭环。前后 PID、实际 binary 和活动配置 revision 一致。 |

公共脚本：[cpar-batch-c-live.py](../../scripts/acceptance/cpar-batch-c-live.py)。模型来自该 Client-Key 实际 `/models` 视图（gpt-5.6-terra、grok-4.5、grok-4.20-0309）；每次请求上限 64，但旧构建的约束兑现标为 **UNVERIFIED_EXISTING_RUNTIME**。没有用单次用量低于 64 证明硬上限。Krill pin 沿用 [ADR-0088](../adr/ADR-0088-channel-pin-diagnostic-execution.md) 的实际 serving 路径，不创建临时路由或扩权。

### 当前账号与连接阻断

| 渠道 | 当前脱敏账号证据／revision | 当前真实结果与阻断 |
|---|---|---|
| Grok Build | 319bde2905a6 / 92；a6370161014a / 未记录；78f9a5c88de7 / 98 reauth_required | 仅第一账号由推理 ledger 证明成功。其他账号未逐一 pin；native pin 不支持，不能把池中的所有账号标 PASS。 |
| Grok Console | 60c641a66eb5、a6d65643d5ac；revision 未记录 | 首两个 Responses 请求 CredentialUnauthorized，后续已记录 CredentialUnavailable；Messages 四条未提取安全闭合错误码。失败分类与实际根因分开，不能据此推定每份 SSO 的具体失效原因。 |
| Codex | 071b294bce46 / 1 | 12 次公共请求 FAIL；8 条提取 CredentialUnavailable，Messages 四条未提取错误码。未尝试恢复／轮换账号。 |
| openai-compatible / Krill | 5afbb699ec53 / 0 | exact pin 确认选择 Krill Responses endpoint、route-0b9a2bb7-3f85-487a-bba2-0871bacc46ca、gpt-6-sol；Responses decoder FAIL，另四项旧桥接准入拒绝。 |
| Kimi Coding | 88c42108fcd4 / 593 | 已有 OAuth 和 endpoint，但活动 graph 无 enabled route；推理 BLOCKED。建立／发布生产 route 超出本次账号调用授权。 |
| Kiro、Claude、Grok Official、Grok Web、Kimi API、独立 Anthropic-compatible | 本轮 serving inventory 未见对应账号 | 实际推理 BLOCKED；没有创建账号，合成 fixture 不填补真实账号证据。 |

Inventory 是本轮服务视图，没有宣称全设备、所有归档 credential 或所有用户环境都不存在账号。两份 Console 和一份 Build revision 为 null，是回执没有记录该值，不能推定 revision=0。调用引起正常 usage/lease/health/quota 和诊断审计记录；没有配置发布、权限修改、服务重启或部署。

## 参考实现与仍存在的软件阻断

参考源码均冻结到不可变 SHA。它们用于理解现有原生行为，不能代替 CPAR 的语义契约或实际 Provider 验收。

### Kiro 原生输出上限

参考 `hank9999/kiro.rs`，SHA `e09625c3e7dea40aa9463d517e63f916828ef92b`：[公共 Messages 类型](https://github.com/hank9999/kiro.rs/blob/e09625c3e7dea40aa9463d517e63f916828ef92b/src/anthropic/types.rs#L117-L130) 接受 max_tokens；[converter](https://github.com/hank9999/kiro.rs/blob/e09625c3e7dea40aa9463d517e63f916828ef92b/src/anthropic/converter.rs#L623-L641) 使用 thinking prompt tags；检查该转换文件未发现 max_tokens 转为硬输出上限，[原生 conversation 类型](https://github.com/hank9999/kiro.rs/blob/e09625c3e7dea40aa9463d517e63f916828ef92b/src/kiro/model/requests/conversation.rs#L14-L30) 的已检查字段也没有硬输出上限。

这是“所检查参考实现未证明可用原生字段”的结论，不是所有 Kiro 服务永远不可能支持上限的断言。CPAR 不删除 Messages 必填 max_tokens，也不使用提示词预算作为等价 hard cap。因此 Messages 的全部正例和其他协议带显式输出上限的组合仍 **SOFTWARE BLOCKED**；无上限 Chat/Responses 的本地通过不能关闭 CPAR-30/41。新增字段需要原生协议证据及真实 hard-cap 验证。

### Grok Web 原生多轮连续性

参考 `chenyme/grok2api`，SHA `7c889a960e2638341b4dae9a5c81af0e0f38c87f`：[gateway](https://github.com/chenyme/grok2api/blob/7c889a960e2638341b4dae9a5c81af0e0f38c87f/backend/internal/infra/provider/web/gateway.go#L244-L248) 固定原 conversation，[session continuation](https://github.com/chenyme/grok2api/blob/7c889a960e2638341b4dae9a5c81af0e0f38c87f/backend/internal/infra/provider/web/gateway.go#L327-L367) 使用 conversation_id、load_existing 和 parent_response_id；[存储状态](https://github.com/chenyme/grok2api/blob/7c889a960e2638341b4dae9a5c81af0e0f38c87f/backend/internal/infra/provider/web/chat.go#L646-L655) 绑定 account、conversation 和 upstream parent response。

这提供了原生续接线索。CPAR 当前仅 temporary new conversation 发送路径，没有对应账号／原生 conversation／parent 的完整公共多轮 transport 和成功 lineage。参考的 [input normalization](https://github.com/chenyme/grok2api/blob/7c889a960e2638341b4dae9a5c81af0e0f38c87f/backend/internal/infra/provider/web/chat.go#L658-L711) 还把多角色消息及工具格式化为文本；CPAR 冻结规格不接受这种角色或工具模拟作为等价基础路径。三协议多轮文本及 Web Messages 输出上限仍 **SOFTWARE BLOCKED**，不是已接受的新能力例外。

### 原生响应 metadata

Official/Console 当前增量映射不能完整保留 summary、encrypted reasoning、签名、引用及 logprobs。现已明确失败，避免丢失后假成功；这些具体能力继续 **SOFTWARE BLOCKED**。plain reasoning、原生加密推理和 summary 是不同语义，不能用其中一个样例代替其他能力。其他通用 codec 的保真回归不证明该 native adapter 已完整支持。

## 工单与扩展完成台账

`基础局部 PASS` 只指上表的合成工厂行为范围。对每个渠道，取消、重试、保存、续接、压缩、WS 和完整 Usage→ledger 的逐渠道组合未在本轮新增穷尽证据；B 的共性回归可以复用，但不能替代本渠道的声明／账号／模型闭环。

| 工单 | 基础本地状态 | 当前旧构建真实状态 | 本票整体 |
|---|---|---|---|
| CPAR-30 / #39 Kiro 装配 | Chat/Responses 无上限局部 PASS；Messages 输出上限 BLOCKED | 无本轮 Kiro 账号 BLOCKED | BLOCKED |
| CPAR-31 / #40 openai-compatible | 三协议 JSON/SSE 四轮 PASS | Krill 6 pin FAIL，非完整工具验收 | BLOCKED |
| CPAR-32 / #41 anthropic-compatible | 同上 PASS | 无独立对应账号 BLOCKED | BLOCKED |
| CPAR-33 / #42 Codex | 同上 PASS；上限拒绝不重发 PASS | 当前账号组合 FAIL | BLOCKED |
| CPAR-34 / #43 Claude | 同上 PASS | 无对应账号 BLOCKED | BLOCKED |
| CPAR-35 / #44 Grok Official | 同上 PASS；额度归属 PASS；metadata BLOCKED | 无对应账号 BLOCKED | BLOCKED |
| CPAR-36 / #45 Kimi Coding | 两种配置上游各三协议 PASS | 有账号，无 enabled route BLOCKED | BLOCKED |
| CPAR-37 / #46 Kimi API | 两种合成配置各三协议 PASS，真实 API 存在性未证明 | 无对应账号 BLOCKED | BLOCKED |
| CPAR-38 / #47 Grok Build | 三协议 JSON/SSE 四轮 PASS | Responses/Messages 代表闭环 PASS；Chat FAIL；新构建 NOT_RUN | BLOCKED |
| CPAR-39 / #48 Grok Console | 三协议 JSON/SSE 四轮 PASS；metadata BLOCKED | 当前账号组合 FAIL | BLOCKED |
| CPAR-40 / #49 Grok Web | 已知不可表示请求拒绝 PASS；多轮正例 BLOCKED | 无对应账号 BLOCKED | BLOCKED |
| CPAR-41 / #50 Kiro | 两地区／IDE/CLI、Social、定性 effort 局部 PASS；其余 factory 凭据种类 NOT_RUN；Messages BLOCKED | 无对应账号 BLOCKED | BLOCKED |

| 渠道声明 | 本轮逐渠道扩展结论 |
|---|---|
| OpenAI-compatible、Codex、Kimi Coding | reasoning / parallel / WS，以及适用配置下 stored / continuation / compact：完整逐组合本地与真实闭环 NOT_RUN。 |
| Anthropic-compatible、Claude、Grok Official | reasoning / parallel / WS：完整逐组合 NOT_RUN；Official 的不可表示原生 metadata 如上 BLOCKED。未声明 stored/compact 不自动承诺。 |
| Kimi API | 依实际配置与模型声明登记；本地合成 Chat/Responses 选择不证明真实厂商能力；扩展完整闭环 NOT_RUN。 |
| Grok Build | reasoning / stored / continuation / compact / WS：完整逐组合 NOT_RUN；未声明 parallel 不自动承诺。 |
| Grok Console | reasoning / WS：完整逐组合 NOT_RUN；不可表示原生 metadata BLOCKED；未声明 parallel/stored/compact 不自动承诺。 |
| Grok Web | stored / continuation / WS：完整逐组合 NOT_RUN，原生连续性本身 BLOCKED；未声明 reasoning/parallel/compact 不自动承诺；工具限制仍按原规格保留。 |
| Kiro | reasoning / WS：high 定性 effort 的无上限局部 PASS，完整 reasoning 输出/历史和 WS 逐渠道组合 NOT_RUN；未声明 parallel/stored/compact 不自动承诺。 |

既有 capabilities 未关闭以获得绿色结果。无账户或当前授权失效是环境阻断；Kiro hard cap、Web continuity 和 native metadata 是软件／协议阻断；旧构建 Chat 400 和 decoder 错误是实际失败；新源码未部署为 NOT_RUN，四者不混写。

## 正式门禁与审查

初轮实现检查点 `bc9b2b5ac62c27acc62a4f3bf53b1806e85522ea` 的 Full：**PASS，44/44，Rust 1,431 passed / 0 failed / 12 ignored**，2026-09-30 10:13:50–10:18:08 UTC。见 [initial Full receipt](assets/cpar-batch-c-20260930/initial-full-check.md) 和 [immutable summary](assets/cpar-batch-c-20260930/initial-full-summary.json)。它证明评审前检查点；后续有行为修复和更强断言，不能直接作为最终源码 Full。

最终实现源码检查点：**待冻结**。最终 Full：**NOT_RUN（待修复后执行）**。命令：`CARGO_NET_OFFLINE=true CHECK_REPORT_PATH=docs/reports/assets/cpar-batch-c-20260930/full-check.md bash scripts/check.sh full`。

Standards / Spec 双轴固定点审查：比较 `45a315a...bc9b2b5`，两名独立只读探子均返回终态。见 [separate axis reports](assets/cpar-batch-c-20260930/review.md)。

| 审查轴 | 发现、处置与覆盖 |
|---|---|
| Standards | 已声明生产增量／测试结构范围 complete；0 文档硬违规。1 个低风险 Duplicated Code 判断：保留不同 native decoder 的所有权与限额，未扩大为状态机重构。1 项证据强度问题：公共成功断言已补强如上。未完整读每条 inventory/pin JSON、未改动代码和外部参考。 |
| Spec | Coverage PARTIAL，完成判定 FAIL / C BLOCKED。新增 1 个 P2：文本 delta/done 事件级 logprobs 未校验；主线程先执行失败复现，再在处理文本前校验。Official/Console 两类事件拒绝，四项集中 Provider 回归 PASS。已有 hard-cap、Web continuity、metadata、扩展及真实新构建要求继续阻断，没有改为能力支持。 |

评审后主线程承担源码复核与运行验证；没有声称后续修改已经获得完整独立 Spec 审查。正式本地门禁通过也不关闭仍未满足的业务完成条件。

用户已有 AGENTS.md 改动及其他未跟踪产物保留；仅精确暂存本批次文件。没有向 GitHub 发消息、改工单状态或 push。已有账号验证授权已使用；剩余的新构建实测需要独立的受控发布环境或生产部署授权，Kimi 路由发布需要明确共享配置变更范围，而软件阻断仍须实现或由用户明确调整未完成范围。
