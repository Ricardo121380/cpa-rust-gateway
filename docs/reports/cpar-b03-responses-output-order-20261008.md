# CPAR B-03：Responses 原生 SSE 整体输出顺序

**B-03 / P2：RESOLVED_LOCAL。** 最终接受源码为
`5e51f5f170a01b813631824b004eb2fcbf190201`。公开 loopback 正反例、相邻回归、
干净隔离 Full 和两轴独立评审均完成。原始 13 项 P2 仅新增关闭 B-03，剩余由
10 降为 **9**；发布仍 **BLOCKED**。此结论不扩展为 CPAR-03、整个 Batch B、
#57、真实 Provider 或生产完成。

## 任务映射、基线与范围

仓库为 `/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway`，分支
`codex/prism-v4-delivery`。开始时实际 HEAD 为
`9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2`，与用户检查点一致。
检查当前 decoder、B-02 报告及最新 release-readiness 发现后，确认 B-03 仍是
最终 output 的 ID 集合比较，尚无整体顺序修复。未回退到历史提交。

任务直接映射到 [原始 B-03 发现](assets/cpar-release-readiness-20261005/review-results.md)，
通过 issue-tracker 读取 #9 / #27 / #31 / #57；只读快照保存在本切片资产目录。
B-02 接受源码仍为 `15babae64154f8ff5fa1055f856d11640385633a`，其单项完成校验
不能替代 B-03 的整体输出顺序。规则、依赖、预算偏差和授权见
[Task card](assets/cpar-b03-20261008/task-card.md)。

本轮源码提交：

| 提交 | 范围 |
| --- | --- |
| `edb4a93f8689ea3986917ec8befbeec297260889` | 顺序校验、公开正反例、契约补充和首个 RED 证据 |
| `5e51f5f170a01b813631824b004eb2fcbf190201` | Responses bootstrap 保留不可重试协议错误、双候选真实计数对照 |

产品只改 [upstream_response.rs](../../crates/protocol-openai-responses/src/upstream_response.rs)
和 [runtime.rs](../../apps/gateway/src/runtime.rs) 的既有分支；测试修改
[batch_c.rs](../../apps/gateway/src/runtime/batch_c.rs)、
[responses_done.rs](../../apps/gateway/src/runtime/batch_c/http/responses_done.rs)，新增
[responses_order.rs](../../apps/gateway/src/runtime/batch_c/http/responses_done/responses_order.rs)。
未改依赖、schema、Canonical 词汇、metadata 规则、通用重试策略或其他发现。
契约补充位于 [BC-PROTOCOL-001](../contracts/BC-PROTOCOL-001-openai-responses-adapter.md)。
未触及 frontend 或 shared surface，无需跨边界变更日志。

开始时主工作区已有用户修改的 AGENTS.md 及大量未跟踪材料；定向执行保持这些
内容，提交逐路径排除它们。最终保全清单核对 4315 个原文件 SHA256 和 4336 条
原始 status 行，见 [workspace-preservation.json](assets/cpar-b03-20261008/workspace-preservation.json)。
Full 使用独立、已提交、干净 detached checkout，结果不依赖这些未跟踪材料。

## 顺序语义与最小修复

原生 encoder 的输出数组按 `OutputItemStart` 建立位置。B-02 已有 ID → index
及 completed-item metadata 状态。新增校验复用该状态：

1. item start 接收显式 `output_index` 时，必须等于现有表示能准确保留的下一位置；
   缺省仍沿用下一位置。重复、跳号或与建立顺序冲突的位置显式报错，避免后续
   encoder 静默重新编号。这是本地准确表示的边界，不是所有上游事件到达排序规则。
2. 收到终态 `response.output` 时逐位置枚举；该位置的 ID 必须关联同一已建立 index，
   并保持 B-02 已校验的精确 item-done metadata。原有重复、遗漏、未知 ID 和
   全部 items ended 检查继续保留。没有排序、重建数组、丢弃字段或覆盖状态。
3. 失败在 ResponseEnd 前传播，并复用 B-02 的永久 Rejected 生命周期。
   同批剩余帧、后续 push 和 finish 都不能恢复成功。
