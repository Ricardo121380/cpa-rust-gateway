# AC-01 已授权生产处置证据

[主报告](../../cpar-archived-credential-production-repair-20261002.md)、[发布判定](release-readiness.json)、[#57本地归集](issue-57-evidence.md)。生产单字段修复与fresh-copy验证PASS，历史凭据材料门槛PASS；发布仍等待新inventory、完整pre-upgrade备份及独立部署授权。

公开回执只含别名AC-01、计数、时间、必要版本／代码摘要、布尔核对、HTTP状态和错误分类。原数据／密钥／密文／定位／明文不进入本仓库；全部保存在new-vps的root0700操作根：

`/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884/archived-credential-production-repair-20261002`

- [capture-pre-receipt.json](capture-pre-receipt.json)、[plan-pre-receipt.json](plan-pre-receipt.json)：新一致基线、父认证／子失败、新nonce与正确子AAD，同明文。
- [transaction-rehearsal.json](transaction-rehearsal.json)：执行同一有界CAS脚本，修复／回退／重应用、stale／parent漂移拒绝、5秒锁超时、10秒截止点回滚。
- [production-transaction.json](production-transaction.json)、[commit-confirmation.json](commit-confirmation.json)：唯一生产事务，1行ciphertext、其他控制表／字段／关联不变。
- [capture-post-receipt.json](capture-post-receipt.json)、[plan-post-receipt.json](plan-post-receipt.json)：新修复后完整集合，普通41/41与native5/5通过。
- [post-old-native-v2.json](post-old-native-v2.json)、[post-candidate-native.json](post-candidate-native.json)：2组18个实际原生操作，正确UID与kernel网络隔离。
- [runtime-drift-reconciliation.json](runtime-drift-reconciliation.json)、[final-live-check.json](final-live-check.json)：1次修复前独立active轮换、2次后续轮换、另1条既有失败重试计数，精确身份／字段／审计归因，原材料保护。
- [post-repair-validation.json](post-repair-validation.json)：54张原表迁移保持，原生及最终生产只读结果，生产未部署。
- [diagnostic-tool-corrections.json](diagnostic-tool-corrections.json)、[operator-source-digests.json](operator-source-digests.json)：工具失败保留与更正，公开配方与主机字节相同。
- [issue-57-live.json](issue-57-live.json)、[issue-57-evidence.json](issue-57-evidence.json)、[public-boundary-check.json](public-boundary-check.json)、[delivery-static-receipt.json](delivery-static-receipt.json)。

受保护根的 `capture-pre/` 和 `capture-post/` 均包含两库、backups、六把key、old ELF、unit/current、ownership/mode和私有HMAC基线；`original-envelope.private.bin`、`repaired-envelope.private.bin`、`target.private.json`、apply intent及运行期精确差异留主机。`transaction-rehearsal/`、parent-drift／deadline副本、`post-migration-pristine/`、old/candidate独立native副本全部保留。原capture-2、diagnosis及失败密文没有覆盖。

配方为本次实际主机操作的审计来源，依赖保留的私有基线和namespace runner；不作为通用管理命令或可重复生产迁移。`production-transaction.py` 必须显式传 `apply-authorized`／`rollback-authorized`；已有提交回执则拒绝重复写入。任何新操作仍需匹配当时的精确授权及私有前置。`*.v1.py`／`*.v2.py` 是故意保留的失败版本，不应执行。最新 `validate-post-repair.py` 保留了原v3最终检查假设，实际成功的有界最终收尾使用 `final-live-check.py`，所有阶段结果按回执读取，不把部分脚本exit2标成整体PASS。

明文只在受保护进程中认证／重新封装／导出比较，无正文落盘或外发；后台Provider阻断限隔离副本。本任务Provider请求0，不声称现有运行中服务从未自行续期。
