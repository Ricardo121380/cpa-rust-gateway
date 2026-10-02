# AC-01 授权生产修复、fresh-copy 复验与发布判定

日期：2026-10-02。**AC-01 生产单字段修复 PASS，历史凭据可读性阻断已解除；总体发布仍 BLOCKED，等待发布时重新核对、完整 pre-upgrade 备份及独立部署授权。** 18:03:51 CST 提交了唯一一次生产修复事务；18:23:12 CST 完成最终只读核对。没有部署、服务停启、配置发布、账号重新授权或本任务发起的真实 Provider 请求，C／#57／#9 保持未关闭。

## 授权、基线与固定身份

用户对[已审查的单字段生产处置申请](assets/cpar-archived-credential-diagnosis-20261002/production-repair-proposal.md)明确回复“授权”。范围为 new-vps 上 AC-01 的 ciphertext 重新封装、新一致副本的原生复验及必要有界单字段回退；部署和真实推理仍需独立授权。起始报告 HEAD 为 `a4034b129ecbadddd17eb4e835d355c875bfb3a6`，[原诊断](cpar-archived-credential-aead-diagnosis-20261002.md)和更早报告提交 `6bd5ea4a` 的失败现场与回执均保留。

签名候选仍是 source `80f5838902eb56be6dccc3ce047eb227d6c18e6c`，ARM64 ELF SHA256 `542a3c30aae7f09cac80b7cce33f3a153a87a93fe1e842cd6d8c8e355a930a87`；没有产品代码或候选身份变化，复用原签名、制品、410 项隔离回归、source80 Full45/45 和两轴评审。没有重构建、重签或重跑完整演练。

前置与最终检查都确认生产仍运行 old source `b0cf36e387b774439acc23830221389eb3ac74d0`、ELF SHA256 `0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b`、schema31、PID499070；unit/current、18180/18181 监听及六把持久／已加载／原捕获 key 匹配。凭据和 raw IDs、原／新密文、数据库及瞬时解密材料只在 new-vps 的受保护目录处理。

## 根因与精确变更

原根因结论保持 **HIGH**：archived config revision0 内的 active oauth_json credential revision2/key_version1 存放了直接父配置 AAD 下的认证封装，未与自己的 config_version_id 上下文匹配。密文结构完整，父 AAD 可认证、子 AAD InvalidTag；旧／候选原生应用都复现旧状态异常。复制时未重新封装是强支持的形成机制；具体操作者、创建命令和首次异常写入准确时刻仍未证实。本轮没有把推测提升为历史事实。

[新 pre-repair 集合](assets/cpar-archived-credential-production-repair-20261002/capture-pre-receipt.json)在 18:02:40 CST 创建。两库使用 SQLite Online Backup，覆盖已提交 WAL；同连接 data_version、六把 key、归档目录 inotify 和 runtime 在 0.155894 秒窗口内稳定，未重试、停服或暂停写入。保留两库、backups、六把 key、old ELF、unit/current、ownership/mode 元数据和私有 HMAC 基线。

[fresh 材料计划](assets/cpar-archived-credential-production-repair-20261002/plan-pre-receipt.json)再次核对目标、直接父、活动配置、最新回滚前驱全部上下文，确认与原诊断基线相同。目标仍未被 active 或 current rollback predecessor 引用；父材料认证成功，原子 AAD 仍失败。以新随机 nonce 和目标自己的 AAD 重新封装完全相同的父明文，原／新封装只写独立私有文件。

[同脚本隔离事务回归](assets/cpar-archived-credential-production-repair-20261002/transaction-rehearsal.json)通过：正向修复、精确原密文回退恢复54张表、重新应用、过期预期密文拒绝、父 revision 漂移拒绝、实际持有 writer lock 约5.005秒后拒绝，以及实际触发10.001秒事务截止点后完整回滚。不是仅检查配置声明。