4. 明确协议矛盾若发生在 Responses bootstrap，不再被折叠成可重试的
   BootstrapTruncated，而是保留 UpstreamProtocolError 为 NonRetryable。
   传输 EOF 等既有 bootstrap 分类保持原样。

delta 到达顺序及不同输出项的完成先后不改变 output 数组顺序。两项开始后，
message 和 reasoning 的 delta 可以交错，reasoning 可以先完成或后完成。
没有新增全局 sequence_number 排序规则或必填事件；缺 start/done index、缺最终
output、缺细粒度 done/part-added/delta 的既有兼容路径仍有正例。
官方对 output_index 位置的定义及当前抓取哈希见
[官方来源回执](assets/cpar-b03-20261008/official-source.json)。

## RED → 最小修复 → GREEN

首个 RED 在产品源码与 base 完全相同的前提下，仅添加公开测试 harness。
两个不同项 `msg-b02`（index 0）与 `rs-b02`（index 1）的内容、身份及五类 done
确认均合法，唯一变化是最终数组由 message/reasoning 变为 reasoning/message。
实际公开输出仍成功 completed，并被 encoder 输出为 message/reasoning：
矛盾被接受及改写。cargo 真实退出 **101，0 passed / 1 failed / 0 ignored**。
见 [原始 RED](assets/cpar-b03-20261008/red.json)、
[产品源码绑定](assets/cpar-b03-20261008/red-binding.json) 和
[可复现 harness patch](assets/cpar-b03-20261008/red-harness.patch)。

初次修复后虽然定向与 Full 通过，独立复核指出 bootstrap 公共错误被重新分类。
继续沿调用链发现该分类可重试，而原 fixture max_attempts=1 不能充分证明没有重放。
补充两合法候选、max_attempts=2 的公开精确上游模型路径；Provider 唯一性规则
保持不变，fixture 采用现有 exact-model 入口。上游均为合成 loopback peer。

| 双候选对照 | 修复前实际结果 | 最终源码实际结果 |
| --- | --- | --- |
| 主端点运输 EOF，备用端点合法 SSE | 200 completed；接收 1+1；Attempt 2 | 同样 200 completed；接收/handoff 1+1；Attempt 2 |
| 主端点同批读取顺序矛盾，备用端点合法 SSE | 错误被备用结果挽救：200 completed；接收 1+1；Attempt 2 | 502 UpstreamProtocolError；主接收/handoff 1，备用 0；Attempt 1 |

补充 RED 真实退出 **101**，产品基于初次实现 `4dd28268…`，runtime 未改。
见 [补充 RED](assets/cpar-b03-20261008/bootstrap-fallback-red.json)、
[harness/源码绑定](assets/cpar-b03-20261008/bootstrap-fallback-red-binding.json)、
[补充 GREEN](assets/cpar-b03-20261008/bootstrap-fallback-green.json)。
普通 EOF 对照证明备用候选确实可用，没有关闭重试或把预算设成 1 来规避验证。

## 公开可观察结果及回归

HTTP 测试经实际 Actix 公开 route 分派、鉴权、生产 compiler/snapshot、lease、
provider builder、真实 TCP loopback 上游、decoder 和 writer。HTTP route 使用
in-process test service；WebSocket 使用实际 loopback HttpServer、101 handshake
及 RFC 6455 客户端。测试是本地合成输入，不是实际 Provider 推理。

