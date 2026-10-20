# CPA-Rust #62 C4：Codex client_metadata 本地接收与加密诊断

C4 限定功能、归属、加密保存、授权读取与生命周期门禁达到 **PASS_BOUNDED**；本轮整体回归为 **PARTIAL**，因为最终两次 Full 均在既有 Heartbeats/EOF/timeout 用例失败，根因仍 **UNKNOWN**。不能给无保留的完整交付 PASS。

本轮只实施 2026-10-20 新批准的 D1-A Codex C4 子范围。CPAR 是逐字段审查后请求 metadata 的最终接收方，完整加密保存用于授权本地诊断，不转发 Grok Build，不提供 Codex 服务端追踪、历史摄入或审批处理，也不宣称与上游原生消费等价。ADR-0107 的旧 Accepted 决策未改写；C4 不是对 D2 或其他字段的整体批准。

## 基线和不可变输入

- 源工作树：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-native-namespace-20261020`，`feature/62-native-namespace`，实际 HEAD `973e1f589d24d0220b58d95071d91f824f29074d`。参考 SHA 只用于定位，没有回退或用旧远端覆盖本地交付。
- 独立工作树：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020`，`feature/62-client-metadata`。
- 实现提交：`50957d2369f705b7c6f440801950a92fb6522c92`；审查发现的 StoredResponse 副本修复：`32ece7f4bd4ea02cdb07256af2a447842e156840`。
- 最终验证源码 commit `32ece7f4bd4ea02cdb07256af2a447842e156840`，tree `43698ae7521552c26f983d93cecfac49b6048d67`；验证工作树以该 commit detached，使用其自己的 manifest/lock 和独立 `target-full`。
- 源工作树 18 项既有 dirty/untracked 文档及规则逐项携带、记录来源/原 hash/mode/link 与实际应用 hash。三个共改文档通过独立 index blob 只暂存本轮增量，既有内容留在 working tree。其余既有改动未暂存。Full 明确是带 18 项 overlay 的验证，不能称为纯产品树 Full。
- GitHub #58/#61/#62 及依赖只读快照均已核对；三票 OPEN，#62 仍被 #61 阻塞。本轮不写 GitHub，不推送、不合并或部署。

输入清单与验证前后比较、overlay 逐项记录及保存校验见本报告资产目录。未将旧方案因为携带而整体批准，也没有 reset、clean、stash、覆盖或广泛暂存。

## 字段契约与限定行为

新增 [ADR-0116](../adr/ADR-0116-local-codex-request-metadata-diagnostics.md) 和 [BC-PROTOCOL-010](../contracts/BC-PROTOCOL-010-local-codex-client-metadata.md)。来源是固定的真实 Codex 0.161.0 ingress 与官方固定 commit 的序列化代码；这证明客户端形状，不证明其声明真实或目标服务具有相应消费语义。

- 外层只允许 `root_turn_id`、`session_id`、`thread_id`、`turn_id`、`x-codex-installation-id`、`x-codex-turn-metadata`、`x-codex-window-id`。每键可缺失，普通 string 可空，null/非 string 拒绝；缺失不捕获，显式 `{}` 捕获。
- 完整原始字段 JSON 最多 65,536 UTF-8 字节，含 escaping/空白；普通内外 string 最多 1,024 字节。超界拒绝，不截断。保留完整外层原字节及内部 JSON string，不重序列化改写。
- 内部 20 个可选非 null 成员逐项分类；5 个观测选择值仅允许 `turn`/`user`/`exec`/`seatbelt`/`workspace-write`，4 个状态为 boolean，window 为 u64 整数，客户端时间为非负 i64；其余明确 string。审批、sandbox、执行、来源和 model 都仅为不可信客户端声明，不成为 CPAR 权限。内外镜像身份同时存在时必须一致，不补造缺失成员。
- 未知、未分类、非法形状、重复键（含 escaped-equivalent、内部 blob）拒绝；history_ingest、workspaces、tool namespaces、compaction、parent/fork/subagent 未获准。
- 仅认证后的 Responses HTTP，且匹配候选非空、实际全部为 GrokBuildResponses adapter、Responses target、Canonical/CanonicalBridge 时本地消费。混合候选、其他 adapter/协议/模式/WebSocket 不准入。无 User-Agent/header/宽泛能力位开关。

生产代码路径中，合法且获准的 C4 在投影前移除，并在执行前确认保存；不合格 C4 返回安全 ClientRequestError，持久化/确认失败返回 RecordingUnavailable。字段不会进入 Build body、其他模型字段、14 个实际生成的 tracking/header、普通事件/日志/错误/计费。这里的生产代码行为由受控公共 HTTP 验证，实际生产环境未启用或验收。

