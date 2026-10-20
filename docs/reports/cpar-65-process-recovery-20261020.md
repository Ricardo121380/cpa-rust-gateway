# #65：诊断证据的真实进程重启与升级恢复受控切片

日期：2026-10-20（Asia/Shanghai）。**受控切片 PASS_BOUNDED_PROCESS_RECOVERY；最终 overlay Full 45/45 PASS。**
本轮仅承接 [#65](https://github.com/Ricardo121380/cpa-rust-gateway/issues/65)，父规格
[#58](https://github.com/Ricardo121380/cpa-rust-gateway/issues/58)。**整票保持 OPEN / PARTIAL / BLOCKED_BY_63_61**；
原生 #65 → #63 → #61 依赖不变，未关闭工单、推进 #66/#67 或更新/合并 develop/main。

## 范围、基线与保留

最新实现基线为 #64 工作树的 `370604297a956047fda4d02c78ca3b0f7fc8cbd5`，本地与远端一致，
包含最终 #64 代码 `6edb8085f9e850edc55e68d382ed97acbc54a0c5`。主目录仍在 #62 的
`098c92a51ac549a38c8f15fd265e0fb379de2c77`，没有把它误作最新实现，也未按历史 SHA 回退。
本轮建立 `feature/65-evidence-process-recovery`，工作树为
`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar65-evidence-process-recovery-20261020`。

原主目录 9 tracked / 81 untracked、#64 目录 9 tracked / 7 untracked，两个 index 均为空。
全部 106 个原文件的字节、mode、link、HEAD、分支及完整 status 独立核对不变。
16 份既有规则/文档 overlay 原样保留，只进入 validation-only 本地提交，未混入 feature 提交。
主代理亲读 AGENTS、CONTEXT、ADR 索引、架构/crate/quality 文档、#59/#63/#64 报告及
ADR-0074/0109/0112/0114、BC-OBS-006/007 和备份恢复契约；当前工单正文、评论、依赖均另行冻结。

本轮只验证 schema35 已交付的**失败 Build 完整 NativeUsageEvidence**、schema36
**MessagesSummaryEvidence** 和**没有 companion 的旧/缺失历史 UNKNOWN**。
`cost_in_usd_ticks` 继续 NOT_ADMITTED；#61 尚未准入的成功 Responses 元数据和真实模型关系不获替代资格。
#64 的原样客户端请求、准确预拒绝及实际回答/工具/摘要/取消阻塞保持，未改请求策略、删字段或重做无新依据的研究。

## 实现与不可变输入

产品提交 `ddad4d634c9688d2ed3df066fafd3f28f29b2631` 添加三文件；
`5f3c2b4035dca3aa20a74bb086afab1295b19c7d` 最小修复摘要内容 oracle。
没有生产 Rust、schema、Cargo manifest/lock、依赖或客户端策略修改。

- [可复现进程驱动](../../scripts/acceptance/cpar65-process-recovery.py)：拥有式合成状态、真实 serve、管理/公共 HTTP、独立 TLS receiver、阶段身份与状态核对。
- [operator helper](../../apps/gateway/tests/cpar65_process_support.rs)：ignored、test-only，调用现有 encrypted backup 和 queue/writer replay；不充当验收服务器。
- [隔离环境操作说明](../support/evidence-process-recovery.md)：构建/reader、外部 keys、空 target 恢复与真实采用流程。

最终验证源码为 `7dfe34ab3ff177dd7cb3fb211a80be169bffd811`，等于产品源码 `5f3c2b40` 加原16份 overlay。
验证树 `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar65-process-20261020-fixed`，
独立 target 为 `/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar65-20261020/target-full-final`。
实际历史 schema35 源码为 `098c92a51ac549a38c8f15fd265e0fb379de2c77`，使用独立源码树和
`target-schema35` 从自身 manifest/lock 构建，未移动其它工作树的测试二进制。
source/tree、全部输入、manifest/lock、overlay、构建 artifact、serve/reader/operator SHA 和阶段配置见 source-manifest。
后续报告/证据提交不改变这些执行输入，不冒称它们已经包含在先前 Full 中。

## 真实执行链与隔离条件

全部验收服务是 stock `gateway serve` 子进程，经生产 bootstrap、配置加载、两监听器、compiler/factory、
实际 leased Build adapter、原生 encoder、生产 egress/DNS-pinned pool/TLS、decoder、writer 与投影链。
每次显式 `--background-provider-tasks disabled`，传入私有 state/credentials/key ring 和 active version。
readiness 验证管理鉴权及数据面 401；SIGTERM → wait → exit0 → 原 PID 已不存在 → WAL checkpoint 后才进入下一阶段。
异常清理退出独立登记，不用 cleanup 结果替代场景内停止。

复用现有 macOS acceptance-only `cpar-device-loopback.c`，只在拥有的子进程中改变 DNS/connect 的最终 peer。
固定 URL 仍为 `https://cli-chat-proxy.grok.com/v1/responses`，完整 auth/header、模型 override、egress 校验、
TLS hostname/SNI 校验均保留；合成 fixture 采用显式 host-sized `127.0.0.1/32`、独立合成 CA。
Seatbelt 仅允许 receiver 端口出站，IPv4/IPv6 文档地址、无关 loopback 均 EPERM；另一个 receiver accepts=0。
同一生产 TLS 链对不可信 CA、错误 hostname 各有一次 TCP accept、零完整 HTTP receive，返回503/EgressUnavailable。
正常证书和原样请求可执行。合成 DNS/CA/peer 不授予公网、真实 Provider TLS/认证或 DNS-rebinding 资格。
没有新增生产 endpoint override、关闭证书验证、放宽鉴权/协议或调用真实收费 Provider。

## 恢复矩阵与不变量

| 阶段 | 已验证行为与证据边界 |
| --- | --- |
| B 非空35→36 | 真历史 serve 建普通/原生账号、两条停用意图、两份 native identity observation、模型开放/客户端权限、两条自定义连接/路由；公共请求生成旧 Usage/账本及无 companion 历史。新版生产 bootstrap 升级，旧表全部列及行内容哈希不变，新增 summary namespace 初始为空。没有 down migration 伪造旧状态。 |
| A schema36 正常重启 | 两类 JSON/SSE 证据真实写入后正常停止/新 PID 重启。exact owner/Request/Attempt、来源、response ID、完整 Usage 字节/精确数字、两 item/三 part 的全部文本与顺序、缺失/空 body、完整性均授权读回，所有既有业务行不变。 |
| C 轮换与错误钥 | 保留 key1，active 切key2，真实新进程旧证据仍可读，新两类证据 key_version=2；旧 ciphertext 不 reseal。key2-only reader 可读新证据，不能解开旧证据；缺/错key认证失败，同组异主UNKNOWN，交换 ciphertext 的隔离副本认证失败。 |
| 真实写故障 | 两类 INSERT trigger 仅为标注的合成 SQL fault。生产公共请求各只被接收一次，先返回503/RecordingUnavailable。解除故障后原 pending transaction 完成，授权读回可确认晚到持久化；原 RequestFinished 仍 failed，每次单 Usage/source/ledger，无透明重发。不能把晚到 commit 说成客户端曾成功。 |
| expiry/receipt | 合成 SQL 时钟fixture将原记录 retention index 置0/1；生产 GC清除 payload。summary保留无内容 Attempt receipt，Native Usage保留普通 source-event 防重建屏障。相同与有效不同内容 replay 经现有 writer 失败，key2重启与恢复后仍不重建/续TTL。此 fixture 不冒充自然经过30天或公开请求生成的证据。 |
| D 加密恢复并服务 | 用独立 Backup Key 对一致整库 snapshot AEAD备份，保留key1/key2和其它必要credentials。真实管理restore在空target生成restore-target.sqlite3；停止host后，在独立空serving-state采用它作为control.sqlite3，再由新serve实际启动。完整业务行、公开模型/权限/停用状态、两类授权读回、旧UNKNOWN、receipt和账本保持。source/host/target及新历史均保留。 |

完整业务比较包含20类表的全部列和值（B升级前无summary表时为19类），对 B/A/C启动/D恢复分别比较，
不是总行数比较。schema_migrations 允许新增36；quarantine 可记录合成 replay 冲突；materializer/checkpoint
运行元数据不冒充业务变化。未完成 SSE与TLS失败没有已验证Usage/ledger，读回UNKNOWN；费用也UNKNOWN。

## schema / binary / reader / key 兼容

| 状态 | 实际 binary / reader | 结论 |
| --- | --- | --- |
| 非空schema35 | 固定098c92a5历史serve和usage-evidence reader | 完整失败Usage JSON/SSE PASS；该源码无summary命令，静态确认，未用其冒充summary reader。 |
| schema35 | 当前固定reader，read-only | NativeUsage PASS，summary UNKNOWN；读回不迁移schema。 |
| schema36 | 历史098c92a5 usage reader | 实际非零拒绝、不创建明文输出；不从版本号猜兼容。 |
| schema36，key1+key2、active2 | 当前serve/reader | 旧key1及新key2两类证据PASS，业务身份和权限保持。 |
| schema36，只有key2 | 当前reader | 新key2记录PASS；旧key1记录错误。此结论不声称只有key2可以bootstrap含旧凭据的完整服务。 |
| schema36，错key / ciphertext交换 | 当前reader，独立故障副本 | 认证失败，无明文文件；外属/缺失/过期同为UNKNOWN。 |
| schema34 | 当前reader的既有schema-prefix存储夹具与Full回归 | 非历史serve升级证明。按契约无companion为UNKNOWN；本轮未新建schema34历史binary演练，不能扩展B的35→36结论。 |
| encrypted36恢复 | 当前真实serve/reader，原Backup Key和全部Master Key版本 | 实际启动采用恢复control.sqlite3，readback/receipt/业务state PASS；binary-only回退和destructive down migration均非恢复流程。 |

每个动态结论绑定 matrix 中实际 SHA、key-file hash、argv、PID、状态路径与授权输出hash。
私有明文只经授权reader写入canonical0700目录下独占0600文件；数据库、备份、keys、bearer及这些明文不进入Git。
普通事件/quarantine/serve/operator日志用本轮真实合成秘密和私有marker逐项检查。SQLite GC仅为逻辑清除，不主张介质、WAL或旧备份物理擦除。

## RED、验证与两轴评审

旧设施缺口另存baseline-gap：T12固定schema32→34，未覆盖新证据namespace，部分启动未默认关闭Provider后台任务；
#59/#63既有恢复只启动reader CLI，不能当serve恢复。原设施未被改版本断言后冒充通过。
开发期Python cwd隔离、重复policy名称、重复凭据导入去重、空Responses输出、非法created Usage、错误SSE失败帧oracle、
TLS公共状态/SNI oracle错误均保留为harness失败；没有据此修改生产逻辑、放宽协议、增加timeout或隐去旧失败。

初次Standards/Spec发现同一P2：首次summary只查结构和数量。主代理在真实authorized-reader输出上做受控变异，
旧oracle错误接受改text、part调序、改responseID三例；修复后独立literal完整item oracle三例均拒绝，再重跑整个矩阵。
这是验收oracle缺陷，不是生产数据损坏证据。RED/GREEN、初审与最终分轴报告全部保留。

最终Full、进程矩阵、来源身份和计数见下方回执汇总；Standards PASS（0硬违规/0未解判断项），
Spec PASS_BOUNDED_IMPLEMENTATION，整票PARTIAL/BLOCKED。评审本身为只读静态复核，未冒称子代理重跑Full。

## #65逐项AC

| 原序 | 最终状态 | 说明 |
| --- | --- | --- |
| 1 新证据/旧未知/非空状态的真实重启升级恢复 | PARTIAL / BLOCKED | 失败NativeUsage、MessagesSummary与旧UNKNOWN的受控切片PASS；成功Responses额外元数据仍依赖#61，#65→#63→#61不解除。 |
| 2 exact owner/Request/Attempt及精度完整性、公开入口和真进程 | PASS（受控范围） | 生产serve公共请求、独立receiver、授权reader、完整独立原文oracle与实际新PID。 |
| 3 key轮换、缺key/交换/expiry/异主 | PASS（受控范围） | 真进程active2读旧/写新；各reader负例与重启/恢复后的receipt同/异replay覆盖。 |
| 4 账号/停用/模型权限/custom routes/Usage/账本/旧历史 | PASS（受控范围） | 完整行语义哈希与恢复后公开入口核对；无清空/重导入、无补零或历史enrichment。 |
| 5 适用schema/reader与一致恢复、保留新历史 | PASS（仅35→36） | 真实历史35→36、实际旧reader拒36、整库AEAD与恢复后新serve；schema34仅既有契约/存储夹具边界。 |
| 6 隔离合成、固定身份及恢复说明 | PASS（受控范围） | 私有全新状态、OS限制、独立receiver、source/binary/reader/key/config/overlay绑定；真实Provider及共享服务未运行。 |

**overlay Full、真实进程验收、远端CI、真实Provider、发布/部署分别报告。** 纯产品树不带既有overlay的Full为NOT_RUN。
#64真实回答、实际两工具循环、实际摘要读回及活客户端取消仍BLOCKED。下一步回到#61固定Build JSON/SSE/profile字段资格，
以及#62/#64实际客户端必需控制与原样历史的准确契约，资格成立后再做真实工具/续轮/取消验收。
不凭本恢复切片宣称三客户端首版可用，不重新开展无新证据的字段资格研究。

## 最终执行回执

最终进程验收使用上述隔离验证树实际构建的二进制，**823 checks 全通过、13 个不同 serve PID 全部正常退出并收割**。
独立 receiver 为 **14 TCP accepts / 11 完整 HTTP requests**：其中额外3次分别是1次隔离正探针和2次TLS拒绝；没有后台Provider请求。
恢复后为 **13 Request / 13 Attempt / 13 RequestFinished / 10 Usage / 10 ledger**，
账本已知input200/output70，priced_rows=0；这不是Provider费用为零，费用继续UNKNOWN。
过期后NativeUsage live=3（key1一份、key2两份），summary总4行（3份live ciphertext和1份无内容key1 receipt）。

| 固定执行输入 | SHA-256 / 身份 |
| --- | --- |
| 当前 serve 与两种授权 reader | `8ec06f330f099f4111c74e15c149cda9155c03d640985ed930ac55e37d7a076f` |
| 历史 schema35 serve 与 Usage reader | `7c14f0754b7108868cef79a2f91145ce9e00875c130ec4111a719fb213d8ff41` |
| 当前 test-only operator | `87f8c1e320dc3ed49ae4550619fe559c7393564336d85a484492bd92bf167f32` |

最终 **overlay Full 45/45 PASS**，Rust **1575 passed / 0 failed / 16 ignored**（128组），758.733秒。
6994个验证输入在Full前后、最终build和进程验收后再次核对不变；三个产品输入与提交5f3c2b40逐字节一致。
serve/reader与operator实际Cargo artifact、feature/profile、manifest/lock及源码来自同一验证根；
不把测试profile binary与stock serve宣称为同一个文件。16个ignored含需opt-in的真实客户端与operator；
本轮operator及真实serve矩阵已另行实际运行。

| 验证层 | 结果 |
| --- | --- |
| 定向真实进程恢复矩阵 | PASS_BOUNDED，823/0；固定最终binary/source与独立receiver |
| 最终overlay Full | PASS 45/45；1575/0/16，输入不变 |
| Standards | PASS，0硬违规/0未解决判断项，初审P2已RED并修复 |
| Spec | PASS_BOUNDED_IMPLEMENTATION；整票PARTIAL/BLOCKED_BY_63_61 |
| 纯产品树、不带既有overlay的Full | NOT_RUN |
| 远端CI | 本报告冻结时尚未推送；最终交付与#65评论按推送SHA独立回读，本地Full和push不替代CI |
| 实际客户端/真实Provider/发布/部署 | NOT_RUN；#64既有实际客户端BLOCKED不改变 |

- [进程与状态、计量、备份/restore摘要](assets/cpar-65-process-20261020/process-summary.json)
- [完整阶段、reader授权与独立接收回执](assets/cpar-65-process-20261020/process-matrix.json)
- [source、binary、manifest/lock、overlay与构建身份](assets/cpar-65-process-20261020/source-manifest.json)
- [最终构建与执行输入闭合](assets/cpar-65-process-20261020/final-binding.json)
- [Full回执](assets/cpar-65-process-20261020/full-final-receipt.json)与[45步记录](assets/cpar-65-process-20261020/full-final-check-report.md)
- [Standards](assets/cpar-65-process-20261020/review-standards-final.md)、[Spec](assets/cpar-65-process-20261020/review-spec-final.md)、[初审](assets/cpar-65-process-20261020/review-initial.md)
- [oracle RED](assets/cpar-65-process-20261020/review-oracle-red.json)和[GREEN](assets/cpar-65-process-20261020/review-oracle-green.json)
- [既有工作区保留证明](assets/cpar-65-process-20261020/preservation.json)
- [推送前远端分支、工单稳定字段与原生依赖](assets/cpar-65-process-20261020/github-prepush.json)
- [保留失败/Full/矩阵的原始日志包](assets/cpar-65-process-20261020/verification-logs.tar.gz)与[逐成员索引](assets/cpar-65-process-20261020/archive-index.json)

原始日志包仅含受控日志、回执和输入hash；不包含数据库、密钥、加密备份本体、reader私有明文、用户diff或原overlay内容。
解压成员按原项目secret规则与禁止路径规则逐项检查，再按字节/hash回读验证。报告和证据增量仅做静态检查，不重复声称执行Full。

完整6994文件输入清单及完整Cargo构建artifact流因既有source/target文件名触发secret文本规则，
仅保留在本地并由SHA绑定，未加入日志包；实际gateway/operator artifact身份仍在source-manifest。
详见[排除说明](assets/cpar-65-process-20261020/archive-exclusions.json)。未改变原始清单、构建流或源码。
