# CPAR B 批次本地交付与验收 — 2026-09-30

## 范围与结论

- 来源：[实施规格 #9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)，B 批次 CPAR-18–29 / #27–38。Kiro 专用完整适配 CPAR-30 / #39 属于 C。
- Checkout：`codex/prism-v4-delivery`；固定起点为 A 批次提交 `24142a9008e58fbc87d0b5791bbb166b45234850`。
- 授权范围：本批次实现、隔离合成本地验证和当前分支提交。管理契约单向同步只更新 Prism vendored contract；生成客户端再次生成后内容相同，未修改前端业务。见 [cross-boundary-log](../cross-boundary-log.md)。
- 本地真实 gateway HTTP 验收 **PASS，322 项**；最终 Full 门禁 **PASS，44/44**，Rust **1,420 passed / 12 ignored**，Prism 单元测试 **459/459**。Standards / Spec 审查为 **PARTIAL**；已定位发现经复现、修复和主线程回归关闭，覆盖限制见下文。
- 真实 Provider、真实账号、收费推理、生产发布、远端 CI：**NOT_RUN**，没有用本地 fixture 的通过代替这些结论。
- EgoLite 产品流程：**NOT_RUN / 不适用**。本批次未改变浏览器产品流程；公开 HTTP、SSE 和 RFC6455 WebSocket 由真实 loopback 客户端验证。

## 工单映射

下表描述实现和本地验收覆盖，不能据此推定真实渠道支持所有组合。

| 工单 | 实现与本地行为证据 |
|---|---|
| CPAR-18 / #27 | 三协议已知字段、角色、顺序、历史、模型和扩展严格准入；native exact 也校验，未知语义不能逃逸；3×3 JSON/SSE 文本及工具闭环 |
| CPAR-19 / #28 | 稳定 call ID/name、对象参数、声明匹配、一次有序结果；错误结果、多轮回传和交错 Unicode 参数；坏参数显式失败且没有成功终态 |
| CPAR-20 / #29 | auto、none、required/any、named 与并行/串行控制有明确映射；真实 outbound 保留控制；显式并行参与 capability admission；上游违约失败 |
| CPAR-21 / #30 | OpenAI effort 原值保留；Messages 数值 budget 原值保留；无精确等价的 effort↔budget 拒绝；Chat reasoning_content 与答案独立，JSON/SSE reasoning-only 历史完整回传且不越过未完成工具结果 |
| CPAR-22 / #31 | 原生 Responses item/part IDs、phase、annotations、reasoning 层次及 Messages signature/citations/cache history 保留；跨协议无法表示时明确失败，无成功 replay root |
| CPAR-23 / #32 | 六个 nullable 源计数、measured/estimated/unknown 和 input accounting 进入事件、read model、ledger；精确 attempt lineage；缓存及 reasoning 子集不重复计费；失败/取消/已解码但投影失败的用量保留；同 transport chunk 的后续坏帧不丢失已验证计数 |
| CPAR-24 / #33 | 唯一终态；真实 SSE/WS/启动中 TCP 关闭终止上游并释放容量；有限且不同绑定的早期 retry；语义已送出/取消后零 retry；局部认证失败不阻断 sibling；累计启动预算 |
| CPAR-25 / #34 | 各协议安全错误形状；count_tokens 明确 unsupported，认证及已知模型权限先于 capability 拒绝；不合成 token 数量 |
| CPAR-26 / #35 | store:true 在租约/上游前要求所选候选声明 StoredResponses，独立于 compact/WS；可公开编码后才写 AEAD stored response；保存先于成功 terminal；精确 Client-Key 所有权、TTL、损坏密文；真实进程重启后 public GET |
| CPAR-27 / #36 | 恢复工具及 native metadata history；精确 route/candidate/endpoint/account/revision 再准入；重启后恰好一次原目标正例，失效 lineage 在上游前拒绝且零 fallback |
| CPAR-28 / #37 | CPAR compact 独立 AEAD/table、有限历史及精确所有权/lineage；真实重启后继续；畸形、异主、独立新记录的过期/损坏均本地失败 |
| CPAR-29 / #38 | 真实 RFC6455 鉴权/Origin、文本分片、二进制/UTF8/碎片边界；共享执行器工具/thinking/usage；session continuation；root 冲突不先 completed；close/队列 overflow 取消及容量释放 |

## 高层本地证据

运行入口：[cpar-batch-b-http.py](../../scripts/acceptance/cpar-batch-b-http.py)。每次建立独立临时 SQLite、随机合成管理/客户端/Provider 凭据和两个 loopback TLS Provider，启动实际 `target/debug/gateway serve`。所有自己启动的进程在 `finally` 中终止；未使用现有服务或真实 Provider。公开回执只保留值无关调用分类和断言，不保留私有状态目录、凭据、请求体或响应体。

结果：[HTTP receipt](assets/cpar-batch-b-20260930/batch-b-http.json)，322 个 PASS。三协议 streaming 150 ms 启动预算实测 chat 165 ms / responses 158 ms / messages 156 ms，各恰好一次上游调用；非流式受控 peer 约 3 秒终止后同样没有 fallback。

既有 [Agent multi-turn / request-chain gate](assets/cpar-batch-b-20260930/agent-regression.json) 同样通过真实本地 gateway：JSON/SSE × stored/non-stored × 三轮，12 次完成、3 次无 attempt 的 decode 拒绝；工具回传和原始 reasoning summary 均完整重放，调用及 materialized ledger 计数一致。

HTTP 计数与事件/账本核验贯穿以下边界：

- 3×3 JSON/SSE 文本、工具、连续回传和并行控制；语义不支持的正反例分开。
- Content projection 失败前，完整 JSON 中已知的 Messages Usage 仍进入唯一 ledger 行；Chat 同一 TLS 写入中合法 Usage 后跟坏帧，三个公共协议均失败且已知 input/output/cached/reasoning 四项保留于唯一 ledger 行；取消后的 input/cached 保留，未知 output 保持 absent。
- Stored response、compact、native thinking/annotations 的正例均先结束并重启 gateway，再通过公开 GET/continuation 验证。过期/损坏测试各使用独立新记录，避免前一次 GC 掩盖后一次密文错误。
- SSE 与 WS 关闭实际客户端 socket，Provider 观察上游 EOF，再发新请求证明容量恢复；启动中取消没有第二次调用。
- 初始负例只有 primary candidate；单独发布合成 fallback 配置后才验证 503 的 primary→fallback 两次调用、两端都失败时最多两次，以及 401 无透明重试但新请求可用 healthy sibling。
- Streaming 累计启动预算严格限制启动时长；非流式保留已有有限生成 ceiling，耗尽 route 累计预算后禁止再开始 fallback。该兼容行为不是扩大后续 retry 窗口。

回归先复现并修复了三个实际边界：数据 listener 的 Actix 默认 half-close 保留导致客户端 FIN 后仍占上游；WS root 冲突先发送 completed；JSON 先投影失败导致后排 Usage 丢失。

## 决策、数据兼容与限制

[ADR-0100](../adr/ADR-0100-exact-protocol-semantics-and-usage-evidence.md) 明确严格兼容边界，并 supersede ADR-0034/0036/0037/0038 的丢失式思考/签名/用量处理。无法等价的签名、引用、结构化 reasoning、缓存控制和外来 encrypted reasoning 拒绝；拒绝通过只证明错误处理正确，不代表该桥接能力可用。

Schema 0033 给既有 ledger 增加 Usage evidence metadata。旧数据默认 unknown，保留旧 unknown fingerprint；不会把缺失计数或未知历史填成零、measured 或 exact。只有来源与 input accounting 明确、适用维度充足且无子集冲突时才产生 exact cost；estimated/unknown 不冒充精确账单。

数据 listener 禁用 HTTP/1 read-half continuation，使 FIN 取消 bootstrap/idle stream。客户端等待推理时必须保持收发两端开放。管理 listener 沿用原行为。

真实 Kiro、Grok、Codex、Claude、Kimi 等渠道的模型、账号、预算与生产组合没有在本批次调用。源码/本地纯 codec、受控 HTTP 和真实渠道验收保持分层；整体规格 #9 的真实组合与全站验收仍需后续授权证据。

## 门禁与审查

严格 `cargo clippy --offline --locked --workspace --all-targets --all-features -- -D warnings`：**PASS**。Prism 单元测试 **PASS，459/459**，见 [frontend test summary](assets/cpar-batch-b-20260930/frontend-tests.json)。

Full 收敛修正了旧验收入口：legacy reasoning-to-chat 改为原始可见 reasoning parity（七 parity、二 hardening、一 unsupported）；旧 Responses SSE 合成 peer 补齐 item.done；Chat capability 及 Claude→Chat 预期与 B 保真实现一致；无来源证据的旧管理 Usage 明确 unknown；Anthropic 非对象参数在更早的 Canonical 层拒绝，旧 JSON/SSE snapshot 保留新增源 Usage 明细；Agent 多轮合成 peer 验证原始 summary、ID/status 保真，不再要求 flatten 成 content。原差异报告保留历史范围并链接新的决策，没有删掉坏参数、截断或 stale/misclassification 负例。

完整 `CARGO_NET_OFFLINE=true CHECK_REPORT_PATH=... bash scripts/check.sh full`：**PASS，44/44**，见 [Full receipt](assets/cpar-batch-b-20260930/full-check.md)。该轮在实现源码检查点 `f6c37ef02c7acb4bcf31e7f2a47d8c55eec1a7fd` 上执行；后续仅更新报告、回执与提交说明，与此实现源码检查点分开核对。该轮再次执行实际 gateway 的 Agent multi-turn / request-chain gate 并通过。Rust 1,420 项通过、12 项按既有规则忽略，没有改变 ignore 规则；前端 double build、TypeScript、密钥扫描、依赖策略和 RustSec audit 均通过。

Standards / Spec 固定点审查：**PARTIAL**。比较固定起点 `24142a9` 到首轮实现提交 `b8cba0f`；主线程将可定位发现逐一复现并修复，最终 Full 在修复后的源码检查点上通过。

| 审查轴 | 实际覆盖与限制 |
|---|---|
| Standards | 主要生产增量、schema 0033 和高层脚本已读；跨界日志/提交 trailer 可核验，已读范围无规范硬违规。大型 diff 有截断，测试模块、fixtures、生成 contract/receipt 未逐 hunk 全文审查。 |
| Spec — #27–32 | 核验 Canonical、准入/工具控制、目标投影、三协议相关 codec、Usage observer、计费/ledger 和 HTTP 验收片段；未穷尽所有 codec 分支。 |
| Spec — #33–38 | 核验执行/budget 调用点、HTTP pump、保存顺序、continuation、compact、WS cache 提交及对应脚本片段；完整错误/隔离矩阵、进程级密钥轮换未在此审查穷尽。 |

| 发现 | 修复及主线程验证 |
|---|---|
| P2 / #30：reasoning-only Chat 输出不能回传 | 请求 decoder、native admission 和上游 builder 共同接受非空 reasoning_content + null content；仅含空/错类型思考内容的请求及无精确等价的跨协议历史仍拒绝。新增 codec、Router、Provider 回归；真实 JSON/SSE 的下一轮 outbound 与原历史相同。 |
| P2 / #32：同 chunk 合法 Usage 后跟坏帧导致计数丢失 | decoder 暴露已验证 pending Usage，source 合并内部/外部 pending，HTTP 失败补偿可见；从单字节到完整载荷各分片尺寸均保留。实际 gateway 的三个公共协议均失败而保留唯一 ledger 行，输入/输出/cached/reasoning 四个已知源计数完整保留。 |
| P2 / #35：store:true 未检查选中候选能力 | ingress 将明确保存需求传至共享 executor，在候选准入时要求 StoredResponses；JSON/SSE 缺能力负例零上游调用，既有声明的保存/重启/续接正例继续通过。 |
| Standards 维护性候选：已失效 Usage narrowing 的命名/参数链 | 删除 P12ResponseUsageProjection、恒等投影 helper 及其传参链；源计数直接保留，完整回归覆盖三协议编码和 ledger。 |
| 主线程追加顺序边界：空 Canonical content 的 reasoning-only assistant 可能越过 pending tool result | 工具历史验证要求追加 assistant 工具块非空；先复现失败再修复。Core 回归及真实 JSON/SSE 负例通过，均在上游前拒绝。 |

原三处行为发现均先产生失败证据；修复后高层 HTTP **322/322**、严格 Clippy 和 Full **44/44** 通过。额外的两名集中只读探子经催收后仍未形成终态结论，已中止；这轮补充复核 **NOT_RUN / 未形成可用结论**，未计入 PASS。主线程承担修改、最终 diff 和运行验证。不能把以上 PARTIAL 审查描述为全仓无缺陷。