[生产事务](assets/cpar-archived-credential-production-repair-20261002/production-transaction.json)使用 `BEGIN IMMEDIATE`，在同一事务内重查精确目标／直接父／版本关系、状态、revision、key_version、旧密文和活动／回滚关系；绑定参数 CAS 恰好命中1行。唯一操作写入是目标 `upstream_credentials.ciphertext`；事务内全部53张控制表逐行比较，另52张完全不变，目标表所有其他行和列不变。未操作 admin-auth 库。明文、credential/config revision、status、kind、key_version、关系、权限与原审计不变。锁等待0.000080秒，事务0.109459秒，提交前认证、integrity/FK及精确差异断言通过；没有第二次生产修复或生产回退。

## fresh post-repair 原生与迁移验证

[新 post-repair 一致集合](assets/cpar-archived-credential-production-repair-20261002/capture-post-receipt.json)在 18:04:06 CST 捕获，稳定窗口0.156636秒。两库完整性、FK与[AEAD 认证](assets/cpar-archived-credential-production-repair-20261002/plan-post-receipt.json)通过：普通41/41、归档32/32、active3/3、draft6/6、native Grok5/5；目标自己 AAD 下可读且明文等于可信直接父。

原生请求全部限 fresh 派生 state/key 目录，真实旧／签名候选 ELF，UID995/GID986，独立 mount/net/PID/IPC/UTS 与 chroot，CapEff/CapBnd0、NoNewPrivs、只有 loopback、无外部路由，宿主生产和 credentials 路径不可见。后台线程对外连接由内核隔离；导出正文只做瞬时本机比较，不保存。

| 实际管理操作 | 旧 ELF / schema31 | 签名候选 ELF / schema34 | 结论 |
| --- | --- | --- | --- |
| 配置详情／Credential summary | 200 / 200 | 200 / 200 | 原有元数据读取保持 |
| 目标与父 metadata | 200 / 200 | 200 / 200 | 原生认证读取恢复 |
| 目标与父 export | 200 / 200 | 200 / 200 | 两版目标输出均与直接父精确相同 |
| 归档配置 validate | 200 | 200 | 原503材料阻断消除 |
| 归档 fork | 409 revision conflict | 409 revision conflict | 生命周期保护保持 |
| 归档 publish | 409 lifecycle conflict | 409 lifecycle conflict | 未获得直接发布归档的权限 |

来源：[old 原生回执](assets/cpar-archived-credential-production-repair-20261002/post-old-native-v2.json)、[candidate 原生回执](assets/cpar-archived-credential-production-repair-20261002/post-candidate-native.json)，2组共18项预期结果全部通过。

另一个 pristine 派生副本已通过候选真实 store-open 完成31→34，[最终验证回执](assets/cpar-archived-credential-production-repair-20261002/post-repair-validation.json)核对相对修复后基线54张原表全部原列和行数保持、41+5份材料可认证。CLI audit 后续仍因空 capability view 报 `missing_endpoint_capability_profile`、exit1，未标作整个 CLI PASS；签名候选独立 serve 使用持久视图后，上表读取／validate 全通过。保留已完成的 pristine 迁移对象，不为修正后续操作探针重复迁移。

## 运行中的真实漂移及工具更正

不能用静态快照全表差异声称生产只有一次事务。pre 与 post 之间，另一条 active 凭据在18:03:42.914 CST 发生独立 OAuth 轮换，比本次修复事务更早；该行 ciphertext/revision、续期状态 revision/observed time 与新增 `credential_oauth_rotated` 事件的私有身份和时间精确相符。[归因回执](assets/cpar-archived-credential-production-repair-20261002/runtime-drift-reconciliation.json)证明未涉及目标／父，不被本次写入覆盖。两快照有3张控制表变化，其他51张原表（含认证表）保持；不能把这说成前后53张全部保持。

