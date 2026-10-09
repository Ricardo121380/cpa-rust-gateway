# CPAR 原生账号、会话续接与定向补证（2026-10-09）

本批从 `85a168da39e79b49c58e83767bdc2bbce0b8a00a`、分支 `codex/prism-v4-delivery` 接续。
授权范围为本地实施、合成材料、loopback、隔离检查、EgoLite、报告、本地提交和本批 GitHub 同步。
真实 Provider 调用为 0；真实账号、生产数据、push、远端发布、部署均未执行。
用户修改的 AGENTS.md 与初始 diff 字节相同；其余原有未跟踪文件保留。

## 源码与证据绑定

初次行为代码提交为 `eab0f044ee2c6679db4943772a6bf9d789d213fb`；归档格式修正提交 `2bbd48b9dbd487d8ffb0b02cf11edb3b69dab29b` 与它的 apps/crates/web/scripts、权威契约及行为契约 diff 为空。后续 Standards 展示/交接修正及当前干净门槛绑定 `3b4f78c86b0fad2d048c3e09df46f2ab964d60a3`。定向证据同时保留初始工作树记录和最终干净 checkout 回放，不能互换源码归属。
[原始定向索引](assets/cpar-native-session-batch-20261009/directed-evidence-index.json)记录命令、时间、退出码、源码及日志 hash。
[最终执行索引](assets/cpar-native-session-batch-20261009/final-evidence-index.json)保留本轮117份唯一原始执行回执、命令、时间及日志 SHA256。此前门槛为[45步日志](assets/cpar-native-session-batch-20261009/final-clean-full-steps.md)和[源码/环境/计数 attestation](assets/cpar-native-session-batch-20261009/final-gate-attestation.json)；最新源码独立复评与 GitHub 读回仍待最后收口。
[归档 manifest](assets/cpar-native-session-batch-20261009/manifest.json)只含安全回执、元数据和截图；临时 SQLite、凭据、TLS 私钥不归档。
归档日志仅规范多余空白尾行、cargo诊断的行尾空格并保留末尾换行；manifest 同列原始与归档 SHA256及具体转换。output 中原始日志及回执未改写。
失败保持原始唯一文件。仅实际页面旧实现的丢响应失败及缺失原生删除预览的 404 是行为 RED；脚本非法优先级、容量、模式、空目录或重复工具历史的失败是 harness 错误，不能当产品缺陷。

## 六票 AC 与边界

| 工单 | 实施与定向证据 | 本地结论与边界 |
| --- | --- | --- |
| #14 / CPAR-05 | 真实管理 HTTP 身份矩阵：Codex/Claude × same/different/unknown × enabled/disabled，共12组；首次创建随后维护。EgoLite 18项首次、同/异/未知身份、停用维护、callback丢响应、维护丢响应、绑定及发布失败 | 定向 PASS。HTTP 判断真实受控 identity guard；fixture 页面只投影这些已验证判定并验证控件、保存/应用、回读与不重放，不能称真实 OAuth 浏览器身份验收 |
| #16 / CPAR-07（当前 PARTIAL） | 实际 embedded /admin-ui/ Build grant；CDP 暂停成功响应前独立 SQLite 确认保存，再丢连接，关闭/离开/返回只 GET 同原会话；保存账号ID/revision、身份、runtime分别显示 | 有意义旧页面 RED、修复 GREEN；exchange1、账号1、创建1、cancel0。actor/TTL 单元和 HTTP 隔离分别记录。恢复只限原actor、原TTL、当前进程/管理会话，无跨进程 receipt 承诺。已有 Web/Console/零一多目标及停用保持复核 |
| #22 / CPAR-13 | 普通/native 页面分别启用停用；19项生命周期矩阵包含草稿、发布/应用失败、已应用、拒绝、丢响应先读。普通受控 TLS 长流/并发新请求与 native production facade held stream 分开 | 定向 PASS。停用撤去新分配和后续重试资格，在途自然完结；启用只恢复许可，auth/权限/额度/容量/continuity仍独立 |
| #23 / CPAR-14（新展示复验待定） | 原生 preview GET 列出精确连接、服务、public/upstream模型、停用状态、剩余账号；排除其他adapter。graph锁+库存代数+target revision同事务删除；批次新鲜预览只允许本批已确认移除项消失 | HTTP与页面定向 PASS；普通与native DELETE有独立长流/并发新请求证据，普通bootstrap失败删除后零fallback。实际普通/native请求、Usage、非空账本ID集合及审计/存储响应保留，共享连接/模型/其他账号保留；无替补保证、无上游注销承诺 |
| #36 / CPAR-27 | 保留获批严格Build正常刷新例外；真实生产worker、文件SQLite、publicJSON/SSE、证明range/CAS负例与实际stock进程重启 | 定向 PASS。两根首轮各1、刷新后续接各1，总推理4、刷新1、fallback0。手动重新授权后旧range不匹配新runtime revision，JSON/SSE拒绝且成功根仍4。非法/缺失/冲突/noncontiguous证明不生成成功根；普通OAuth/APIKey/Web/ConsoleSSO仍精确revision |
| #56 / CPAR-47 | Versions403及恢复、选择历史基线并核对非空差异/游标、compare403、两类audit403、backupPreview403、Settings403不可apply；保留恢复确认/备份加密与目标/unknown先读 | 新增家族定向6项 PASS；Preferences为仅内存会话偏好，管理资源权限拒绝N/A有源码依据。历史桌面/窄屏/主题/焦点/Escape证据仍仅证明各自原场景 |