| 验收点 | 实际覆盖与结果 |
| --- | --- |
| 顺序冲突失败 | 已交出 source 的 SSE 只有 response.failed / UpstreamProtocolError，无成功 completed；bootstrap 为 HTTP 502 / UpstreamProtocolError |
| 永久失败 | 冲突后同读取批或后续读取追加合法 completed 仍失败；公开 decoder 的后续 push/finish 同样失败 |
| 请求与 Attempt | RequestFinished 为 Failed，error_code 为 UpstreamProtocolError；交出 source 的 AttemptSucceeded 保留；bootstrap 为 AttemptFailed，未伪造 Attempt 的含义 |
| store:true | 冲突 GET 404、续接失败且不增加上游接收/handoff；无成功 StoredResponse |
| WebSocket | 顺序冲突 response.failed；GET 404；同连接 previous_response_id 返回 error，只有一次上游接收/handoff，不能留下成功续接 root |
| 正常多项、交错 | 顺序、两类内容、精确 native metadata 在完整 SSE output、GET、下一轮 output 和实际上游历史中保持一致；合法 WS 两轮也一致 |
| 兼容省略 | 无最终 output、无 index/细粒度确认及只有 snapshot 内容仍成功，未新增无依据必填事件 |
| 相关边界 | 重复/缺失/未知最终项、重复/冲突/跳号 index、反转 incomplete 终态、跨 1/7/31/整块读取；B-02 自带分块和 metadata 反例继续执行 |
| 相邻回归 | B-01 多段 assistant 历史、B-02 五类 done、metadata/lifecycle 和永久失败、native tool rounds、protocol/router 全部通过 |

Request/Attempt 观察来自实际 handler 的 event sink；测试将这些真实事件 append 至
临时 SqliteEventStore 后重开读取。这验证观察及持久化表示，没有声称生产异步
writer 或进程重启验收。StoredResponse 的自动保存、GET 和续接使用实际公开路径。

## 最终验证与源码绑定

| 检查 | 命令/回执 | 退出及数量 |
| --- | --- | --- |
| 定向公开与相邻 batch_c | `cargo test --locked -p gateway runtime::batch_c --all-features`；[回执](assets/cpar-b03-20261008/final-batch-c.json) | 0；26 passed，0 failed，0 ignored |
| 协议与路由 | `cargo test --locked -p protocol-openai-responses -p gateway-router --all-features`；[回执](assets/cpar-b03-20261008/adjacent.json) | 0；224 passed，0 failed，1 preexisting ignored |
| 严格定向 Clippy | `cargo clippy --locked -p protocol-openai-responses -p gateway --all-targets --all-features -- -D warnings`；[回执](assets/cpar-b03-20261008/final-clippy.json) | 0 |
| 最终 clean Full | `bash scripts/check.sh full`；[逐步检查](assets/cpar-b03-20261008/full-check.md)、[原始输出](assets/cpar-b03-20261008/full-output.json)、[结构化回执](assets/cpar-b03-20261008/full-receipt.json) | 0；45/45 steps，1477 passed，0 failed，12 ignored |
| 本地文档收口 | `bash scripts/check.sh docs`；[回执](assets/cpar-b03-20261008/docs-gate.json)、[逐步检查](assets/cpar-b03-20261008/docs-check.md) | 0；7/7 steps，946 Markdown 文件、146引用测试检查通过 |

Full checkout 为 `/Volumes/Asgard/Agentprojects/CPA-Rust/b03-20261008-validation`，
执行 HEAD 精确为 `5e51f5f170a01b813631824b004eb2fcbf190201`，前后 status 均空。
15 个被审源码/文档 blob 的 SHA256 未变。CARGO_NET_OFFLINE=true，复用主仓库
target 缓存，Full 回执写至 checkout 外；没有利用未提交源码通过验证。
ignored 沿用原套件，未增加 skip 或弱化断言。Full 中的本地 harness 自测名称
不代表生产 graph/live 请求或部署已经执行。

定向命令发生在提交前的主工作区，保留实际 execution HEAD 和当时 dirty 状态；
[targeted-binding.json](assets/cpar-b03-20261008/targeted-binding.json) 用实际测试文件
SHA 绑定最终相同 blob，不把旧 HEAD 改标成新执行。协议/router 定向与最终
decoder hash 一致，整套最终 Full 另行覆盖全部最终源码。
完整命令、退出码、时间和中间失败见
[execution-index.json](assets/cpar-b03-20261008/execution-index.json)。

此前 B-02 在未修改的传输故障用例中出现一次 EgressUnavailable，随后同源码
定向 4/4 及 Full 通过，**原因未确定**。保留
[B-02 原报告](cpar-b02-responses-sse-done-validation-20261008.md) 的事实；本轮各次
Full 均未再现，未归因为环境问题，也未改该用例、timeout、并发或断言。
本轮 fixture 构建/配置失败、初次 wrong-target runtime 修改、早期 lint 和用例
预期错误均有独立原始回执；误改的 Chat/Anthropic 分支已撤销，最终 diff 只改
Responses bootstrap。所有这些开发失败仍为失败，没有被重跑结果覆盖。