最终18:23:12 CST [只读核对](assets/cpar-archived-credential-production-repair-20261002/final-live-check.json)确认目标与父全部列、配置／active／rollback关系仍等于修复后快照；活动图、账号身份、绑定、路由、权限、认证库、unit/current/key/PID/listeners保持。该已识别的独立 active 凭据又有2次轮换，事件、actor、revision和续期时间吻合，最新材料自身 AAD 可认证。另一 active 凭据只推进了既有续期错误的 retry/failure counter，错误内容、材料、status、revision都未变，计数+1。原来123次失败早已存在；没有把本次观察解释成 Provider验收成功或候选新增故障，也没有扩大为该账号的恢复工作。公开不包含这些对象的原始标识或错误文本。

本轮操作工具出现的失败已原样保留：[更正回执](assets/cpar-archived-credential-production-repair-20261002/diagnostic-tool-corrections.json)。初版把合法运行期漂移作为单字段修复失败，后续最终检查仍有同类静态全表／单个续期行假设；修正为上述有界、精确身份／事件／revision／时间／字段核对，未允许未解释差异。另一次 native 启动前，公开探针被复制成 root600，服务 UID 无法读取；仅把相同公开代码置为0644，外层root0700不变，原失败派生目录和日志保留，之后两版实际18操作通过。所有产品源码、密钥和秘密文件权限未改变。

原v1/v2配方、私有异常日志、失败namespace、原capture／诊断副本、新 pre/post完整集合、原／新密文和CAS回退对象均保留。相关工具原失败、最小修正和独立原生验证形成回归；[源码摘要](assets/cpar-archived-credential-production-repair-20261002/operator-source-digests.json)与提交的公开配方逐字节匹配。没有把原历史 AEAD_INVALID_TAG 判为误报。

## 发布判断、回退与下一项工作

原 #57／#9 的历史账号、权限、连接、路由、Usage／账本兼容及迁移回退要求不变；原发布方案要求 `resolve and reverify archived credential readability`。本次已有真实生产单字段修复、fresh-copy AEAD和旧／候选原生验证，所以此材料门槛从 FAIL 改为 **PASS**，这是满足原门槛而非豁免。

新[发布判定](assets/cpar-archived-credential-production-repair-20261002/release-readiness.json)把旧 `PRODUCTION_ARCHIVED_CREDENTIAL_AAD_CONTEXT_MISMATCH_UNREPAIRED` 原因标为 RESOLVED；总体 **BLOCKED / PENDING_RELEASE_TIME_INVENTORY_COHERENT_PRE_UPGRADE_BACKUP_AND_SEPARATE_DEPLOYMENT_AUTHORIZATION**。生产仍old/schema31，部署NOT_RUN，真实渠道首批验收NOT_RUN，原C独立渠道缺口保持；不自动关闭C、#57或父#9。[#57本地归集](assets/cpar-archived-credential-production-repair-20261002/issue-57-evidence.md)与[结构化归集](assets/cpar-archived-credential-production-repair-20261002/issue-57-evidence.json)已更新，GitHub只读确认仍OPEN，写入0。

本次没有触发回退条件，因此生产回退NOT_RUN。必要时仍可使用已保留原封装，在核对原身份／关联保护条件后，以当前密文恰好等于本次新封装为CAS，仅回写ciphertext；同脚本已在fresh副本验证恢复54张原表。若目标已被别人改动则停止，不覆盖。回退恢复已知历史异常，材料门槛回到BLOCKED。两套 fresh完整集合供灾难恢复准备，不直接整库覆盖运行中的生产；若需整套恢复／停服则另行申请具体窗口。

下一项工程是根据[既有部署与首批验收方案](assets/cpar-execution-evidence-20261002/release-and-first-acceptance-plan.md)，重新取得发布时inventory及可信备份并交付准确切换对象／操作／回退申请。此次授权没有扩展为部署或推理预算，不在本轮申请之外执行服务操作。账号可读性已修复不能替代真实token有效性和渠道协议验收。

本轮交付静态检查、JSON／Python语法、链接、限定diff、docs门槛及受保护主机秘密边界核验见[交付回执](assets/cpar-archived-credential-production-repair-20261002/delivery-static-receipt.json)；用户原AGENTS修改及无关未跟踪内容保持。
