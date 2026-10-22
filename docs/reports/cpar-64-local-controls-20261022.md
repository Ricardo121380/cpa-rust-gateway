# CPAR #64：Messages 本地 M3/M4 控制闭环

日期：2026-10-22，Asia/Shanghai。

**本轮两项本地职责完成：M3 keep:all 恒等处理、M4 metadata.user_id 完整加密留存与授权读回。**
这是指定生产装配与独立受控上游中的 `PASS_BOUNDED`，不完成整个 #64。
原样 Claude Code 仍 `BLOCKED_BEFORE_FIRST_RESPONSE`；M1/M2/M5、#61、固定 Build 服务资格及工单依赖保持独立。

最终不可变代码候选：`9035551dcef79e130e17f7591fa3ca8a0ff3a265`。
实现提交为 `32da5424383d6ad4b6939fa02e073f174a7736ae`，随后最小回归修复为 `9035551d`。
本报告和精选证据另行提交，不把它们的提交 SHA 冒充运行候选重验。

## 分项结果与边界

| 范围 | 状态 | 本轮实际证据 |
| --- | --- | --- |
| M3 独立 | PASS_BOUNDED | JSON、1/7/31 字节 SSE 各通过；仅唯一 keep:all 编辑、每轮完整历史谓词，标准输出 applied_edits=[] |
| M4 独立 | PASS_BOUNDED | 同四种传输；原始字段逐字留存、真实 owner/Attempt 读回，不进入 Build body/header/input 或公共响应 |
| M3+M4 组合协议 | PASS_BOUNDED | 四变体各五请求、两次相关工具循环及后续问答；20 条逐请求 summary/metadata/Usage/身份/账本回执 |
| 拒绝与确认故障 | PASS_BOUNDED | 有界套件 11/0/0；M1/M2/M5 独立及组合、非法历史/策略、持久化失败均在发送前失败 |
| 原样实际 Claude | BLOCKED_BEFORE_FIRST_RESPONSE | 2.1.302，两次原样400、receiver=0；本地 M4 两份 Request-only 原值已授权读回 |
| 实际 Codex/Pi 回归 | PASS_LOCAL_CONTROLLED | 严格 runner exit=0，七场景与 Codex 六项成功门槛全通过；真实工具、磁盘续接、失败与同进程取消 |
| 定向真实进程恢复 | PASS_BOUNDED | 267 项检查、8 个实际 serve 进程、新旧证据共存、重启/轮换/receipt/隔离备份恢复与防重放 |
| 最终 Full | PASS，45/45 | Rust 1656 passed / 0 failed / 17 ignored；明确包含18文件继承 overlay |
| registry yanked 查询 | UNKNOWN_TIMEOUT | Full 内及一次定向复查均超时；cargo audit 两次 exit=0，不将该查询写成已确认 |
| 真实 Provider | NOT_RUN | 全部上游流量由独立受控接收端承担，无真实账号或收费请求 |
| 远端 CI | NOT_RUN | 无 push/PR/评论/工单修改 |
| 发布、部署 | NOT_RUN | 无共享服务或生产状态变更 |

机器可读状态见 [verification-summary](assets/cpar64-local-controls-20261022/verification-summary.json)，
全部精选文件的 SHA-256 见 [evidence-manifest](assets/cpar64-local-controls-20261022/evidence-manifest.json)。

## 实际基线、规则与保全

从指定 worktree `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020`
的实际 HEAD `4099d260fd40fa63ad328844c06fa677705b488b`、分支 `feature/62-client-metadata` 开工。
先核实 worktree、index、dirty/untracked、相关报告及 GitHub 正文/依赖。
`e4d5d4c1fedd2d3fa394a5a155eeb528d249c173` 至该 HEAD 的产品代码/manifest/lock/scripts 差异为空；
历史 SHA 仅定位证据，没有回退生产工作树或从旧 main 开工。

主代理完整承接适用 AGENTS、CONTEXT、domain/issue 规则、架构/边界/质量门禁、
ADR-0114/0116/0120、BC-PROTOCOL-010、BC-ROUTER-004、BC-OBS-007，及字段方案 M1–M5/D1/D2。
最新 [Codex 组合报告](cpar-62-codex-combined-20261022.md) 的后续状态覆盖旧字段计划中的 Codex 阻塞描述；
没有重复实现已经完成的 Codex 字段。

