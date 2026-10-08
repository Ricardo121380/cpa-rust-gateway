# Release-readiness B-04 / P2：启动失败的已校验 Usage

**B-04：`RESOLVED_LOCAL`。最终源码：`062f8281279b9bc6a0dc36b2859120e26e5f85dd`。**
实现、公开 gateway loopback 验证、Store/control/账本读回、干净隔离 checkout Full，以及固定
base 的独立 Standards / Spec 均完成。本结论只关闭 release-readiness B-04，不是 CPAR-04，
不关闭整个 #32、Batch B、#57 或发布。发布继续 `BLOCKED`。

## 范围、基线与提交

实际起点为分支 `codex/prism-v4-delivery` 的
`08812db2373583b8a725b80071efe75b69f9d7ae`。读取工作区、最新报告及原始发现后推进，
未把用户检查点当作回退指令。B-03 已接受源码仍为
`5e51f5f170a01b813631824b004eb2fcbf190201`。

| 提交 | 内容 |
|---|---|
| `42fae997ef7c82f7e611e97f22da8b5a79634233` | 首次实现、公开路径测试、多 Attempt 持久化一致性和 ADR/契约 |
| `640f4a188336326b58c85266182d8faabad5d725` | Standards 修复：ADR-0103 需求追踪登记 |
| `062f8281279b9bc6a0dc36b2859120e26e5f85dd` | Spec 修复：partial 历史冲突不得误伤其他明确 Attempt |

固定 base 到最终源码共 26 文件：14 Rust、12 文档。后续本地收口提交只包含报告、Task card、
追踪状态及本轮证据；提交号由 Git 记录，不把先前运行改标为文档提交执行。前端及共享
API/生产 envelope/schema 未变。原有 `AGENTS.md` 和 4319 个其他原始文件保留，共 4320 路径
SHA-256 不变，见 [保留回执](assets/cpar-b04-20261008/workspace-preservation.json)。