## E-05 输入与断言地图

- `provider-grok/tests/p12_10d_native_account_workers.rs`：相同非空sub/client、scope集合顺序不同正例；missing/empty sub、同email异sub、异client、异scope不形成range；竞争CAS返回StaleClaim且原material/proof不被失败提交覆盖。
- `apps/gateway/src/credential_refresh/native_batch_a.rs` 联合 `runtime/m2_continuity.rs`：真实 RuntimeCredentialRefreshWorker 产生durable range，再重开NativeStore/EncryptedResponseStore和公共handler；JSON/SSE拒绝missing、conflict、noncontiguous、manual revision，实际transport计数不增，错误新root不存在且不能再次引用。
- `scripts/acceptance/cpar-e05-process-http.py`：固定生产OAuth issuer/CLI host、临时CA原hostname/SNI、OS deny非loopback；停止原PID、同state启动新PID，startup worker同claim/CAS提交1..2；私有推理内容与工具往返保留；foreign ClientKey与手动重新授权旧root拒绝。最后实际native DELETE后非空Request/Usage/ledger、4个成功root保留。
- `gateway-upstream`：Build范围仍检查expiry、capacity、health和账号；非Build OAuth/bearer/WebSSO/ConsoleSSO不得设置Build range，old revision拒绝、current exact revision可lease。
- `gateway-router::continuation_selection_requires_exact_lineage_revision_and_capability` 与 `continuation_once_keeps_exact_revision_and_never_falls_back`：精确Provider/upstream/channel/candidate/revision/capability和一次执行/RetryClosed；不同config/pin保持原准入。
- `cpar-batch-b-http.py`：410项公开HTTP与实际进程历史/compact/WS回归，记录foreign/expiry/corrupt/disabled/capability拒绝及受控receive计数。它覆盖通用共享边界，不替代Build正常刷新证明。

## 依赖票复核地图

八票初始自身 AC 已全部勾选，但OPEN且有原生依赖；按依赖顺序读取最终状态后收口。
#17混合导入结果/回读不重放、#20分离auth/runtime/health/quota/capacity、#24历史恢复账号清单与确认、#25精确隔离解除且零推理、#26显式Pin一次与证据过期、#37 compact AEAD/TTL/owner/restart、#38 WS鉴权/工具/取消/失败根、#55运行诊断失败/全局阻断/unknown回读各自保留原AC。
共享复核使用当前 A 的 native mixed/import、restore-deleted、local-release、channel-pin 页面，accountStatus定向单元，B 的 compact/WS HTTP，以及 D 的运行诊断场景；不因关闭前置就自动关闭下游。

## 最终定向回放与过程记录

- 干净 `eab0f044` checkout：生命周期页面19项、Codex/Claude页面18项、下游共享页面14项均PASS。#56本家族6项包括真实选定历史基线后的非空差异、Versions/compare两种拒绝、两类审计拒绝、backup preview及Settings拒绝。共享页中的旧配置恢复有恢复账号名单和显式确认；Pin打开/选择零推理、显式执行一次、条件变化失效。
- 实际普通管理HTTP/TLS进程：独立DELETE held stream自然结束，删除生效后新请求使用保留的sibling；bootstrap失败删除后后续重试零fallback。删除前Request/Usage/ledger ID分别7/3/3，删除后11/6/3，全部原ID保留。
- 干净 `2bbd48b9` stock build：实际启动worker、实际原PID停止及新PID启动、Build正常刷新后JSON/SSE续接通过；推理4、刷新1、fallback0、成功root4。之后native DELETE保留Request/Usage/ledger非空ID集合6/4/4及共享endpoint/model。外国ClientKey和重新授权后的旧root不增加推理或成功root。
- 实际 `/admin-ui/` 最终Build授权丢响应回读PASS，原actor读一次，账号ID/revision对应SQLite；exchange1、创建1、账号1、cancel0。外国actor GET404、poll/cancel503，未登录GET404。最终截图捕获了dialog过渡帧，不能作为文字可读性证明；同UI既有清晰截图与最终DOM/HTTP断言分开记录。
- compact/WebSocket及通用续接回归410项PASS，实际受控上游receive223与执行记录223相等；它不替代Build worker证明。前端70文件/500测试、type-check、check:full双构建均PASS。
- 两次Full失败均保留：首次在日志归档空白尾行失败，此前Rust1499通过；第二次在既有慢流时限测试报EgressUnavailable。该单项随后以完整测试名执行1项PASS。一次不完整`--exact`过滤产生0项，仅是执行入口记录，不能作测试通过证明。最终Full以原门槛重跑，未放宽时限或标准。
- 最终Grok入口一次缺少taskSpace参数、一次被macOS iCloud密码浮层遮挡，均保留失败；补齐调用参数并用Escape关闭浮层后完成实际产品验收。这两次不作为设备恢复行为RED。受控服务已按本任务PID/stop marker停止，EgoLite space37已finish一次。