## Request 诊断闭环

新增独立 `RequestMetadataEvidence`，不是 NativeUsageEvidence 或 MessagesSummaryEvidence。复用现有 bounded queue、SQLite writer、SecretStore/XChaCha20-Poly1305、确认期限、私有 CLI output 和 maintenance 生命周期；没有重建存储平台。

原认证 Request 和其私有证据同队列 slot/同事务确认，必须收到 Persisted 才能执行、取得 lease 或发送。超时后的实际持久化状态为 UNKNOWN，不能据此重发上游请求。失败无虚假成功；不为补证据增加 Usage、账本或 Attempt。

schema 37 新表每 Request 一份，AAD 独立绑定 owner、Request、版本、创建与过期时间。原始 Request/owner 由认证及持久索引核验；客户端值不能生成或覆盖 owner、Request/Attempt、路由、凭据、审批或 sandbox 策略。读取返回一份不可变 capture 和当前零个或多个实际 durable Attempt；可选 Attempt selector 必须属于该 Request，未实际发生的 Attempt 不能伪造。

`store:true` 的 StoredResponse replay root 在本地字段消费后才快照；旧 StoredResponse 格式中没有 C4 副本，不能另起保留时钟。continuation 仅合并历史 messages，compaction 新建 extensions。

授权诊断仍需要本地数据库及外部 key 权限，使用新 `gateway request-metadata-evidence` 命令，精确指定 owner+Request（可选实际 Attempt），只写 canonical 0700 私有目录下新建、NOFOLLOW、0600 文件；原文不写 stdout/公共错误。授权边界不是从客户端自报值或 HTTP 参数获得。

## 验证与审查

先保留原 C4 公共边界 RED：合法字段实际 HTTP400，而目标为200。缺失本地前端依赖的构建失败、解决前提的命令和后续 RED 均保留，未把编译失败当行为 RED。

初次 Spec 审查发现 1 项 P2：提前克隆的 `store:true` 请求会把完整 C4 写入旧 StoredResponse 的独立生命周期。新增公共 HTTP 用例在修复前准确失败；最小修复移动快照时点，JSON/SSE 解密核对与 C4 GC 后检查通过。初次 Standards PASS、Spec FAIL 与后续修复后的两个 PASS 分别保存，不将审查问题抹去。

| 检查 | 最终状态 | 边界 |
| --- | --- | --- |
| C4 公共 HTTP | **PASS_BOUNDED**：9 passed | 真实 compiler/factory/lease/Build adapter、独立 receiver、JSON/SSE 1/7/31、完整授权 reader |
| 持久化/reader/实际 CLI 私有输出 | **PASS_BOUNDED**：5 passed；ignored child 由父用例真实启动 | owner/冲突/幂等/keys/expiry/receipt/schema/备份恢复/实际进程 kill+restart |
| 最终 Full | **FAIL**：两次均 gateway 269 passed / 1 failed / 4 ignored | 原脚本、原断言/期限，独立构建目录，18 项 overlay |
| Standards | PASS 静态 | 最终聚合 27 文件，无可操作发现，固定 commit/tree+实际输入 |
| Spec | PASS 静态 | 初次 P2 已修复，无其他新增可操作 C4 缺口；不代表 Full 或客户端 |
| 原样 Codex 严格成功门禁 | **FAIL/BLOCKED**：CLI exit1、HTTP400、独立接收0、六项门禁全 false | 保留完整请求字段、当前安装程序、独立 OS 隔离，无降级/关工具/删字段 |

公共正例单独使用合法 C4 合成请求：正常回答、summary、工具和 known Usage 20/7 保持；独立 receiver 检查 body 和 14 个实际 Build headers，无字段/值暗映射；同 Request 精确完整原文读回；每个工具场景五次请求/接收/Attempt/Usage/ledger/capture，相关结果为42。无 Attempt 预拒绝、32 类非法/超界/重复输入、其他 adapter/模式、缺 writer/key、写锁、冲突、丢确认、幂等 replay 和 held receipt 均独立验证。

持久化用例覆盖异主和多Attempt关联：后者合成两条 durable Attempt 记录，不冒充公共fallback多次发送验收；公共HTTP正例每Request实际Attempt1。另覆盖历史未知、活跃/旧密钥轮换、缺 key/错 key、到期 tombstone、互换密文/时钟/index 损坏、schema36->37 不改变旧 Request 字节、加密备份恢复、独立真实 reader 进程、existing-output 拒绝、queue overflow、原 2 秒确认 timeout、实际 writer SIGKILL 后重启读回。ignored child 由父测试真实调用，不能仅以忽略数量冒充执行。

