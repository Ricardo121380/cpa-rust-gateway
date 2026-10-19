# #63：标准 Messages 摘要加密留存与受控 JSON/SSE 工具闭环

日期：2026-10-19（Asia/Shanghai）。本轮是独立受控切片：**PASS_BOUNDED_CONTROLLED_SLICE**。
#63 整票仍为 **OPEN / PARTIAL / BLOCKED_BY_61**。#61 的额外 Usage/固定真实服务 profile 资格仍未获得；
`cost_in_usd_ticks` 继续 **NOT_ADMITTED**。Claude Code 实际客户端属于 #64；真实 Provider、收费、远端 CI、
发布与部署均 **NOT_RUN**。本报告不关闭 #58/#61/#62/#63，不改变原生依赖，不推进 #64–67。

## 基线、隔离与源码身份

开工原目录 `/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway` 的 HEAD 与本地/远端分支均为
`098c92a51ac549a38c8f15fd265e0fb379de2c77`，`feature/62-codex-responses-compat`；前置代码
`ffedb1fa947fd46ee582c02c76f0eda6648372fc` 位于祖先链。历史 SHA 仅作定位，未回退。保留全部 9 个 tracked
修改与 81 个 untracked 文件，index 为空；最终独立核验 90 个文件字节、模式及链接全部保持。

实现隔离在 `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar63-build-messages-summary-20261019`，
分支 `feature/63-build-messages-summary` 从该已验证 feature 基线叠加。未更新或合并 develop/main。
只提交本轮增量；四个共改文档使用基线加自有增量构造 index，既有 overlay 字节仍保留在工作树。

| 身份 | 固定值 |
| --- | --- |
| 初始功能提交 | `0b264fea5affae4229e4566cd36c3070272e182b` |
| 关联工具历史与读回回执 | `c6d0ffe36ab782ae4b5869012a15425b50762ab2` |
| 最终功能/回归源码 | `0d196a66d60cbdbb5c5441a158e5d2f33dda7966` |
| 最终功能 tree | `e1dda68f049862de563b11bb137fbf1240bd448b` |
| 本地验证源码（包含 overlay，禁止推送） | `2cfa920167e9445786c3600468eadc1d1bc58a8b` |
| 本地验证 tree | `863db56fa4f41c576628ebfa263d55b0a33819a7` |
| Cargo.lock SHA-256 | `ee3d735f0f7aa364f3f4675e75a87751c8ad3784f0788ac244c4d391ce479cce` |

Full 在本项目独立目录 `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar63-messages-20261019-0d196a66d60c`，使用自己的 manifest/lock 与全新 target
`/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar63-20261019/target-full-0d196a66d60c`。16 个必要的既有文档 overlay（包括未跟踪的领域规范与 ADR）仅复制到验证树，作为
validation-only 本地提交固定。它们不混入产品提交；逐项路径、哈希和模式见
[source manifest](assets/cpar-63-messages-20261019/source-manifest.json)。这是一项明确的 **overlay Full**，
不能称为裸交付树或远端 CI 通过。初次冻结但未运行的 `0b264fea` 验证树保留为 NOT_RUN；修复前
`c6d0ffe3 + e8ff3bd0` 的 Full 45 步、1571 Rust PASS、14 ignored 只作历史记录，不能替代最终修复验证。

主代理亲自读取适用 AGENTS、CONTEXT、架构、crate boundaries、质量门槛、ADR 索引及 ADR-0100/0107/0108/
0112/0113、BC-OBS-006 和交接材料；当前工单正文、评论与原生依赖另行冻结。新交付契约为
[ADR-0114](../adr/ADR-0114-confirmed-build-messages-summary.md)、
[BC-OBS-007](../contracts/BC-OBS-007-messages-summary-evidence.md) 与
[公开支持说明](../support/grok-build-messages.md)。无前端/共享边界改动，无新增依赖或 lock 改动。

## 实现与加密读回

生产装配只在**实际选中的 GrokBuildResponses + Messages + CanonicalBridge** 上启用 collector。
它先验证完整原 Canonical 生命周期，保留确认过的 reasoning done item、output 位置、原生 response ID、
part 数组顺序、JSON/SSE 来源、确认时间与完整性；将获准 summary 从客户端投影中消费。
缺失 body 保持缺失，显式 `content:[]` 保持空数组；body-only 仍走原严格投影，mixed body/summary 不放行。
这是完整的已确认解码语义留存，未宣称逐字节 whole-wire 保真；64 KiB 上限拒绝而不截断。

