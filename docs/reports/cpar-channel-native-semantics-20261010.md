# CPAR 渠道原生语义证据（2026-10-10）

本轮核对源码基线 `d3d2e3c5bf0d9521f3fb0d830344b34053f62155`；T12 驱动提交为 `611949b6faef9788d04b3283cc4c7ad45c6dcf38`，最终行为与验收源码为 `c1b304d02e61fe1c75653a32fba920093462add6`。后者修复旧 Usage 的严格读取／重放兼容，并补充对应事件存储契约；原生 Kiro/Web builder、decoder、能力声明及页面没有变化。本原生语义结论为 **PARTIAL / BLOCKED**，不把有限来源中的字段缺失解释为上游永久不支持。

来源版本、URL、实际下载 SHA256、字节数及观察 UTC 见 [一手来源索引](assets/cpar-channel-delivery-20261010/primary-source-index.json)。AWS 文件是一手生成 SDK 定义；grok2api 文件是冻结参考实现作者的原代码，不是 xAI 官方协议承诺。未读取真实账号材料，未发送真实 Provider 请求。

## A：Kiro IDE／CLI 硬上限

| 待证明的语义 | IDE 来源 | CLI 来源 | 当前结论 |
| --- | --- | --- | --- |
| 原生字段、合法范围、单位 | AWS Toolkit 生成模型和 serializer：`@amzn/codewhisperer-streaming`，冻结 `e1b0f9bc573bac1830f0b9aa7ced25a6c69bb477`；请求公开字段为 conversationState/profileArn | Q CLI 生成 input，冻结 `15cc8f3cd18c4272925ce1c7053268eedff1ea0a`；conversation_state/profile_arn/agent_mode | 未证明当前 IDE／CLI 原生 hard cap 的字段或范围 |
| reasoning 与可见输出如何计数 | 所查定义没有对应计数契约 | 所查定义没有对应计数契约 | NOT_PROVEN |
| 命中上限的终态／截断标识 | 现有 Kiro EventStream 白名单及 tool_use/end_turn 不提供已验证的 cap 终态 | 同左 | NOT_PROVEN |
| 当前端点、凭据和地区适用性 | 所查 SDK 是 AWS CodeWhisperer/Q 系列定义，不能自动外推当前所有 Kiro IDE 端点 | 官方 Q CLI README 明确新 Kiro CLI 为闭源；旧 Q SDK 不是当前 kiro.dev 原生定义 | NOT_PROVEN；需要当前版本来源 |

