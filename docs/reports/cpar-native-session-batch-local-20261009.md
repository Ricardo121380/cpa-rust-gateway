# CPAR 原生账号、会话续接与定向补证（2026-10-09）

本批从 `85a168da39e79b49c58e83767bdc2bbce0b8a00a`、分支 `codex/prism-v4-delivery` 接续。
授权范围为本地实施、合成材料、loopback、隔离检查、EgoLite、报告、本地提交和本批 GitHub 同步。
真实 Provider 调用为 0；真实账号、生产数据、push、远端发布、部署均未执行。
用户修改的 AGENTS.md 与初始 diff 字节相同；其余原有未跟踪文件保留。

## 源码与证据绑定

当前定向验证为未提交工作树证据，最终干净源码 Full、EgoLite 回放、独立评审和 GitHub 读回在后续收口追加。
[原始定向索引](assets/cpar-native-session-batch-20261009/directed-evidence-index.json)记录命令、时间、退出码、源码及日志 hash。
[归档 manifest](assets/cpar-native-session-batch-20261009/manifest.json)只含安全回执、元数据和截图；临时 SQLite、凭据、TLS 私钥不归档。
失败保持原始唯一文件。仅实际页面旧实现的丢响应失败及缺失原生删除预览的 404 是行为 RED；脚本非法优先级、容量、模式、空目录或重复工具历史的失败是 harness 错误，不能当产品缺陷。

## 六票 AC 与边界

| 工单 | 实施与定向证据 | 本地结论与边界 |
| --- | --- | --- |
| #14 / CPAR-05 | 真实管理 HTTP 身份矩阵：Codex/Claude × same/different/unknown × enabled/disabled，共12组；首次创建随后维护。EgoLite 18项首次、同/异/未知身份、停用维护、callback丢响应、维护丢响应、绑定及发布失败 | 定向 PASS。HTTP 判断真实受控 identity guard；fixture 页面只投影这些已验证判定并验证控件、保存/应用、回读与不重放，不能称真实 OAuth 浏览器身份验收 |
| #16 / CPAR-07 | 实际 embedded /admin-ui/ Build grant；CDP 暂停成功响应前独立 SQLite 确认保存，再丢连接，关闭/离开/返回只 GET 同原会话；保存账号ID/revision、身份、runtime分别显示 | 有意义旧页面 RED、修复 GREEN；exchange1、账号1、创建1、cancel0。actor/TTL 单元和 HTTP 隔离分别记录。恢复只限原actor、原TTL、当前进程/管理会话，无跨进程 receipt 承诺。已有 Web/Console/零一多目标及停用保持复核 |
| #22 / CPAR-13 | 普通/native 页面分别启用停用；19项生命周期矩阵包含草稿、发布/应用失败、已应用、拒绝、丢响应先读。普通受控 TLS 长流/并发新请求与 native production facade held stream 分开 | 定向 PASS。停用撤去新分配和后续重试资格，在途自然完结；启用只恢复许可，auth/权限/额度/容量/continuity仍独立 |
| #23 / CPAR-14 | 原生 preview GET 列出精确连接、服务、public/upstream模型、停用状态、剩余账号；排除其他adapter。graph锁+库存代数+target revision同事务删除；批次新鲜预览只允许本批已确认移除项消失 | HTTP与页面定向 PASS；普通与native DELETE有独立长流/并发新请求证据，普通bootstrap失败删除后零fallback。实际普通/native请求、Usage、非空账本ID集合及审计/存储响应保留，共享连接/模型/其他账号保留；无替补保证、无上游注销承诺 |
| #36 / CPAR-27 | 保留获批严格Build正常刷新例外；真实生产worker、文件SQLite、publicJSON/SSE、证明range/CAS负例与实际stock进程重启 | 定向 PASS。两根首轮各1、刷新后续接各1，总推理4、刷新1、fallback0。手动重新授权后旧range不匹配新runtime revision，JSON/SSE拒绝且成功根仍4。非法/缺失/冲突/noncontiguous证明不生成成功根；普通OAuth/APIKey/Web/ConsoleSSO仍精确revision |
| #56 / CPAR-47 | Versions403及恢复、选择历史基线并核对非空差异/游标、compare403、两类audit403、backupPreview403、Settings403不可apply；保留恢复确认/备份加密与目标/unknown先读 | 新增家族定向6项 PASS；Preferences为本地sessionStorage偏好，管理资源权限拒绝N/A有源码依据。历史桌面/窄屏/主题/焦点/Escape证据仍仅证明各自原场景 |

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

## 冻结新候选的本地前置条件

| 条件 | 当前判定 | 依据 |
| --- | --- | --- |
| 本批必要实现/契约/定向证据 | 定向PASS，最终提交及收口待追加 | 上述六票地图，单向OpenAPI生成客户端 |
| 最终clean Full/前端double build/两轴独立评审 | PENDING | 需固定最后行为源码后执行，不能用旧029332回执代替 |
| Grok Web原生多轮 | 未满足 | 仍是后续独立软件范围，本批未新增例外 |
| Kiro native输出硬上限 | 未满足 | 仍是后续独立软件范围 |
| 完整既有数据升级兼容及回退 | 未满足 | 仅有有界合成保留/恢复证据，不代表全量既有数据 |
| 当前真实渠道组合及声明冻结 | BLOCKED | 本批未授权真实账号/请求，真实执行0 |
| 新发布制品/签名/回退/远端CI/部署/生产 | NOT_RUN | 本批只本地源码实施，不push或发布 |

结论：冻结新发布候选的全部本地前置条件尚未满足。#57与父规格#9保持OPEN；E-05已裁定，不能再写“等待用户决定”。