例外不基于 User-Agent，不称为完整 LosslessBridge；Passthrough exact 不变。Responses 继续输出 summary，
Chat、其他 Provider、未知 reasoning、密文、签名、引用与未准入 Usage 不继承省略资格。

`MessagesSummaryEvidence`、独立 private queue companion、schema 36 的 `messages_summary_evidence` 与
`SqliteMessagesSummaryReader` 均独立于 NativeUsageEvidence、UsageSummary、日志事件与账本。
request-local sink 绑定实际已确认 Attempt；writer 核验 Request、Attempt、exact owner、原事件索引、execution
checksum/channel 和 transport 后，在同一事务中使用现有外部 key ring 与 XChaCha20-Poly1305 加密。
AAD 域为 `cpar:messages-summary:v1\0`，绑定 owner/Request/Attempt、payload version、creation/exclusive expiry。
相同证据幂等，冲突保留原记录；Attempt 仅作精确重放事务锚，不产生新上游 Attempt、telemetry 或账务源。

JSON 成功及 SSE `message_stop` 前，必须完成完整留存的 **Persisted** 确认与合法标准投影。
Enqueued/Disabled、缺 writer/key、加密/SQL 写入、身份/执行冲突和确认超时均不能放行。
挂起实际提交后的回执时可授权读回，但客户端仍不能成功收口，且上游网络与 lease 已释放。

授权 reader 只读打开，核验 exact owner/Request/Attempt、AEAD、所有事件索引及执行事实；外属、缺失、过期和
schema 34/35 历史缺失同为 UNKNOWN，损坏/错 key 失败。独立 CLI
`gateway messages-summary-evidence` 复用显式参数及 canonical 0700 目录中独占、no-follow、0600、sync 输出。
存储回归经加密 backup/restore 后，两次独立 reader 进程完整读回；stdout/stderr、公共响应和事件日志无摘要。

30 天 TTL 到点即不可读。评审发现旧 GC 删除行后可能允许旧证据重建：已保留 RED（旧实现错误返回 Persisted），
修复为每事务最多清除 128 份过期密文，并保留无摘要内容的 Attempt receipt，部分索引排除已清除行。
相同/不同证据、writer 重启及换钥后都无法重建/续期。恢复必须保留这些 receipt 与事件/执行/账本的 coherent
snapshot 和全部引用 key versions；历史不补造，不把 down migration 或仅回退二进制当恢复流程。
SQLite GC 是列级逻辑清除；本轮不声称覆盖既有备份、WAL 或介质的物理擦除。

## 真实公共路径的受控工具连续性

经公共 `/v1/messages`、生产 compiler/factory/auth、实际 leased Build adapter、真实 UpstreamClientPool，
将已装配的原生 body 与模型 override 交给已准入的独立 loopback TCP receiver；受控 handoff 不发送生产凭据。
这证明受控公共协议路径，不构成真实 Build HTTPS、OAuth、生产服务 profile 或真实账号资格。

每种传输各五个请求：第一次工具调用 → 标准工具结果回传与回答 → 第二次关联工具调用 → 结果回传与回答 → 后续问答。
第二次工具参数显式引用第一次结果；客户端仅使用收到的标准 text/tool_use/tool_result 历史，测试 helper 不补回摘要。
逐轮核对实际发往上游的所有历史 call ID/name/arguments/results 以及后续 text，外发历史无隐藏 summary。
参数保留中文及精确整数 `9007199254740993`。

| 变体 | 五轮 | 独立接收 | Request / Attempt / Usage / ledger | 结束时 lease | Usage / model |
| --- | ---: | ---: | --- | ---: | --- |
| JSON，31-byte body chunks | 5 | 5 | 5 / 5 / 5 / 5 | 0 | input 20、output 7；literal grok-4.5 |
| SSE，1-byte chunks | 5 | 5 | 5 / 5 / 5 / 5 | 0 | 同上 |
| SSE，7-byte chunks | 5 | 5 | 5 / 5 / 5 / 5 | 0 | 同上 |
| SSE，31-byte chunks | 5 | 5 | 5 / 5 / 5 / 5 | 0 | 同上 |

标准 Usage 输入显式包含已知 `cached_tokens:0`，所以 Messages exclusive input 20 可验证；不从缺少 cache 明细
推定零。费用均 UNKNOWN，无额外原生价格字段资格。上游 body、override header、公共模型三者 literal 一致，
没有构造别名证明。每次成功立即 exact-owner 完整读回：每轮 2 个 reasoning items，output positions 0/2，共 3 个
summary parts，第一项 body 缺失，第二项空 body；item 哈希在 JSON/SSE 间逐轮一致。source 字段准确区分 JSON/SSE。
最终加密读回 metadata 回执与 ciphertext 哈希见 [controlled matrix](assets/cpar-63-messages-20261019/controlled-matrix.json)。