### 历史 EOF/timeout 失败

上一轮 Full 的 HTTP503/预期200、随后同二进制重跑通过，原因仍 UNKNOWN。本轮早期受影响 gateway 并发回归再次出现：268 passed/1 failed/4 ignored；同一失败二进制定向重跑1 passed。保留失败二进制 hash、完整日志及重跑，未修改断言或期限。

加入仅测试用失败诊断后，并发回归为267 passed/2 failed/4 ignored，另一个 Messages deadline 测试也失败；定向重跑通过。本次可定位到 Heartbeats 的500ms总期限，在公开响应头之前返回 EgressUnavailable：实际 Attempt1，独立 receiver 完整请求0，Request/Attempt 写入2，SQLite failure/quarantine/pending均0，lease已归零。完整 receiver0不能推定网络完全没有字节，也不能否定真实 Attempt。为何超过期限仍 UNKNOWN；不能宣称 C4、证据写入或某项初始化已被证明是根因。

最终固定源码两次 Full 使用同一 SHA-256 `6b969388341b73776d04867e32924dda70631bdc509e0e5bfd4bc1bda9ab9f9b` 的二进制，均为269 passed/1 failed/4 ignored。首次 Attempt 从开始到结束625ms，Heartbeats 在公开响应头之前 EgressUnavailable；两次均完整接收0、实际Attempt1、writer提交2且SQL/quarantine/pending错误均0。失败二进制定向原断言重跑1 passed/0 failed（2.46s）。这不解释并发时超过500ms的原因，也不证明与C4无因果关系。原二进制本地保留，未提交大型可执行文件。

两次 Full 都是32步骤PASS、Rust tests步骤FAIL，后续12步骤因fail-fast未执行；随后按原脚本的后续步骤单独完成12/12 PASS，末尾明确保留原Full FAIL。另以明确命令完成其余workspace（exclude gateway）1312 passed/0 failed/11 ignored、全部gateway integration 19 passed/0 failed/2 ignored、同一固定失败二进制的C4 filter 9 passed/0 failed。它们不重新定义Full为PASS。所有命令、日志和分组统计见资产。

## 原样客户端观察

原驱动 SHA-256 `52daceb172dfb286aafcb67aaeb9916e907c7ff3f7c4c3aef3199daa4f5a7da9` 未修改，同时启用 `--require-codex-success --require-prompt-release`。当前安装为 Codex0.161.0、Pi0.84.3（shell与declared bin）、Node26.8.2；安装树字节/mode/link前后相同。独立Seatbelt实测IPv4/IPv6非loopback、真实HOME读取、临时目录外写入均EPERM。没有relay、降级、关工具、删字段或弱化六项门禁。

实际Codex仍完整携带12个root字段、7个metadata string成员、20个blob成员，以及namespace、`web_search/external_web_access:false`、parallel控制和三条带id的非assistant输入。结果CLI exit1、唯一实际请求HTTP400、独立Build接收0、Attempt/Usage/ledger观测均0，cost UNKNOWN；六项成功门禁全部false。旧expected400 preparation断言通过不能转成兼容PASS，外层runner exit1/FAIL。

实际Pi RPC受控正例保持五次增量200、五次独立接收及Request/Attempt/Usage/ledger各5；负例收到SSE failed而未伪装成功。取消经过真实delta、lease1、abort，同PID98611观察后仍活、独立close1和lease0，abort→release上界13ms，在服务stop及客户端清理前完成，严格prompt-release门禁通过。观察后的RPC清理退出143；清理后该PID确已不存在。这些本地受控通过不解除#61成功metadata与cost阻塞，也不是三客户端或真实Provider验收。原样Codex没有获得完整成功，#62仍OPEN/BLOCKED。

新的当前客户端原始ingress/配置留在私有隔离目录；提交资产仅含字段形状/类型/哈希、结果和隔离/安装身份，不额外复制当前C4私有值。C4完整原文的授权reader验收使用独立合法合成请求，未把它冒充上述原样Codex的成功。

## 兼容、恢复及仍保留的阻塞

schema36->37 仅增加空 namespace，不补历史数据，不暗改任何旧 payload v1。新 reader 对 schema34–36 返回 UNKNOWN，对37验证读取，更新 schema 拒绝；现有 Usage/summary readers 在37继续适用。旧二进制拒绝37，不能以单换旧二进制或 destructive down migration 当恢复。