## 冻结新候选的本地前置条件

| 条件 | 当前判定 | 依据 |
| --- | --- | --- |
| 本批必要实现/契约/定向证据 | PARTIAL | #16直接导航尚未动态复现；#23最新展示回放NOT_RUN；其他定向结果按上述六票地图保留 |
| 最终clean Full/前端double build | PASS_LOCAL | clean源码3b4f78c8、45步PASS、Rust1499通过/12忽略、前端500/type-check/双构建PASS |
| 两轴独立评审 | 待复评 | 第二次Standards有一项名称展示发现，Spec为一项P2未满足；最新修正源码须重新评审 |
| Grok Web原生多轮 | 未满足 | 仍是后续独立软件范围，本批未新增例外 |
| Kiro native输出硬上限 | 未满足 | 仍是后续独立软件范围 |
| 完整既有数据升级兼容及回退 | 未满足 | 仅有有界合成保留/恢复证据，不代表全量既有数据 |
| 当前真实渠道组合及声明冻结 | BLOCKED | 本批未授权真实账号/请求，真实执行0 |
| 新发布制品/签名/回退/远端CI/部署/生产 | NOT_RUN | 本批只本地源码实施，不push或发布 |

结论：冻结新发布候选的全部本地前置条件尚未满足。#57与父规格#9保持OPEN；E-05已裁定，不能再写“等待用户决定”。

## 初次评审后修正与当前待验边界（2026-10-10）

[初次两轴终态整理](assets/cpar-native-session-batch-20261009/review-initial.md)：Standards FAIL（三项文档标准违反及一项报告事实错误，smell 0）；Spec FAIL（一项 P2，直接浏览器 Back/Forward 绕过关闭时可能丢失恢复 handle，仅源码推断）。GitHub 在该评审时尚未写入。

当前展示修正使用既有 ResourceIdentity/复制入口，移除新增原生删除与完成页正文的内部主键；缺失姓名不回退导入标记。DESIGN §42 补充未知结果、批次维护和删除核对约定；Preferences 已更正为仅内存会话偏好。工作树 type-check 与 70 文件/500 项前端测试 PASS，不能替代最终 clean Full 或浏览器复验。

原 EgoLite space37 已提前 finish，恢复返回 `task space not found: 37`。已按 ego-browser Skill 请求允许新建一个空间；当前未获得答复。直接浏览器导航尚未动态复现，未据静态推断宣称行为 RED，也未执行该修复；#16 及其依赖收口需要这项补验。此前显式关闭后离开/返回的实际嵌入页面记录仍是该明确路径的有效证据。

待复现服务已按本任务 stop marker 正常停止；该执行回执只记录预备服务生命周期，不是直接导航行为 RED。


## 评审修正候选的干净检查

展示及交接修正源码 `3b4f78c86b0fad2d048c3e09df46f2ab964d60a3` 的干净隔离 checkout 已完成 Full 45 步 PASS、Rust1499通过/0失败/12忽略、前端70文件/500测试 PASS、type-check PASS；Full 的 Management SPA 步调用 check:full，完成双构建。stock gateway 已按该源码构建，但新页面验收未执行。详见 [新45步](assets/cpar-native-session-batch-20261009/review-clean-full-steps.md)及[新候选 attestation](assets/cpar-native-session-batch-20261009/review-gate-attestation.json)。

该源码的 apps/crates/权威管理契约/行为契约相对 eab0f044 diff 为空；此前 E-05、OAuth 和持久化记录按各自 source 保留。本次展示改动的实际页面回放仍待新空间确认；不能把更新后的 lifecycle 脚本当作已经重跑。

## 第二次评审与普通删除预览修正

[第二次两轴终态整理](assets/cpar-native-session-batch-20261009/review-second.md)绑定 `184f9daa`：Standards FAIL（一项普通删除预览名称展示违反，smell 0）；Spec PARTIAL（一项 P2 直接导航未满足，仅源码推断）。#14、#36、#37、#38 无独立阻塞项，可在本轮规范修正与最终门槛后单独收口。#16及仍未解除前置的其余票保持OPEN。

普通删除预览已复用 ResourceIdentity，路由名称使用既有 version-scoped 路由与模型库存；精确主键继续用于 DOM 机器引用、API及删除核对。浏览器断言改为名称与精确引用分别检查。工作树 type-check、70文件/500项前端测试及验收脚本语法检查 PASS；尚不代表最新干净 Full 或页面回放。

直接导航复现和最新普通/原生展示回放仍为 NOT_RUN，等待已发送的新 EgoLite 空间许可。未对直接导航推断实施修复，未宣称行为 RED 或该路径 PASS。