最初 index 为空，继承修改为10个 tracked 文件、8个 untracked 文件，共18文件。
逐一冻结原字节、mode、uid/gid、symlink；最终仍全部保全。
其中 ADR index、traceability 的本轮后继链接使用独立 index blob 仅提交自有追加内容，
继承前缀及其余16文件未提交。没有 reset、clean、stash、整体暂存或清理历史失败。
详见 [baseline](assets/cpar64-local-controls-20261022/baseline.json)、
[overlay](assets/cpar64-local-controls-20261022/overlay.json) 和
[preservation](assets/cpar64-local-controls-20261022/preservation.json)。

只读复核时 GitHub #61/#62/#63/#64 均 OPEN，正文与开工快照相同；
`#64 → #63 → #61` 未变。完整摘要与正文哈希见
[github-status](assets/cpar64-local-controls-20261022/github-status.json)。

## 新契约与最小纵向实现

新增 Accepted [ADR-0121](../adr/ADR-0121-local-messages-request-controls.md) 和
[BC-PROTOCOL-015](../contracts/BC-PROTOCOL-015-local-messages-request-controls.md)，
明确这是独立于 Codex C4 的 Claude D1-A 子集决定。旧 Accepted 决定保留，只追加后继边界。
一手字段定义及检查日期记录在契约中：
[Messages metadata](https://platform.claude.com/docs/en/api/messages/create)、
[context editing](https://platform.claude.com/docs/en/build-with-claude/context-editing)。
这些定义没有授予固定 Build 服务资格。

M4 的闭合子集为原始 metadata 对象，只允许可选字符串 user_id。
缺失不捕获；空对象、空字符串可捕获；null、非字符串、未知键及重复键拒绝。
字符串上限512 Unicode scalar、原始字段上限65,536 UTF-8字节，超界不截断。
转义、内部空白、Unicode 和原始 token 完整保留；user_id 内部即使是 JSON、甚至包含重复名称，
也不解析或重写，不从 device_id/account_uuid/session_id 派生权限、身份、恢复或路由信息。

资格仅限已认证标准 Messages HTTP、非空且全部精确模型匹配的实际
GrokBuildResponses/Responses/CanonicalBridge candidates。
空候选、混合/其他 adapter、其他模式、User-Agent 或请求开关都不能授予本地消费资格。
合法 M4 先经现有有限队列原子确认 Request+私密证据，只有 Persisted 才能继续。
随后其他控制可被拒绝而留下 Request-only；不制造 Attempt。
实现复用 protocol-anthropic 原始扩展、runtime 候选检查、router 的具体本地消费接缝及 HTTP 确认屏障。

M3 只接受完整形状 `{"edits":[{"type":"clear_thinking_20251015","keep":"all"}]}`。
每一轮检查全部有效工具关联与历史；Reasoning/Opaque/native thinking/redacted_thinking、未知策略、
额外键或其他 keep 值均失败。工具结果限字符串或闭合 text block 数组，避免将未表示的原生块当文本。
不解释工具参数和普通文本中的同名字符串。
恒等处理不改变可见消息、角色、顺序、工具调用或结果关联；控制字段不进入 Build 原生请求。
成功 JSON 根和最终 SSE message_delta 根准确输出 `context_management:{applied_edits:[]}`，
不伪造清除数量、token 数或上游执行回执；无 M3 时保持原输出形状。

没有为 M1 adaptive/display、M2 整体 effort、M5 block cache_control 增加资格，未采用 D2。
首版三客户端使用 Grok Build 的目标、既有 capability ledger 与成本准入边界没有变化。

## 加密版本与持久化责任

Codex C4 保持 payload v1 的原始 JSON 形状、AAD 和协议意义；Claude M4 使用显式
`anthropic_messages_metadata` source、payload v2、schema38。
v2 的独立 AAD 域绑定 source、Messages 协议、认证 owner、Request、版本与创建/过期时间；
解密后还要求 payload/source/version/durable Request protocol 一致。
没有把 Claude 数据暗塞入 Codex v1。

schema37→38 仅重建 metadata 表的版本约束，逐列原样复制旧行、密文及 NULL expiry receipt；
不重封旧密文，不改旧事件、Usage、账本或历史 UNKNOWN。
新 reader 同时读 v1/v2，旧 reader 拒绝 schema38；down migration 遇到任何 v2 行或 receipt 均拒绝。
授权 reader 继续以真实 owner 和可选真实 durable Attempt 检查归属，沿用私密 CLI、AEAD/key ring、
30天排他 TTL、有限清理与防复活 receipt。

公共 HTTP 定向测试覆盖缺 writer/key、冲突、丢失确认、真实数据库锁定超时、幂等重放，
以及 Enqueued/Disabled/QueueFull 确认结果注入；真实有限队列满/未运行 writer 状态另由存储集成测试覆盖。
这些来源分开，不把注入结果说成每个队列条件都在一个真实 HTTP 队列里自然发生。
确认屏障挂起时，即使数据库已能读到 Request-only 证据，lease/receiver 仍为零；
放行后仅一次捕获和一次发送。
确认超时保持 UNKNOWN：解除锁后原待写事务可以落盘，但原 HTTP 失败和 FailedFinished 不变，
不发送、不补写重试，也不把晚到持久化重标成原请求成功。

## RED、发现的回归与修复

在基线产品代码上先运行独立 M3、M4 公共正例断言：两者均得到 HTTP400，
各自独立 accept/receive=0，而期望200，形成两条独立 RED。
原日志、最初测试源码和模块补丁分别存于
[RED log](assets/cpar64-local-controls-20261022/red-public-m3-m4.log.gz) 与
[RED inputs](assets/cpar64-local-controls-20261022/red-inputs.json.gz)。
实现后才分别完成 M3、M4 及组合验收。

首次候选 `32da5424` 的冻结 Full 发现真实回归：原生 Messages encoder 把新的 raw metadata
重序列化为 Value，导致既有 `canonical_request_round_trips_through_the_anthropic_messages_wire`
完整相等断言失败。该次 Full 明确 FAIL，不覆盖或重标：
[failed Full](assets/cpar64-local-controls-20261022/full-failed-32da5424.md)、
[原日志](assets/cpar64-local-controls-20261022/full-failed-32da5424.log.gz)。

`9035551d` 只修复 native inverse 的原始 metadata 序列化，并增加空白/转义/opaque 内层 JSON 回归。
保留旧完整相等及 outbound fixture 断言；protocol-anthropic 34/0/0 与定向 Clippy 通过。
这没有改变本地消费后 Build 请求的字段集合。

开发期还出现了接收端 oracle、SSE 测试解析、fixture 生命周期、混合候选配置约束及工具调用命令错误，
逐项修正测试夹具或命令，没有放宽产品约束、改超时或削弱断言。
首次 strict CLI 命令被 OS 临时目录路径 guard 拒绝，未启动客户端；改用当前实际 tempfile 目录后才实际运行。
所有原始开发日志保留在私密目录，精选历史及摘要见
[development-history](assets/cpar64-local-controls-20261022/development-history.json)。

## 冻结候选的协议与客户端验收

最终本轮公共协议回执运行 11 passed / 0 failed / 0 ignored。
入口均经真实 Messages HTTP、鉴权、生产 compiler/factory、实际 leased Build adapter 与独立 receiver。
单项 M3/M4 各四变体各一次发送；组合四变体各五请求，逐条比较独立输入 oracle，
保留两次相关工具调用及结果的完整关联、可见历史和后续问答。
标准 JSON 与 SSE 语义一致，1/7/31 字节分片都通过。

20条组合回执每条均核对 input=20/output=7、客户端/接收/override 模型 grok-4.5、
两个有序 summary item 的加密授权读回、M4 原值读回、context_kept=true 和唯一终态。
每变体 Request/Attempt/Usage/ledger/receiver 均精确为5、lease=0；
按真实 Request/Attempt/Usage source_event_id/ledger 做独立 join，不只比总数。
M4 不进入原始 outbound header、body、model input、公共响应或普通事件。
记录见 [protocol receipts](assets/cpar64-local-controls-20261022/protocol-receipts.json) 和
[有界验收日志](assets/cpar64-local-controls-20261022/protocol-acceptance.log.gz)。

M1/M2/M5 七个非空组合在 JSON/SSE 各覆盖，合法 M4 仍读回但 receiver=0、无 Attempt/Usage/ledger。
非法 M4、重复字段、超界、M3 非法 edit/keep、原生推理历史和工具结果块继续预拒绝；
后续历史另有首轮200、次轮非法历史400的重新检查。
空候选在 production compiler 被拒绝，混合/其他 adapter/mode 另有公共入口零接收反例。

原样 Claude 的入口、symlink、mode、arm64 文件、冻结副本与隔离运行时均重查：
2.1.302，SHA-256 `5262ce9de86243fed0d21edc39b271555d56780d0112e2b194439fe1c6662d37`。
OS 探针证明 IPv4/IPv6 外发、无关 loopback、真实 home/外部凭据读取、外部写入及子进程探针被拒绝；
独立无关 receiver=0，外部 sentinel 未变。没有读取真实凭据或更改客户端安装。
原样两个请求仍保留 M1/M2/M5 并返回400；两个不同认证 Request 的 user_id 长度均150，
完整 raw field 的授权读回均通过，source=v2、实际 Attempts=0。
Request/FailedFinished=2/2，receiver/Attempt/Usage/ledger/summary=0；lease=0。
原始 strict driver 外层 exit=1 正是兼容阻塞，内部观察测试 exit=0；没有改用能掩盖失败的成功判断。
见 [actual Claude summary](assets/cpar64-local-controls-20261022/actual-claude-summary.json)。
没有真实工具执行、summary、delta/lease 或 in-flight cancellation，因此不授予这些实际客户端结论。

严格实际 Codex/Pi 回归保持原驱动和所有门槛：Codex CLI 0.161.0、Pi 0.84.3、Node v26.8.2；
安装文件前后未变。七场景全部执行；Codex 六项成功门槛全 true，两个实际 PID 完成正常工具与磁盘续接。
正常 Codex/Pi 各五次独立接收/Attempt/Usage/ledger，真实 result.txt 均为 `42\n`。
Pi 的额外 background400 仍作为负探针保留，Request/Finished=6，不把它计为第六次成功。
Codex C4 对每次出现的原值继续按真实 owner/Attempt 完整读回，未转发；缺失也有独立证明。
失败场景各一次实际 Attempt 与已知 Usage，取消时 CLI 同进程仍活着，观察到 peer close 和 lease=0 后才清理。
Pi/Codex 本次 abort→release 采样上界分别16/14ms，一秒及 bounded eventual 均通过；这不是延迟 SLA。
详见 [actual Codex/Pi summary](assets/cpar64-local-controls-20261022/actual-codex-pi-summary.json)。
原驱动的 C4 presence oracle 三项测试也通过。

## 定向真实进程恢复

复用 #65 的 TLS shim、OS 隔离、synthetic control-plane/key fixtures、管理面备份/恢复与新进程接管设施；
不机械重复其完整历史矩阵，也不新增通用证据平台。
旧 schema37 binary 从实际基线4099重新构建，身份见
[schema37 binding](assets/cpar64-local-controls-20261022/schema37-binding.json)，
不使用来源绑定不足的开发期缓存推定最终结果。

267 项检查全部通过，8个实际 serve 进程均 readiness 成功、exit=0 并被回收。
8次 metadata 捕获包含3份旧v1、5份v2，其中一份v2为400 Request-only。
每个成功捕获恰一次完整 HTTP 接收，总计7 POST；TLS shim 总 accept=8，
另一次来自流量前明确的 owned-loopback 连通性探针（既有恢复驱动第307行），不是额外 HTTP 请求。
接收端所有 body/header 均不含本地 metadata/context controls。

迁移和真实 PID 重启前后快照一致，旧密文与 receipt 原样保留；
新 reader 读 v1/v2、旧 reader 拒绝新 schema、外属 owner/其他真实 Attempt 拒绝，实际 Attempt 选择通过。
幂等重放不延长 TTL、冲突拒绝；轮换到 key2 后旧数据可用，缺旧 key/错误 key 失败关闭。
expiry 进程部分使用显式 SQL retention-clock 故障夹具 `(created,expires)=(0,1)` 触发真实 GC，
验证 NULL versioned receipt、重启及防复活；不声称等待了30天或该时钟夹具保留原 AAD。
有效 AAD 与精确排他 TTL 边界由新增存储集成测试使用受控 now 参数验证。

管理面恢复写入独立 target，再由新的 stock gateway 作为 control.sqlite3 接管，
验证授权读回、公开模型视图、receipt 状态与防重放；不是仅重新打开一个恢复 reader。
没有替换生产数据库或共享状态。完整定向回执见
[process recovery](assets/cpar64-local-controls-20261022/process-recovery-summary.json)。

## 最终 Full、独立审查与交付

Full 运行于独立 detached worktree `cpar64-m34-full-20261022-9035551d`、全新 target，
实际代码/manifest/lock 与明确18文件 overlay 一并冻结。
7,271个输入在 Full、实际客户端、有界协议回执与进程矩阵后内容、mode、uid/gid、link 完全相同；
前后输入 manifest SHA-256 均为 `6d8d2a49589bcda7e7a791cc5ffe0d9afd339387addeeef8dea49488b41ed033`。
不是裸树 Full，也不是开发 target、旧 binary 或证据提交的重验。
身份与完整输入表见 [candidate binding](assets/cpar64-local-controls-20261022/candidate-binding.json)、
[source inputs](assets/cpar64-local-controls-20261022/source-inputs.json.gz)。

项目 Full exit=0、45/45，Rust1656/0/17，包含 #63 summary、#59失败 Usage、Codex C4、
取消释放、无重发与严格拒绝回归。见 [Full check](assets/cpar64-local-controls-20261022/full-check.md)、
[原日志](assets/cpar64-local-controls-20261022/full.log.gz)、
[Full summary](assets/cpar64-local-controls-20261022/full-summary.json)。
依赖策略输出 advisories/bans/licenses/sources 均 ok。RustSec 加载1348条 advisory、扫描366项 lock 依赖，
audit exit=0；其 registry yanked 查询超时，一次定向复查仍同样超时，故此项保持 UNKNOWN，
详见 [audit follow-up](assets/cpar64-local-controls-20261022/audit-yank-followup.log)。未为此更改锁文件或检查规则。

### Standards

首次独立只读审查覆盖 `4099d260...32da5424`：0项规范违反、0项需整改 smell。
修复后由新的干净探子只读复核 `32da5424...9035551d` 及 decoder/core 交互：仍0 findings。
两次均区分 tooling 与人工判断，没有替主代理执行修改或最终验收。

### Spec

首次独立只读审查对照新契约及前继规则，未发现遗漏、错误实现或越界；
修复后新的独立复核确认 raw token、缺失字段、唯一字段、先消费后 Build、旧断言与 M1/M2/M5 边界，0 findings。
静态 PASS 不替代本报告列出的动态测试；首次 Full 的真实回归保留且已修复。

分轴回执见 [initial reviews](assets/cpar64-local-controls-20261022/independent-reviews.json) 和
[fix reviews](assets/cpar64-local-controls-20261022/independent-fix-reviews.json)。
Standards 合计0 findings，Spec 合计0 findings；未合并或重排两个轴的结论。

未修改前端源码、真实账号/凭据、共享服务、全局配置或工单依赖；未进入 #66/#67，
未 push、创建PR、评论/关闭工单、发布或部署。
cost_in_usd_ticks 仍 NOT_ADMITTED；未知 native success companion、未收到的计数及真实费用保持 UNKNOWN。
没有测试白名单、任意 JSON 准入或把本地控制伪装成 Build 原生能力。
当前磁盘规则与 overlay 已明确记录，不声称本会话自动重新加载了任何继承规则。

全部原始失败记录、实际客户端原请求、私密读回及隔离数据库保留在
`/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-local-controls-20261022` 及回执记录的 OS 临时目录。
精选资产不包含实际 metadata 原值、解密数据、密钥或数据库；仅包含哈希、边界、统计及受控测试回执。
本轮完成标准是上述两项本地职责，#64整体仍 OPEN / PARTIAL / BLOCKED。