SSE 同时经标准 Messages decoder 验证与帧检查：message/content-block 生命周期、文本增量、分片工具 JSON 参数、
唯一 message_stop。原生 item done 可交错且倒序到达，最终留存仍按原 output 顺序；非法 done/terminal 不成功。

## 严格失败、取消与计量

- 已知 thinking/budget、top_k、孤立 tool_result、带 signature 的 thinking 历史在发送前拒绝，独立接收数与
  Attempt/Usage/ledger 为零；非获准 transform mode 同样发送前拒绝，具体 400/503 按既有准入语义核验。
- 未知 part、null/显式空数组与已观察 delta 矛盾、重复/冲突身份、done/terminal 文本或顺序矛盾、密文/签名、
  mixed body、model mismatch 和未准入 cost 字段在响应后准确失败，单次接收、无成功收口、不重发。
- 实际 key 缺失、SQL trigger 写入失败、Attempt evidence/execution 冲突、queue-only 回执、真实 SQLite lock
  确认超时均失败；已确认 Usage 保持独立 source/ledger。SQL transient failure 只在客户端失败后解除，以便原队列
  自行 settle，不把后续提交当作客户端曾成功。原始失败与修复记录均保留。
- 真实公开监听器 FIN/RST × 等待响应头/idle SSE 与生产使用相同 `h1_allow_half_closed(false)`。
  取消用长上游 deadline，按现有资源观察边界核对 peer close 和 lease=0，再清理 server/client；不靠提前 kill 或
  缩短推理超时取得结果。等待响应头时可能独立 receive=1 但 durable Attempt=0，不能据后者声称零调用。
- 挂起 Usage 或 summary 回执时取消，先释放 source/网络/lease，再 await **同一** pinned future；不重复入队、
  不读 ahead、不重发。native CompletedResponse 与 RequestCancelled 可同时成立；这不代表客户端收到完整响应。
- 断连、physical truncation、语义 EOF、idle/total deadline 与 malformed terminal 均单次失败；已确认 item 可作为
  ConfirmedItems 保存。未收到 Usage 时不生成 Usage/ledger，费用 UNKNOWN；已验证 Usage 可保留而交付 cancelled。

上一轮 Responses cancellation/Usage 回归保持。老测试曾在并发运行中因 server cleanup 抢在 durable
RequestFinished 前发生而失败：已保留失败日志，测试现在先等原 Cancelled 持久事件再 cleanup；没有放松断言、
增加超时或更改生产取消行为。原始直跑与后续全回归证据分开保存。

## #63 AC 对照

J1–J8、S1–S6 按工单 JSON/SSE 原序编号；PASS 限于本轮受控内容，不能抵销 J1 的正式依赖。

| AC | 本轮证据与结论 |
| --- | --- |
| J1 公共 JSON/生产装配/加密/读回，基于 #61 边界 | 受控已知字段与 literal model 路径 PASS；完整 #61 元数据/profile 资格仍 BLOCKED |
| J2 item/part 身份层级顺序、缺失 body | 完整读回两 item/三 part、native 顺序及缺失/空 body 对照 PASS |
| J3 限定交付与 mode/UA 契约 | 实际 selected adapter 三条件门控、严格 mode/UA 回归及 ADR-0114 PASS |
| J4 两工具循环/结果/后续问答 | 标准可见历史与实际上游五轮请求逐项比对 PASS；依赖隐藏/native continuation 的模式仍拒绝 |
| J5 已知 controls/history 预拒绝 | 公共错误与独立 receive=0、Attempt/Usage/ledger=0 PASS |
| J6 unknown/empty/identity/done/write 失败，其他内容不获例外 | negative matrix、SQL/key/ACK/冲突、body-only/mixed/other adapter 回归 PASS |
| J7 回答/工具/已知 Usage/model；成功前确认留存 | 各轮标准 Usage20/7、三处 literal identity、挂起 ACK 阻断成功 PASS；extra Usage 未获准 |
| J8 公开说明/Responses 保留/其他路径不继承/客户端另验 | support/README/BC 与 Responses/Chat/other-adapter 回归 PASS；真实客户端 NOT_RUN |
| S1 复用 summary/meta/model/mode 契约与公共 SSE | 真实 bounded pool + 1/7/31-byte stream，完整 native 解码/校验与私有保存 PASS |
| S2 生命周期/增量/参数/唯一终态/两循环/JSON 一致 | 标准 decoder + 帧核验 + 实际历史 + full item 哈希及 Usage/model 对照 PASS |
| S3 保存与投影在 message_stop 前；故障不成功 | 挂起真实提交回执、queue-only/写/密钥/身份/超时与畸形终态 PASS |
| S4 cancel/disconnect/deadline/malformed，无 ahead/resend/duplicate | 真实 FIN/RST、pending same receipt、EOF/deadlines、独立 counts/ledger/lease PASS |
| S5 独立上游/authorized readback/四账/预拒绝/JSON+Responses | 四变体每组五请求五源四账、外属 UNKNOWN、Responses summary 与既有 Usage 回归 PASS |
| S6 不冒充实际 Claude/真实渠道验收 | 支持说明和报告明确受控边界；实际 Claude #64/真实渠道 NOT_RUN |