需求来源为 [原始 B-04](assets/cpar-release-readiness-20261005/review-results.md)、
[B-03 报告](cpar-b03-responses-output-order-20261008.md)及只读 GitHub
[#32 快照](assets/cpar-b04-20261008/issue-32.json)、[#9 快照](assets/cpar-b04-20261008/issue-9.json)、
[#57 快照](assets/cpar-b04-20261008/issue-57.json)。归属契约沿用
[ADR-0100](../adr/ADR-0100-exact-protocol-semantics-and-usage-evidence.md)，新增持久化决策和
兼容边界见 [ADR-0103](../adr/ADR-0103-failed-bootstrap-usage-attribution.md)。

## RED：实际首读窗口与三协议边界

测试经过 Actix 公开 gateway HTTP 服务、真实 TCP loopback 上游、生产 decoder/source/driver/router。
gateway 服务在测试进程内运行，未声称启动真实部署进程。上游一次缓冲响应；测试专用钩子记录
实际交给 decoder 的字节，断言首个 read 与整个 fixture 完全相等，没有以 TCP write 次数或 sleep
推定读取边界。钩子仅 `cfg(test)`，没有新增公共测试接口。

| 协议 | 同一次实际首读 | 修复前实际结果和窗口 | 裁定 |
|---|---:|---|---|
| Chat | 516 bytes | 合法 chunk/Usage 后损坏帧；`begin` 失败，502 `StreamTruncated`；Request/Attempt Failed，Usage 观察 0 | 受影响，已修复 |
| Responses | 259 bytes | 合法 `response.created`/Usage 后损坏帧；`begin` 失败，502 `UpstreamProtocolError`；Request/Attempt Failed，NonRetryable，Usage 观察 0 | 受影响，已修复 |
| Messages | 340 bytes | 合法 `message_start` 先使 `begin` 交出流，损坏帧随后由既有观察路径解析 | 同一启动缺口不存在，已有保护通过核验 |

Responses [真实 RED](assets/cpar-b04-20261008/responses-red-route-fixed-receipt.json)和 Chat
[真实 RED](assets/cpar-b04-20261008/chat-red-receipt.json)均 exit 101、0 passed / 1 failed，断言是
已校验 Usage 应为 1、实为 0。输出、fixture、原始 harness 和源码哈希均保留。Responses RED
产品行为未变，仅加读捕获；Chat RED 时 Responses 修复已在 dirty 工作区，但 Chat `begin`
仍为原行为，见各 binding。harness 编译、错误模型引起的 404、修复中 import/初始化编译错误
单独保留，不计为行为 RED。

Messages 没有构造非法 Usage 事件：HTTP 200 后 SSE 协议错误，Request 最终 Failed；既有 Attempt
Succeeded/Completed 表示 source 成功 handoff，不表示 Request 成功完成。输入 11、cache read 5、
cache creation 2 已保留；`message_start` 初始输出占位 0 仍归一为未知。此测试不替代 pre-handoff RED。

## 最小修复与已知事实

Chat/Responses decoder 提供只读 Response ID；source 启动失败时取回已经校验、已经合并的 Usage
与 ID，经每请求 driver 的有界可取槽交给 orchestrator。每次开始清槽、失败后仅取一次，保留原始
错误/retry 分类，不增加上游读取、补采或重放。orchestrator 先确认准确的失败 Attempt，再通过
Required 端口确认带同一 Request/Attempt 的 Usage，随后按原策略考虑备用。Usage 确认失败阻止
fallback；专门反例确实配置可用备用候选。无 Usage 或 Usage 自身无效时，不保存猜测数据。

| 路径 | input | output | cached | reasoning | cache read | cache creation | provenance / input accounting |
|---|---:|---:|---:|---:|---:|---:|---|
| Chat 首读失败 | 20 | 10 | 5 | 3 | unknown | unknown | Measured / Inclusive |
| Responses 首读失败 | 20 | unknown | 5 | unknown | unknown | unknown | Measured / Inclusive |
| Responses 有效快照更新后失败 | 22 | 10 | 7 | 3 | unknown | unknown | Measured / Inclusive |
| Messages 交出流后失败 | 11 | unknown | unknown | unknown | 5 | 2 | Measured / Exclusive |

快照 input 20 → 22 保存 22，不加总成 42。缺失保持 `None`，protected read 保持 unknown/null，
`cost_microunits` null、confidence unpriced。事件、读回和账本的 source counters、provenance、
input accounting 逐字段相等。**完整保留错误前已知用量，不代表获得这次上游请求最终完整用量。**

启动失败保持 HTTP 错误和 Request Failed，不生成成功 `completed`、`[DONE]`、StoredResponse
或可成功续接的 Responses 根；公开 GET 读回不存在。Messages 已有后续失败亦无成功终态。
合法备用成功属于另一独立 Attempt，失败 Attempt 不因 Usage 保留而变为成功。

## 归属、持久化、账本和备用

实际公开持久化链路：HTTP → driver/router → P12 EventSink fanout → BoundedEventQueue →
AsyncSqliteEventWriter 确认 SQLite 提交 → production BillingWorker → Store Usage lineage/账本。
protected HTTP 再验证授权、no-store、counter/confidence 和保真；管理 HTTP 为测试 facade 调用
生产只读 Store/control 函数，不代表完整 production composition。

真正可用的 Chat 备用暴露旧 Request 级 Usage 身份冲突：失败 Usage 保留后，第二次成功 Usage
引发 `RecordingUnavailable` 和 quarantine，[多 Attempt RED](assets/cpar-b04-20261008/attempt-storage-red-receipt.json)
exit 101。修复将新明确 lineage 按 `usage-attempt:<SHA256(AttemptId)>` 幂等保存。Request、准确
Attempt、完整 Usage 和账本 source identity 一致；旧 Request SHA/ResponseId 保留精确重放，
不补填未知历史。无效、缺失和冲突 lineage 仍拒绝或排除，不猜账号/Attempt。

同 Request、同上游 Response ID 的 Chat 两候选 received/handoff 各 1/1：失败 Usage 保存
20/10/cached5/reasoning3，成功 Usage 保存 3/4/cached unknown；两个独立 source、两条账本，
分别归属准确 Attempt/账号。同账号另有 Store/control 反例：Request count 1、observations 2，
输入合并 23，缺失输出及费用未知。同 Usage 经真实 Required sink 重复确认：committed 5、inserted 4、
quarantine 0，Usage/账本各一条。账本重放幂等；same-Attempt/unknown 冲突仍隔离，其他明确 Attempt 不受误伤。

Responses 明确协议错误配置可用备用：主候选 received/handoff 1/1、备用 0/0，保存一次失败 Usage，
B-03 NonRetryable 未变。普通传输 EOF 合法备用成功正对照继续通过相邻回归/Full，使用实际接收和
handoff 计数，不以 retry budget=1 证明无重放。

独立 Spec 发现 partial 历史校验回连过宽：同 Request 的 A1 冲突误配健康 A2。主代理 RED 真实确认
`InvalidPersistedGatewayEvent`；修复仅回连相同明确 Attempt，unknown legacy 仍整组校验。GREEN
保留 A2、排除 A1 两事件/一 Request group，strict 仍失败，最终 Store 独立复核 PASS。

人工 append/SQL 的历史身份、同账号聚合和冲突隔离测试，仅证明相应 Store/读模型表示，
不冒称 production writer/重启。上述公开失败、备用和重复确认主测试确实经过真实 async writer
和 production BillingWorker。本轮未验证带新 B-04 记录的真实进程重启。

## 定向、Full 和独立双轴

| 验证 | 实际绑定 | exit / passed / failed / ignored | 结论 |
|---|---|---|---|
| 三协议、负例、快照、可用备用、protected read | dirty HEAD `08812db2`，逐文件 SHA-256 | 0 / 6 / 0 / 0 | PASS |
| B-01/B-02/B-03、Batch C 相邻回归 | dirty HEAD `08812db2`，逐文件 SHA-256 | 0 / 32 / 0 / 0 | PASS |
| 受影响八包完整定向 | dirty HEAD `08812db2`，与 `42fae997` 源码 blob 匹配 | 0 / 949 / 0 / 8 | PASS |
| Spec 修复后 Store `usage_` | dirty HEAD `640f4a18`，最终 Store SHA-256 | 0 / 9 / 0 / 0 | PASS |
| 最终源码干净隔离 Full | committed `062f8281`，前后 clean | 0 / 1487 / 0 / 12；45/45 门禁 | PASS |
| Standards | 固定 base → 最终源码，全部 26 文件，最终 delta/blob 证明 | 文档登记 finding 关闭 | PASS |
| Spec | bootstrap、control/docs、Store 合计全部 26 文件，最终 delta/blob 证明 | partial finding 经 RED/修复/GREEN/复核关闭 | PASS |

定向完整命令是 `cargo test --locked` + gateway/store/control/router/http-actix/Chat/Responses/Messages
八包 + `--all-features`，过滤及所有参数保留在各 receipt。提交前运行仍保留当时 HEAD、dirty 状态和
文件哈希，没有改标成新提交执行。最终 Full 确实重新执行这些 Rust 测试。

Full：`bash scripts/check.sh full`，checkout 为
`/Volumes/Asgard/Agentprojects/CPA-Rust/b04-20261008-validation`；UTC
`2026-10-08T11:22:01.085006Z` 至 `2026-10-08T11:27:20.214834Z`，319.095 seconds。
本地 target 缓存不改变 committed 输入；14 个变更源码 SHA-256 与最终绑定相等，前后 clean。
45 门禁包括 fmt、Clippy、workspace 全特性、SPA、脚本 unit、source policy、文档/契约、secret、
whitespace、依赖策略和 RustSec。Full 输出 SHA-256：
`0abdc8fa13ef67b07f444958187751c834bf2bb646a2452d5a5423413af4a547`。
[门禁表](assets/cpar-b04-20261008/full-check.md)、[Full 回执](assets/cpar-b04-20261008/full-receipt.json)、
[干净源码绑定](assets/cpar-b04-20261008/full-source-manifest.json)保留完整依据。

12 ignored 保持原有授权/专用运行要求：计时、soak、真实 Provider/诊断、Claude bare、crash child、
生产历史输入、property seed、构建/官方/Web canary。未为本轮跳过新测试或弱化断言。Full 中
live/production 字样脚本是本地 unit/graph/harness 检查，不是实际真实渠道执行。

评审只读独立，主代理承担修改/执行。见 [两轴记录](assets/cpar-b04-20261008/review-rounds.md)、
[逐文件覆盖](assets/cpar-b04-20261008/review-coverage.json)。五个中断轮次为
`PARTIAL_INTERRUPTED_NOT_PASS`，后续完整范围补足覆盖，未改标历史 FAIL/中断。
Clippy 两次 105/100 行失败、一次测试 clone lint 均保留；实际提取 lineage 解析、修正 slice 后
GREEN，最终 Full 再通过，未归因环境或跳过检查。

## 本地收口与发布边界

收口文档 `bash scripts/check.sh docs` 已通过 7/7，exit 0，38.005 s；见
[回执](assets/cpar-b04-20261008/closeout-docs-receipt.json)及
[当时 staged 输入绑定](assets/cpar-b04-20261008/closeout-docs-binding.json)。此运行在主工作区的
实际 HEAD `062f8281` + staged 文档上，未声称工作区干净或改标为后续提交运行。


[结构化验证回执](assets/cpar-b04-20261008/verification.json)记录命令、HEAD、状态、退出码、
数量和哈希；[本地 #57 草稿](assets/cpar-b04-20261008/issue-57-evidence.md)未发 GitHub。
工程偏差保留在 [Task card](assets/cpar-b04-20261008/task-card.md)：探索前未记等级/数值预算，
后补 L/100 min，不能追溯声称事前合规。记录区间不等于不可测的实际工作时长。

真实 Provider/真实账号推理调用 0；无生产凭据读取/修改、生产改动、部署、发布、push 或 GitHub
写入。新源码远端 CI、制品、真实渠道和生产验收均 `NOT_RUN`，不能继承旧候选。新代码兼容旧数据；
旧 binary 不认识新 Usage 身份，未来回滚须依 ADR-0103 使用兼容代码/受控恢复，不假定仅回退 binary
即可兼容新库。本轮不执行这些生产操作。

A-01、B-01、B-02、B-03 保持已有 `RESOLVED_LOCAL`，本轮仅追加 B-04。原始 P2 剩余由 9 降为
**8：A-02～A-08、B-05**。S-01、E-01～E-05、O-01 继续独立记录。下一项建议 **B-05**，另行限定
外部版本化 master-key ring 及真实进程重启证据范围；不把 #32、Batch B、#57 或发布记为完成。