[IDE serializer](https://github.com/aws/aws-toolkit-vscode/blob/e1b0f9bc573bac1830f0b9aa7ced25a6c69bb477/src.gen/@amzn/codewhisperer-streaming/src/protocols/Aws_restJson1.ts)、[IDE models](https://github.com/aws/aws-toolkit-vscode/blob/e1b0f9bc573bac1830f0b9aa7ced25a6c69bb477/src.gen/@amzn/codewhisperer-streaming/src/models/models_0.ts)、[CLI input](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/crates/amzn-codewhisperer-streaming-client/src/operation/generate_assistant_response/_generate_assistant_response_input.rs)、[CLI README](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/README.md)。索引中的关键词搜索零命中只是这几个冻结文件的检索结果，不是所有服务实现的否定证明。

当前公开链路是 HTTP handler → runtime Kiro executor → pre-lease production builder → Kiro inference transport → EventStream decoder。定位：`apps/gateway/src/lib.rs:1066`、`runtime.rs:2220/4597/4638/5404`；`crates/provider-kiro/src/conversation_request.rs:261/306`、`inference.rs:337/345`、`event_semantics.rs:139/154`。IDE 使用 q.<region>.amazonaws.com/generateAssistantResponse；CLI 使用 runtime.<region>.kiro.dev。API Key 不冒充 OAuth 目录；profile ARN 仍按凭据种类及端点策略处理。

本轮定向重跑 `runtime::batch_c::kiro_real_factory_bridges_text_tools_and_effort_without_dropping_limits`。这是 **PASS_HANDLER_AND_FIXTURE**：公共 Actix handler、生产 factory/builder、受控 transport 接收断言。Social fixture 分别覆盖 IDE eu-west-1 与 CLI us-east-1，JSON/SSE；每个 fixture 接收8个 uncapped Chat／Responses 工具多轮请求，共32次受控 transport 接收。上限四种输入 × 两端点策略 × JSON/SSE 共16个拒绝，新增 Attempt／transport 接收均为0。它不是 Kiro 真实网络服务验收，也没有证明其他凭据／地区组合。

合法 Messages 必须有 max_tokens，当前没有等价原生传递，因此 Messages 六个基础组合及全部24个独立 cap 行继续 BLOCKED。Chat／Responses uncapped 局部通过不能用于有硬预算的真实验收。未删除必填参数、添加提示词预算、下游截断、猜测 effort 或更改能力声明。

最小下一步：取得当前 IDE／CLI 可追溯的原生 input/event 定义或已获授权的原生抓包，明确字段／范围／计数／reasoning／cap终态，以及端点×凭据×地区×profile适用性。只有这组证据充分后才建立公共边界 RED 并实施映射；本轮没有真实抓包授权。

## B：Grok Web 原生续接与 Messages 上限

冻结参考 `chenyme/grok2api@7c889a960e2638341b4dae9a5c81af0e0f38c87f` 的 [gateway.go](https://github.com/chenyme/grok2api/blob/7c889a960e2638341b4dae9a5c81af0e0f38c87f/backend/internal/infra/provider/web/gateway.go) 展示 WebSocket 方言：`/ws/mgw/`；uid 绑定会话身份；已有会话设置 conversation_id/load_existing/needs_history，消息携带 parent_response_id。conversation 来自原生会话附着事件，parent 来自成功 response.done 的 response.id。两者不能互换。

同版本 [chat.go](https://github.com/chenyme/grok2api/blob/7c889a960e2638341b4dae9a5c81af0e0f38c87f/backend/internal/infra/provider/web/chat.go) 的 REST 分支处理 userResponse.responseId 和 modelResponse.parentResponseId（1021/1074行），是另一套来源；历史格式化（658–735行）也不能自动证明原生会话等价。该文件广义检索命中16次，均为下游 stopReason/stop_reason 表示，不是发现了原生输出上限字段。

当前生产路径仍使用 `/rest/app-chat/conversations/new`，`web_production.rs:312/383` 只接受单 user 消息并设置 temporary/disableMemory；`web_live.rs:162/235` 将 conversationId 投影为 canonical ResponseId、读取最终 modelResponse.message，没有保留已验证的 parent。`web_conversation.rs:121/179/200/221/239` 提供精确绑定原语，但未接入公开 owner／成功终态／并发推进；`web_production.rs:795/854` 的 continuation context／403出口处理不能直接作为精确续接证明。

已定位续接方言和标识候选，但尚未证明它们与当前生产 REST 方言、成功根判定、SSO/account/credential revision/egress绑定及取消并发约束的完整组合。没有为 canonical response_id 猜 parent，也没有拼接历史模拟原生多轮。Messages 输出 cap 的字段、计数和命中终态同样 NOT_PROVEN。

本轮定向重跑 `web_unrepresentable_histories_tools_and_limits_reject_before_lease_or_dns`：**PASS_HANDLER_AND_FIXTURE**；公共 Responses cap、合法 Messages cap、Chat 历史多轮、Responses tools 四类输入均400，Attempt=0、DNS=0。此为 preflight 保护证明；不是 Web 多轮成功证明。

两个定向执行的原 receipt 归属均为 `d3d2e3c5bf0d9521f3fb0d830344b34053f62155` 的工作区，原 dirty 状态保留在执行索引。相关 handler／factory／native request 与测试文件在 d3→c1b 没有差异；本批 Store 兼容修复不改变这些执行前拒绝和受控原生 payload 断言。最终干净 Full 另行验证 c1b，原定向来源没有改写成最终提交。

Grok Web 工具的12个基础组合保持 **NOT_APPLICABLE_ACCEPTED**，单列拒绝证据。其他六个文本多轮组合因原生续接未装配继续软件 BLOCKED；Messages另有必填 cap 语义阻断。已声明 stored continuity／WS 仍待逐项冻结和验收，actual_declared=null 表示未知，不能通过关闭声明消除义务。

最小下一步：先冻结当前可用的 REST 或 WS 原生方言及可信 conversation/parent 成功事件、推进/失效规则与 cap 契约，再把这些状态接入 owner、精确凭据和出口绑定原语，并用公共三协议受控上游验证实际 payload、接收次数、失败根、取消及并发。整个渠道的真实验收仍需要具体账号、环境、授权和预算。

## E-05 与交付边界

E-05 是 **APPROVED / PASS_LOCAL_REUSED**：只保留严格同授权证明的 Grok Build 正常刷新 revision 例外；手动替换、重新授权及其他归属变化继续拒绝。本轮不重新请求政策选择。旧矩阵的 AWAITING_USER_DECISION 仅保留为历史。

本轮新增页面行为为无，因此没有新增浏览器验收。已有 EgoLite 场景按原来源静态复用，不改为本轮动态执行。真实 Provider=0；远端 CI、候选发布制品、发布、部署及生产均 NOT_RUN。具体执行回执见 [本批报告](cpar-channel-delivery-local-20261010.md)。