## 独立评审及本地证据收口

按 code-review Skill，对固定 base 到最终源码的完整 15 文件 diff 执行两轴评审。
两轴原文及主代理裁定见 [review-final.md](assets/cpar-b03-20261008/review-final.md)，
路径与独立 blob 核验见 [review-coverage.json](assets/cpar-b03-20261008/review-coverage.json)。

| 轴 | 最终结论 | 实际覆盖与遗留 |
| --- | --- | --- |
| Standards | PASS | COMPLETE 15/15；前轮完整阅读加最终逐 blob 独立一致性核验及标题规则复核；0 open hard findings，1非阻断测试 helper 重复建议 |
| Spec | PASS | COMPLETE 15/15；前轮完整阅读加最终逐 blob 独立一致性核验和关键分支点验；0 findings |

Standards 曾因提交标题不以 Task ID 开头返回 FAIL；仅更改本轮尚未推送的两个
本地提交消息，新标题均为 B-03 开头。重建时同时省略了首提交正文，精确旧/新
消息和原 commit 对象完整保留，不能说“只有标题改变”。作者、日期、产品树及
工作区均未变；标题问题经新探子闭合，并在新源码 ID 上重新执行 Full。
详见 [commit-title-correction.json](assets/cpar-b03-20261008/commit-title-correction.json)。

一次独立补核误查备份 ref 的目录前缀而返回 PARTIAL，未计作 PASS；之后新的
探子逐项核验具体 first/final refs、15 blobs、消息规则与回执。原 FAIL、PARTIAL
和初次实现的条件性评审保存在资产中；探索性探子的中断/部分结果也不作评审
或测试通过证据。主代理接受一条非阻断测试重复建议，不为其扩张产品改动。

最终证据报告及本地 #57 草稿作为后续文档提交，接受静态文档、引用、secret 与
whitespace 检查；不将新增收口文本虚列入已完成的 15 文件源码独立评审范围。
最终 staged whitespace 曾指出工单快照的多余 EOF 空行；只规范本轮快照的外层
文件结尾，JSON 正文及全部字段保持相同，修复后真实退出 0。原失败与前后 hash
见 [快照格式回执](assets/cpar-b03-20261008/issue-snapshot-formatting.json)；没有改 GitHub 规格。
完成状态和证据 hash 见 [verification.json](assets/cpar-b03-20261008/verification.json)
与 [evidence-hashes.json](assets/cpar-b03-20261008/evidence-hashes.json)。

## 剩余事项与发布边界

A-01、B-01、B-02 的 RESOLVED_LOCAL 保留。本轮仅增加 B-03；原始 P2 剩余
**A-02～A-08、B-04、B-05，共 9 项**。S-01、E-01～E-05、O-01 独立记录，
不混入该计数，也未在本轮修复。

建议下一个切片为 **B-04：失败 bootstrap 的已校验 Usage ownership**。此轮只
纠正 Responses 的明确协议错误分类及重放边界；begin 返回错误之前的 Usage
carrier 仍可能丢失，未声称该缺口已关闭。B-04 不阻断本次 B-03 本地顺序验收，
仍独立阻断发布。后续需要其独立 RED、failed Attempt 与完整 source-counter
ledger 证据，不能继承本切片结论。

真实 Provider/账号推理、远端 CI、新源码制品/签名/远端验证、部署及生产验收
均 **NOT_RUN**。旧 source80 候选的制品、签名和远端证据不可继承。没有修改
密钥/生产环境，没有推送、发布、GitHub 写入或关闭工单。
[#57 证据补充](assets/cpar-b03-20261008/issue-57-evidence.md)仅为本地草稿。
回滚本切片产品改动应同时回滚顺序/分类分支、测试和契约补充；无数据迁移，
原顺序矛盾会再次被接受，需重新标为未解决。原始备份 refs 及历史 RED 可复查。
