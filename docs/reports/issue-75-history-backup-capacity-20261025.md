# #75 大历史查询与加密备份恢复容量闭环

日期：2026-10-25（Asia/Shanghai）。**PASS_LOCAL_CAPACITY_AND_FULL**。
仅在独立本地工作区实施、验证和提交；GitHub #75 仍由操作者决定远端处理。

## 来源与工作区

- 共同基线：`84fbfbf778b582f746c1cafe31a2ce58962e8f77`，已核对容器
  `maintenance/2026-10-25-parallel-implementation-audit/INTEGRATION-RESULT.md` 与实际 Git。
- 分支：`feature/75-history-backup-capacity`。
- worktree：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar-75-history-backup-capacity-20261025`。
  App worktree 工具在容器目录返回 Not a git repository 后，在实际 Git 仓库创建独立 worktree；
  没有 reset、clean、stash 或触碰其他分支的增量。
- 合格原版使用集成记录冻结的 `6a7ae73e79b57576ee37b3ace1d70d60f1909a9b` binary；
  `6a7ae73e..84fbfbf7` 只有八个 docs 路径，所有 Rust/Cargo/lock 与新共同基线相同。
- 实际源码/驱动验收 HEAD：`656c99b2fad042ba57030c4d4c1c40626cc87045`；
  tree `8920c01b6c970d38d63ef14586cbbd8888820298`。构建提交
  `875ea3590dab23c635699c1f34c55c690e6f1885` 至该 HEAD 只改变并发观察的驱动脚本，Rust/Cargo 完全相同。
- Cargo.lock SHA-256：`a3950a764ae2dcae4e04a8a32ada596f33ea105dbfda8e9714b3e32d3216cd6d`。
  本票没有新增依赖、修改 lock、迁移或配置；数据库仍为 schema39。
- 固定 CPA：v8.0.40 / `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`。
  实际对照 `internal/api/handlers/management/usage.go::GetUsageQueue` 与
  `internal/redisqueue/queue.go::PopOldest`；它们是消费队列，不能当作 CPAR SQLite 历史或恢复规范。
  本票未复制上游代码/fixture，没有跟随浮动 main 改变预期。

## 冻结数据与先测量的边界

[冻结计划](issue-75-capacity-plan-20261025.md) 保留最初规模、字段和并发约定，
也如实记录独立审查纠正账本夹具的过程。初始账本克隆指纹/retention 不足以证明有效重放；
早期结果没有当作合格基线。最终 baseline-06 用原版 stock gateway 的 BillingWorker 生成规模账本，
确认 current、零未解决失败，再测量原版备份上限。

- 固定 30 天内 25,000 条合成完整 Request/Attempt/Usage/terminal，100,000 events，
  普通成功/原生失败 Usage/原生 Messages 成功模板为 8:1:1。六种 token 的 null/zero 和来源保持；
  费用全部 unpriced/null，不据此估计真实账单。
- 40,000 条 schema39 quota history，两个 namespace/window、observed/exhausted/unknown/failed 与 partial。
  这是明确的直接 SQL 容量夹具，不冒充 #74 revision-fenced 上游观察证据。
- 公共 HTTP 的小型受控 TLS seed 另保留 4 个真实网关合成请求、25,004 行总账本、
  native Usage evidence 1、Messages summary 2、request metadata 1；没有复制 ciphertext 到新 AAD 身份。
- 账号、普通/native 停用意图、权限组、三个客户端 Key 身份和两条路由来自 #65 的既有 seed。
- 物化前种子库 84,889,600 bytes；合格物化后源库 **98,541,568 bytes**。
  原 98,484,224-byte 结果属于已纠正的早期夹具，不是最终对照。

| 合格原版测量 | 结果 |
|---|---|
| 选择性 #72 快照（300 个 Attempt） | 0.422 秒，完整分页和 CSV/JSONL 一致 |
| 全窗口快照 | 1.088 秒，明确 413，未截断为成功 |
| #74 quota 首屏 | 0.176 秒，200 条，未请求上游 |
| 加密备份 | 1.044 秒后 `MaterialTooLarge`，exit101；无产物，原始业务表摘要未变 |
| 原版备份 helper OS high-water RSS | 11,272,192 bytes |

该规模没有暴露必须改变查询限制的问题，因此保留查询 SQL、20,000 events/16 MiB source、
8 MiB result、32 MiB retained cache 和分页限制。没有用加大内存缓存或删除历史解决备份容量。

## 实施

[ADR-0132](../adr/ADR-0132-bounded-disk-backed-backup.md) 扩展既有
[BC-MGMT-007](../contracts/BC-MGMT-007-encrypted-control-plane-backup.md)：

1. 磁盘快照按 64 KiB 逐块 XChaCha20-Poly1305 加密；新 v2 格式认证完整 header、总明文长度和块序号，
   每块使用互异 nonce，并要求完整块数及 EOF。旧 v1 保留 16 MiB 边界和读回能力；旧 Vec API 仍为 v1。
2. v2 明文上限 128 MiB、artifact 上限 134250541 bytes。显式 Deferred 读事务把容量检查与 SQLite copy
   固定在同一个源 image；要求 `StepResult::Done`，不把 Busy/Locked 当作完成，不循环重试。
3. 操作者文件入口 `gateway_store::backup::create_encrypted_backup_file` 只在加密、sync 完成后发布不存在的文件。
   原有管理 preflight/restore 使用同一校验与空目标发布逻辑，没有新增下载或切换运行库的接口。
4. HTTP 上传写入配置目录的 0600 匿名文件；File 与单个 admission permit 一起经过每个阻塞写入和最终校验，
   HTTP 取消不提前释放仍在运行的磁盘任务容量。重叠操作 503；长度在写入前检查。
5. 保留 quick_check、FK、迁移历史、Backup Key 与 Master Keys 分离、hard-link 无覆盖发布。
   上传持有最初 create_new 的 descriptor，避免按路径重开造成的替换窗口。

可信 operator 应使用文件入口；失败的低层 writer 输出必须丢弃。完整 artifact、独立 Backup Key、
所有被引用 Master Key 版本及外部 bootstrap 配置分别保管。备份不含独立 admin-auth.sqlite3。

## 本轮新进程与容量验收

[脱敏汇总](issue-75-capacity-evidence-20261025/summary.json)、
[86 项检查与表摘要](issue-75-capacity-evidence-20261025/process-checks.json)、
[保全核验](issue-75-capacity-evidence-20261025/preservation.json)。

- fixed-01 **86/86 PASS**；与 baseline-06 的原始业务表摘要相同。
  58 张表的原始数据投影恢复后一致（并发增量另核对）；另对原 4 条 `gateway_request_recording` 逐单元核对，全部保留。
- 源端单 writer 提交 10 批、100 条完整 Request，记录确切 IDs；批次 1–8 的提交前后均观察到实际 SQLite copy
  尚在进行。快照保留已提交的前 10 条、40 events、10 ledger；原库保留全部 100 条。
  恢复记录是源端已提交单元格的精确前缀，不允许半批、半 lineage 或缺账本。
- 原库并发重放和恢复进程 BillingWorker 均 current，水位一致、未解决失败为 0；没有把“两个相同坏账本”当恢复成功。
- 超过小备份的 v2 artifact **98,582,061 bytes**，内部 SQLite image **98,557,952 bytes**。
  创建约 **18.232 秒**；helper OS high-water RSS **11,370,496 bytes（10.84 MiB）**。
- 通过真实受保护 HTTP 完成 preflight **29.755 秒**、restore **29.818 秒**。
  正常 restore-host OS high-water RSS **36,913,152 bytes（35.20 MiB）**。
  完整采用并查询后的新 gateway OS high-water RSS **69,435,392 bytes（66.22 MiB）**。
- 先在解密 staging 超过 1 MiB 后实际 SIGKILL restore-host，确认目标未发布；保留私有部分 staging。
  再启动新 host 明确重试，恢复到不存在的目标，停止 host 后复制到新的空 serving-state，
  由另一真实 gateway 进程采用。source、restore target、adopted DB 都保留，没有替换运行库。
- 新进程核对普通/native 停用状态、身份、模型开放范围与拒绝权限，授权读回三类加密证据；
  外属客户端读回 unknown。原账本、40,000 quota、Request/Usage、配置与路由保持。
  #72 300-row JSON/CSV/JSONL、汇总、全窗口 413，以及 #74 离线首屏恢复前后一致；查询没有上游调用。
- 受控 ENOSPC 在完整第一块之后失败并清理 staging；实际子进程 RLIMIT_FSIZE=8 MiB 分别使大快照写入、
  大恢复写入失败，未发布目标，原数据摘要不变。上传中断释放时序及重叠拒绝也实际检查。
  没有填满物理磁盘，物理 ENOSPC 与断电耐久性未验。

RSS 的 OS high-water 与独立 sampled RSS 分开记录；driver/编译器不计为 gateway RSS。
全部是本机 aarch64 macOS、Rust 1.97.1 调试构建，Cargo jobs=2，单 writer，独立 target/TMPDIR/端口/数据库。
测量同时存在本票门禁构建负载；这些数字描述本轮，不构成生产 SLA、吞吐承诺或更大规模资格。

## 门禁与源码绑定

[Full 回执](issue-75-capacity-evidence-20261025/full.md)：**44/44 PASS，exit0**，
2026-10-25 15:28:58（Asia/Shanghai）完成。Rust **1699 passed / 0 failed / 22 ignored**，
133 个结果集合；ignored 不计通过。正常工作区门禁包括格式、全 targets/features Clippy、
全工作区测试、生产 serve envelope、受控多轮/流式、377 个契约引用、源策略、秘密扫描、
Cargo deny 四类策略与 cargo audit（378 个 locked dependencies，无漏洞）。既有重复依赖 warning 保留。

存储层显式容量套件 **7/7 PASS**，含默认 Immediate 连接、调用方已有 read transaction、v1/v2、
篡改/乱序/截断/超长、ENOSPC、读中断、超过 128 MiB 源库拒绝。Full 中新增容量用例为 ignored，
其显式执行结果单独计，不与全仓通过数相加。取消后 admission 的独立阻塞任务回归也随 Full 通过。

[受测输入核验](issue-75-capacity-evidence-20261025/source-inputs.json) 证明 7159 个受控文件在 Full 前后
逐文件 hash 相同；工作区干净。最终文档后继只改变 docs，Rust/Cargo/migrations/脚本与此受测 HEAD 相同；
文档后继另做链接、契约引用、暂存秘密扫描和 whitespace 静态检查，不重复执行行为测试。

固定进程 binary SHA-256：`326e2a2ef343260b7cdf598cd53d90558d028eda2ad0bde7705d801ce7a66532`。
operator helper：`651ebe326f85959e2f4d71282d3249352659f67848bfe46bca760f42257a0041`。
它们按构建提交嵌入 revision/rust/target 并复制到固定使用的独立 `output/issue75/bin/` 路径，
没有从其他工作区借用最终测试程序。7159 个 tracked inputs 的 manifest、原始日志和私有矩阵保留在
ignored `output/issue75/`；提交内汇总记录各自 hash。继承 #65 driver 的未使用 schema35 参数只是 current 的重复 alias，
本票没有执行 schema35 二进制或用户库升级，也不据此宣称历史版本验收。

## Standards

独立初审发现 upload cancellation permit 生命周期和路线图遗漏两项 P2；另有共享 seeder 重复的 P3。
已修为 File/permit 同体移动、补齐路线图、提取同一 lineage seed。最终独立静态复核
`656c99b2` 在约定范围内 PASS；不把审查员未执行的测试称为独立运行。

## Spec

独立初审/复核发现 ENOSPC 覆盖、writer 假完成、ledger 指纹/retention 与 copy 重叠四类 P2。
已分别以受控空间错误、确切 ID/失败拒绝、真实物化/重放、copy 阶段观察及恢复前缀校验关闭。
最终独立规格/回执核验无 P1/P2 或虚假 PASS；一个 P3 是 ADR 沿用旧夹具大小，结果文档已区分测量阶段。
额外正确性审查还收紧了 size/copy 同一 read transaction、匿名 FD 以及连接默认 Immediate 的条件性失败。
[审查记录](issue-75-capacity-evidence-20261025/review.json)。

两轴分别保留结论：Standards 原 2 项 P2/1 项 P3 已处理，最终范围内无可行动发现；
Spec 原 4 类 P2 已处理，最终余下的 P3 文档数字在本次文档后继纠正。

## 历史失败、共享变化与资格边界

早期 baseline-01 的 Python 版本兼容错误、baseline-02/03 的夹具/响应处理问题、baseline-04 的路径错误、
baseline-05 的纠正前账本测量及首轮 Clippy 失败均留在 ignored output/logs，未改成 PASS。
合格对照只用 baseline-06；新能力只用 fixed-01。冻结规模没有为通过而降低。

共享变化限于 OpenAPI `BackupMaterial.description`、BC-MGMT-007、ADR-0074 的扩展链接、
ADR 索引、roadmap、traceability；schema、现有管理路由/JSON shape、组装和查询预算均未改变。
ADR-0132 已登记，0130/0131 留给并行工作集成时核对。原功能工作区和 dirty 主克隆没有被清理或覆盖。

适用证据是约 94 MiB、既定字段分布的合成 schema39。128 MiB 是实现 ceiling，未当成完整尺度资格。
未删除历史、启用 retention、修改 append-only、实现 rollup 或新统计服务。SIGKILL 私有 staging 可保留；
没有物理擦除、断电、Linux/release/OCI、实际客户端、真实 Provider、固定 Build 服务或远端 CI/部署资格。
原 #61–#67 与首版门槛保留；费用 UNKNOWN，`cost_in_usd_ticks` 仍 NOT_ADMITTED。
未合入 develop/main、push、远端评论/关闭工单、部署或发起真实收费调用。