## 验证与两轴评审

最终 `scripts/check.sh full` **45/45 步 PASS**，Rust **1572 passed / 0 failed / 14 ignored**（127 个测试结果组）；本轮 Messages/summary 19 项均 PASS。
用时 725.704 秒，源码/manifest/lock/overlay 身份与上表一致，验证前后树及 27 份 manifest/input 哈希保持。
完整逐步结果见 [Full receipt](assets/cpar-63-messages-20261019/full-final-receipt.json) 与
[45-step log](assets/cpar-63-messages-20261019/full-final-check-report.md)。14 ignored 按 Cargo 报告保留，包括需显式授权/配置的实际客户端、服务/历史输入、
专项 timing/property/soak 与子进程 fixture；不将 ignored 计为通过。本地 Full 中含 live/production 字样的
脚本是 harness/静态回归检查，不构成实际 Provider、共享服务或发布验收。本轮另外在同一冻结源码及 target 上使用 `--nocapture` 导出 20 份授权读回 metadata
和 4 份独立接收/四账回执，断言五轮 JSON/SSE item 哈希一致、每组五源四账且 lease=0。
证据追加仅文案和受控日志，另做 docs/static/secret/whitespace 检查；不把这些文件冒称为上述 Full 已包含的输入。

- **Standards：PASS**。独立静态评审，初次 1 项 P2（GC 重放）已复现并修复，最终 0 未解决，未报告独立 Fowler smell。
  见 [最终 Standards](assets/cpar-63-messages-20261019/review-standards-final.md)。
- **Spec：PASS_BOUNDED / 整票 PARTIAL_BLOCKED_BY_61**。独立静态评审未发现本轮受控切片遗漏、越界或错误实现，
  见 [最终 Spec](assets/cpar-63-messages-20261019/review-spec-final.md)。两个结论不互相抵销。

保留公共 JSON502/SSE error 无 message_stop 的原始行为 RED；第一次 npm 缺失环境错误不算行为 RED。
保留开发期编译、fixture cache 证据不足、非法 SSE fixture、mode status 预期、写故障 settle、旧 schema sentinel、
旧取消 cleanup race、clippy 与 GC 重放 RED 的失败及修复轨迹；不把它们抹成始终绿色。归档、成员哈希与索引见
[日志包](assets/cpar-63-messages-20261019/verification-logs.tar.gz) 和
[归档索引](assets/cpar-63-messages-20261019/archive-index.json)。包只含受控日志/代码和哈希，不含数据库、keys、
原用户 diff 或原有未跟踪文件内容；所有文本与解压成员按本仓 secret regex/禁用路径扫描。

## 剩余门槛与后续实际 Claude Code 条件

#61 仍需固定生产服务 profile 的一方字段证据与 JSON/SSE 资格；`cost_in_usd_ticks` 不得继承 summary 省略资格，
没有新一方证据不重复做字段推测研究。真实 Build 是否接受不含 summary 的标准历史仍 **UNKNOWN**，本轮 loopback
不能替代真实服务验证。模式若要求未表示的上游身份、reasoning 或 server continuation，继续准确拒绝。

#64 实际 Claude Code 验收需另立固定 executable 版本/哈希、隔离 HOME/keys/history、受控独立上游、可观察的真实
公开 listener 与 egress 限制；使用客户端原始请求和实际工具结果完成两次关联工具循环及后续问答，检查实际
native 外发历史、加密 reader、已知 Usage/账本和取消交付分别成立。客户端须存活到 peer close/lease=0 观察完成，
不能在 cleanup 中提前 kill 制造通过。约束仍须以同一源码身份和准确 listener/deadline 绑定；该验收不会自行
取得真实账号/收费 Provider 或发布授权。原生 #63→#61、#64→#63 依赖保持。
