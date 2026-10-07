# CPAR B-02：Responses SSE 完成确认严格校验与失败闭环

日期：2026-10-08，Asia/Shanghai。**B-02/P2 已 RESOLVED_LOCAL**：旧实现的公开错误成功路径已复现，五类完成确认的闭集、关联、生命周期、内容／元数据一致性和永久失败已修复；公开保存／续接、真实本地 WS、无重放、最终源码 Full 和独立 Standards／Spec 均通过。本轮要求的本地验证与评审没有遗留项。发布保持 **BLOCKED**。

## 身份、源码与交付边界

- 固定起点／任务开始实际 HEAD：`6883f47ea89332563b6ac90ce42c077387b0f9c5`。分支 `codex/prism-v4-delivery`；没有后续 B-02 修复。此前产品为 `939028d63519126f16252c3e72d3e1d5ace7ca75`，A-01／B-01 已 RESOLVED_LOCAL。
- 第一修复检查点：`3aa5b475c6d2b3d8037c47a3cc59ed1b2df49e2d`；本地 TLS mock 的必需序号补齐：`ee65e01279be71695140591d446f8d1c0db5689a`。
- **最终修复源码／Full／独立复核对象：`15babae64154f8ff5fa1055f856d11640385633a`**。后续交付提交仅收口文档和回执，不改变这些产品／测试文件；[源码文件哈希](assets/cpar-b02-20261008/validated-source-files.json)绑定 9 个必要文件。
- 原 [B-02/P2 发现](assets/cpar-release-readiness-20261005/review-results.md)、[#27](assets/cpar-b02-20261008/issue-27.md)、[#31](assets/cpar-b02-20261008/issue-31.md)、[父规格 #9](assets/cpar-b02-20261008/issue-9.md)为任务依据。GitHub 只读快照保留；未写入或关闭工单。[B-01 报告](cpar-b01-responses-multipart-history-20261007.md)及其 [O-01 独立观察](assets/cpar-b01-20261007/additional-observation.md)保持原文。
- 只修现有 Responses native SSE 装配，不添加协议、转换、Canonical 类型、数据迁移、重试策略或能力。无前端、生产、真实 Provider、推送、远端发布、候选签名或 GitHub 写入。

## 实际调用链与最小改动

公开入口是 `gateway-http-actix/src/lib.rs:1961` 的 `POST /v1/responses` 和 `:1422` 的 WS turn。请求沿既有 Client-Key 鉴权、route compiler／snapshot、lease 和 `P12RoutedResponsesExecutor::execute_routed`（`apps/gateway/src/runtime.rs:1912`）执行。`EndpointAttemptDriver::start_openai_responses`（`:4869`）经 `send_admitted_request` 读取真实 loopback 上游；`OpenAiSseEventSource::begin_with_reasoning_policy`／`read_until_event`（`:7265–7355`）调用 `OpenAiResponsesSseDecoder::push`／`finish`。

decoder 改动位于 [upstream_response.rs](../../crates/protocol-openai-responses/src/upstream_response.rs)：`:852` `text_done`、`:913` `part_key`、`:939` `part_confirmation` 使用既有文本／reasoning 累积及安全错误。只补充 output-index 归属、part 初值／完成快照和关闭状态；`:508` `confirm_item_parts` 在 item-done 的旧尾部补全前核对已经确认的内容，防止最终 snapshot 又追加或替换。接收过的新增状态有明确边界：索引小于 64，初值／完成快照共享 8 MiB 留存预算；相同重复不增加副本。

`:568` `push` 将解析／语义失败永久置为 `Rejected`，阻断当前读取中的剩余帧，并在返回已缓冲成功事件前拒绝 terminal 后的语义残留。`OpenAiSseEventSource` 将错误传播给 `gateway-http-actix/src/lib.rs:3184` 的 `pump_source_events`，既有 `StreamError` 和 request observation 完成失败闭环；`:3230` 的完成持久化和 `:1656/:1696` 的 WS root 准备／提交条件仍只允许有效成功生命周期。未改变 B-03 的最终 output ID 集合／整体排序分支。

[公开测试](../../apps/gateway/src/runtime/batch_c/http/responses_done.rs)沿生产编译、lease、provider builder、decoder 和 HTTP／WS writer。HTTP 用实际 Actix route 的 test service 分派；上游是实际 TCP loopback HTTP SSE。WS 使用实际 loopback HttpServer、RFC 6455 handshake 和 tokio-tungstenite 客户端。后者是 workspace 已有包，仅添加 gateway 的 dev-dependency 边，并同步精确 crate-boundary allowlist，未增加版本或宽泛豁免。

测试使用合成账号、临时 SQLite。普通 Responses 的 `stored_responses:true` 沿既有闭集 candidate opt-in，WS 能力为该 adapter 原有能力；没有关闭 store／WS 规避验证。真实 handler 发出的 Request／Attempt 观测经临时 `SqliteEventStore::append_batch` 后重开读回。此项验证真实观测与持久化表示，没有声称部署了生产异步 writer；StoredResponse 的自动保存及 WS 续接使用实际公开路径。

## 事件校验表与依据

规则已补充在 [BC-PROTOCOL-001](../contracts/BC-PROTOCOL-001-openai-responses-adapter.md)。字段定义核对 [OpenAI 官方 streaming-events reference](https://developers.openai.com/api/reference/resources/responses/streaming-events)，2026-10-08 采集；它是未版本化的 live reference，[来源回执](assets/cpar-b02-20261008/official-source.json)保留 UTC 时间、1,431,397 bytes 和 SHA256 `3712a8c2bf32264710c0291328c1a753f10effc4978551c8ef3d43b4d1bf99d8`。实际可表示子集同时受 ADR-0100／0101、BC-CORE-003、BC-STREAM-001、BC-RESP-002／003／004 约束。

所有接收到的确认具有闭集字段；`type`、非空有界 `item_id`、整数 `output_index`、无符号整数 `sequence_number`、适用的 content／summary index 和 payload 必需。output index 必须对应当前 response 中已开始且未结束的 item；content／summary 必须关联相应累积 part。这些事件没有 response_id 字段，response 关联来自活动 decoder；额外 response 身份字段被拒绝。

| 支持事件 | payload／关联 | 完成校验 |
| --- | --- | --- |
| `response.output_text.done` | message／content_index／string text；本地允许省略 logprobs | text 等于累积值；存在的 logprobs 仅空数组，null／错误类型／非空数组均拒绝 |
| `response.content_part.done` | message output_text 或 reasoning body reasoning_text／content_index／object part | part 闭集／类型正确；text 和已观察 annotations 一致，随后 item-done 的完整 part snapshot 必须一致 |
| `response.reasoning_summary_part.done` | reasoning summary_text／summary_index／object part | summary 精确一致；上游可选 interrupted status 无当前原生 Canonical 表示，出现时明确拒绝，不丢弃信息继续成功 |
| `response.reasoning_summary_text.done` | reasoning／summary_index／string text | summary 累积值精确一致 |
| `response.reasoning_text.done` | reasoning／content_index／string text | reasoning body 累积值精确一致 |

最小 part-added 跟踪同时校验初始 shape 和关联，初始文本／annotations 进入既有累积。text-done／part-done 后不能继续文本 delta；part-done 后不能改变 annotations；item 结束后确认非法。开放阶段相同确认无新增语义，矛盾重复失败；未强制创建合同不要求存在的 fine-grained 确认。无确认／无 part-added 的现有合法完成路径继续通过。

event-level logprobs 与 final-part metadata 分开处理：本地原生 encoder 在 `native_output.rs:585–593` 省略 done-event logprobs，兼容仅覆盖省略；final part 原有 null／empty 表示必须保真。未知扩展、非空 token metadata、未支持 part kind／status 无法无损表示时明确失败。没有添加基于猜测的必需事件或全局 sequence 单调／唯一性规则。

## RED → GREEN 与公开状态证据

[初始输入和测试源码](assets/cpar-b02-20261008/red-input.json)、[执行原始记录](assets/cpar-b02-20261008/executions.json)保留最早先测后修的过程。旧产品源码 SHA256 为 `ca99cf45cd832ec83330481d97647364208628195a4b4324c0d6196177b27ff7`，与起点 Git blob 一致。最早 shell 以 tail 结束，未把它的退出码当作 cargo 退出码；完整 FAILED 输出保留。

随后在隔离的原起点上，保持产品源码不变、挂载最终公开 fixture并增加只读诊断，[旧基线补录](assets/cpar-b02-20261008/baseline-red.json)直接捕获 cargo exit 101：先合法 delta `answer`，后 done `contradiction`，最终仍出现 completed；真实 Request／Attempt 观测进入 SQLite并重开后 Request=Succeeded，StoredResponse GET=200／status completed，上游实际接收 1 次。[补录 harness](assets/cpar-b02-20261008/baseline-red-harness.json)保留输入与 patch；该补录不替代最早 RED。

其后 content-part 矛盾和错误 item-done output-index 的公开 RED 均 exit 101；最终对应 GREEN 明确失败。reasoning／summary 类沿原源码同一无条件忽略分支确认，并分别执行公开非法回归，未把未单独执行的旧版反例说成执行结果。

独立初审又发现 done-event `logprobs:null` 类型漏网；[精确 null RED](assets/cpar-b02-20261008/review-null-red.json)在 ee65 产品不变时输出 completed并 exit 101。修正后 [最终定向 GREEN／Clippy](assets/cpar-b02-20261008/review-fix-executions.json)的源文件哈希等于 15baba；省略字段的合法控制保留 final-part null／empty。

| 对象 | RED 公开行为 | 最终 GREEN 公开行为／读回 |
| --- | --- | --- |
| midstream 矛盾确认 | 已输出合法 delta 后仍 completed，RequestSucceeded、GET200 | HTTP 200 的既有 SSE 内唯一 failed／UpstreamProtocolError，无 completed；RequestFailed；GET404，owned continuation 拒绝 |
| bootstrap 同读冲突 | 静态确认旧分支忽略；未另执行旧版 bootstrap | HTTP 502／既有 StreamTruncated 分类；AttemptFailed、RequestFailed，GET404，无保存成功结果 |
| WS 冲突 | 未另执行旧版 WS RED | 101 后合法内容、failed；GET404；同连接 previous_response_id 返回 error，上游接收／handoff 各1 |
| 合法控制 | 原合法能力 | completed、GET200、完整内容和 metadata 不变；HTTP／WS owned continuation 成功，上游各2次 |

AttemptSucceeded 表示 driver 返回活 source，不等于最终流成功。midstream 的既有 AttemptSucceeded 与 RequestFailed 正确并存；bootstrap 错误则 Failed Attempt 与 Failed Request 对齐。未伪造用量、错误类型或成功终态。

## 验收矩阵与相邻回归

- 新增 8 个测试函数全部执行，无 ignored；最终 `batch_c::` **18/18 PASS**。合法 native 控制有 6 种：完整确认、相同确认重复、done logprobs 省略、确认省略、part-added 省略、初始文本。两文本 part、多段 delta、reasoning summary／body、完整终态及 StoredResponse 的 null／empty metadata 都核对。
- 五类确认的字段／关联反例 **75 次**，生命周期／metadata 反例 **15 次**，均通过公开 handler 验证唯一 failed、RequestFailed 的 SQLite 读回、GET404、owned continuation 拒绝、实际 network／handoff 各1；四类 native 矛盾也共用同一完整断言，文本矛盾另保留最早诊断入口。
- 分块补充使用实际 public decoder API：合法流按 1／2／7／31／128／整流读取，Canonical 结果完全一致；五类冲突按 1／7／31／整个 tail读取均失败，后续 push／finish 仍失败。公共上游按多次 TCP body 读取；冲突和后续 completed 连在同一 tail。terminal 后同读残留也不能产生成功返回。
- `store:true` 正反对照始终开启能力；真实 WS 首轮合法及同连接续接成功／失败都验证。失败没有可成功读取／续接的 StoredResponse 或可成功使用的 WS root；实际 provider接收／handoff计数拒绝重放。
- B-01 JSON／SSE 多轮、20 个严格拒绝反例、Grok 原生多片 metadata、工具调用／多轮、Chat／Messages 与合法 terminal 保持通过。协议／router 邻接定向 **224 passed／0 failed／1 既有 ignored**；最终 Full 重新覆盖这些路径，未把旧源码执行转归新源码。

## Full、独立审查与工程闭环

[最终 Full](assets/cpar-b02-20261008/full-check.md)／[精确回执](assets/cpar-b02-20261008/full-receipt.json)／[完整输出](assets/cpar-b02-20261008/full-output.json)绑定 **15babae64154f8ff5fa1055f856d11640385633a**，隔离 clean checkout 前后状态空，9 项源码哈希未变。**45/45 PASS，1469 Rust tests passed、0 failed、12 原有 ignored**；source／crate boundary／契约引用／fmt／workspace Clippy／供应链／secret／whitespace 均通过。

失败证据未覆盖：第一次 Full 因旧本地 TLS mock 的 part-added／done 缺必需 sequence_number 失败，[原输出](assets/cpar-b02-20261008/full-first-failure.json)保留；只给 mock 发帧补序号，原成功、截断、取消、持久化／账本断言未变。[Agent 定向](assets/cpar-b02-20261008/fixture-agent-green.json)及 [ee65 Full](assets/cpar-b02-20261008/full-ee65-pass.json)通过，但该历史 Full 不作为最终 15baba 的证据。

15baba 首次 Full 的未修改 `p11_02_fault_matrix::rate_and_server_status_faults_are_returned_once_without_transport_retry` 返回 EgressUnavailable，[失败回执](assets/cpar-b02-20261008/full-reviewed-failure.json)保留。同一源码、未改 timeout／concurrency／assertion 的 [定向重跑](assets/cpar-b02-20261008/fault-matrix-recheck.json)为 4/4；随后同一源码和默认 Full 完整45/45通过。原因未确定，作为独立执行异常追踪；没有修范围外源码、跳过测试或编造原因。此测试只调用 loopback transport状态码路径，不调用 Responses decoder。

| 审查轴 | 覆盖 | 发现与关闭 | 最终行为裁定 |
| --- | --- | --- | --- |
| Standards | 初审24/24；增量5/8 PARTIAL＋新缺口探子3/3；20个未变blob核对，组合28/28 COMPLETE | 0硬违规；1判断性测试重复已改为现有helper并补核 | PASS，本切片 |
| Spec | 初审24/24；独立增量8/8；20未变blob核对，组合28/28 COMPLETE | 初审FAIL的1P2 event-level null已公开RED/GREEN并复核关闭 | PASS，本切片 |

[两轴原文与准确终态](assets/cpar-b02-20261008/review.md)、[初审FAIL](assets/cpar-b02-20261008/review-initial.md)和 [覆盖集合](assets/cpar-b02-20261008/review-coverage.json)保留。Standards 第一次增量探子误把数组JSON当对象而提前结束，准确记PARTIAL；后用新的单轮探子补三份回执及helper连接，没有追派或把中断记PASS。探子仅静态审查；全部修改、判断和最终执行由主代理负责。后续文档收口由主代理静态检查，不扩大独立源码覆盖。

最终文档收口执行 `scripts/check.sh docs`，**7/7 PASS**；[文档回执](assets/cpar-b02-20261008/docs-receipt.json)保留完整检查表。源码9项与已评审、Full验证的15baba逐项一致；未为文档收口重复执行运行时测试。

## 剩余发现与发布状态

[本地 #57 补充](assets/cpar-b02-20261008/issue-57-evidence.md)仅新增 B-02 RESOLVED_LOCAL。A-01／B-01保留；原13项P2尚余 **10项：A-02–A-08、B-03–B-05**。S／E状态不变，O-01仍OPEN_OUT_OF_SCOPE，不混入原P2计数；B-03、B-04、B-05、E-05、O-01均未修。全B／全Phase不升级PASS或DONE；正式远端Phase Gate、真实Provider、发布候选构建／签名、部署和生产验收均NOT_RUN，发布仍BLOCKED。

旧source80 `80f5838902eb56be6dccc3ce047eb227d6c18e6c` 的制品、签名、Full和迁移演练维持原绑定，不能继承给本轮新源码。只交付本地修复检查点，后续候选在适当批次重新冻结。没有推送、GitHub工单写入或关闭。

[工作区保留回执](assets/cpar-b02-20261008/workspace-preservation.json)核对起始4277个用户修改／未跟踪文件，AGENTS.md不提交。下一项建议独立处理 **B-03：最终 response.output 的整体 item 排序**，保留本轮终态关联修复，不把集合身份一致当成整体顺序一致。