精确保留30天，以 gateway persistence 时间而非客户端时钟起算，`now >= expiry` 立即未知；GC每次最多128份密文并保留非 payload receipt，跨重启/轮换不能续期或复活。新写使用 active key，旧密文需要对应旧 key，无隐式重加密。在线GC不会追溯清除已授权导出或离线备份，恢复reader仍使用原expiry、不续期。恢复必须有一致 SQLite 快照、Request/Attempt/账本/receipt 及所有引用 key，并在隔离副本验证；本轮只测试合成副本，不执行生产恢复。

namespace 生产继续 **BUILD_ACTIVATION_BLOCKED**；ParallelTools、C5 `web_search/external_web_access:false`、C6 非 assistant input message id、Claude thinking/effort/cache/metadata、D2、#61 成功 metadata/cost_in_usd_ticks **NOT_ADMITTED**、#66/#67 均未批准或解决。C4 成功不让原样 Codex 或 #62 变绿，不关闭 #62，不取消三客户端首版要求。

下一项最小任务是独立定位并发 Full 中 Heartbeats 500ms 的响应头前耗时：固定已保存的同一二进制、源码/lock/overlay及原负载，补 client 初始化、连接和 send 各阶段的时间证据，保持断言/期限；现有失败二进制和日志足以作为输入，不需账号或真实Provider。本轮没有扩大到未证实根因的生产修复。

字段方面，C5 仍先需要独立契约与资格材料：需要固定 Build profile 对 `external_web_access:false` 的服务端权威语义，以及 search/open/find、动作阶段、sources/引用、历史与适用 Usage 的完整原始样本。没有这些输入不能将 false 改成 true/default、删工具、转普通 function 或伪造搜索结果。本建议未授权实施 C5，也不重复无新材料的 cost/namespace 调查。

真实 Provider、真实账号/收费请求、远端 CI、push、工单/依赖修改、合并、发布/部署均 **NOT_RUN**。

## 证据入口

- [机器状态、命令和计数](assets/cpar62-client-metadata-20261020/run-summary.json)、[源码/输入绑定](assets/cpar62-client-metadata-20261020/source-binding.json)、[7038项输入清单](assets/cpar62-client-metadata-20261020/source-inputs-before.json.gz)。
- [18项overlay来源](assets/cpar62-client-metadata-20261020/validation-overlay-binding.json)、[仅本轮暂存记录](assets/cpar62-client-metadata-20261020/owned-staging.json)、[原树及feature保全](assets/cpar62-client-metadata-20261020/preservation-final.json)。
- [初次Standards/Spec](assets/cpar62-client-metadata-20261020/review-initial.md)、[最终Standards/Spec](assets/cpar62-client-metadata-20261020/review-final.md)。
- [Full首轮](assets/cpar62-client-metadata-20261020/full-check.md)、[Full完整复核](assets/cpar62-client-metadata-20261020/full-repeat-check.md)、[后续12项检查](assets/cpar62-client-metadata-20261020/full-remainder-check.md)，各自完整日志同名`.log.gz`保存。
- [固定源码C4公共门禁](assets/cpar62-client-metadata-20261020/green-public-final-immutable.log.gz)、[全部gateway integration](assets/cpar62-client-metadata-20261020/gateway-integration-final.log.gz)。
- [实际客户端状态](assets/cpar62-client-metadata-20261020/actual-cli/run.json)、[保留字段形状](assets/cpar62-client-metadata-20261020/actual-client-projection.json)、[清理后进程核对](assets/cpar62-client-metadata-20261020/actual-cleanup.json)。
- [显式allowlist证据包](assets/cpar62-client-metadata-20261020/development-evidence.tar.gz)、[成员哈希清单](assets/cpar62-client-metadata-20261020/archive-members.json)：RED、前提/编译失败、早期回归、源材料与两次最终Full诊断。没有DB、key、客户端配置或可执行文件。
- [实际内容及解包扫描](assets/cpar62-client-metadata-20261020/artifact-scan.json)：49个归档成员逐项解包核对hash/bytes/mode，使用未修改的仓库regex和禁止路径规则。输入清单中一个历史tracked `task-…` 文件名误中凭据模式；仅对该精确行hash例外，原清单不改写，其他字节继续扫描；[例外核验记录](assets/cpar62-client-metadata-20261020/artifact-scan-exceptions.json)记录Git blob与文件hash。
