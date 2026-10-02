# AC-01 有界诊断证据与本机配方

后续：[授权生产修复](../../cpar-archived-credential-production-repair-20261002.md)已经 PASS；此索引保留生产处置前的诊断证据，最新判定见[后续 release-readiness](../cpar-archived-credential-production-repair-20261002/release-readiness.json)。

[主报告](../../cpar-archived-credential-aead-diagnosis-20261002.md)、[发布判定](release-readiness.json)、[生产单字段申请](production-repair-proposal.md)、[#57归集](issue-57-evidence.md)。当前生产 BLOCKED；副本修复 PASS。产品候选仍是 source80 / signed ARM64 hash542。

## 脱敏证据

- [inspection.json](inspection.json)：私有locator对应的精确状态、格式和9个同身份上下文。parent AAD通过，child AAD失败，父子密文相同。
- [history-inventory.json](history-inventory.json)：仅capture备份树内14个SQLite候选，精确Credential／Upstream关联，无软链遍历。
- [history-and-live-check.json](history-and-live-check.json)：最早有意义的父／子副本、三份原pristine54表保持、当前生产精确父子和6key仍匹配。
- [targeted-history-attribution.json](targeted-history-attribution.json)：父创建／OAuth轮换／导出审计；更早bearer/revision0材料不能覆盖；146条同ID历史Attempt保持配置归属未知。
- [native-source-comparison.json](native-source-comparison.json)：旧新3个相关完整文件与原生打开函数相同。
- [repair-receipt.json](repair-receipt.json)：新nonce、child正确AAD、同明文；1行ciphertext变化，其余数据/key/lifecycle/revision保持，41/41+5/5通过。
- [native-cross-validation.json](native-cross-validation.json)：4次精确ELF运行，每次9个诊断管理操作；原500/503、修复200，parent始终200，archived fork/publish仍409。导出正文不保留。
- [migration-and-rollback-receipt.json](migration-and-rollback-receipt.json)：修复副本31→34、54张原表保持；单字段回退/重应用、stale预期拒写；新恢复包静态核验与独立旧启动证据区分。
- [issue-57-live.json](issue-57-live.json)、[父规格门槛摘录](parent-9-gate-excerpts.json)、[交付静态回执](delivery-static-receipt.json)。GitHub只读，0写入。

## 受保护对象

主机new-vps。原任务根为 `/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884`，root:root 0700，非软链。新增私有目录为其中 `archived-credential-diagnosis-20261002/`。

- 原 `credential-audit-failures.private.json`、capture-2、migration-entry-probe、restored-pristine均保留，原54张表旧列未变；没有纠正原FAIL回执为PASS。
- 新 `target.private.json` 是AC-01的精确私有locator；`related-contexts.private.json` 保留上下文映射；`backup-index.private.json` 只映射此次有限备份候选。
- `original-envelope.private.bin` / `repaired-envelope.private.bin` 为原／新封装；不出主机。明文只在受保护进程内用于认证、重新封装和比较。
- `repaired-pristine/` / `repaired-migration-pristine/` 保留修复前后schema31/34；`repair-cas-roundtrip/` 保留有界CAS／回退结果。
- `baseline-old/`、`baseline-candidate/`、`repaired-old/`、`repaired-candidate/` 都是独立启动／读取派生副本；私有日志只在各目录。导出正文没有写入日志或文件。
- `repaired-restore-package/` 保留修复集合的两库、backups、6key、old ELF和unit/current；包自身未再次启动。`repaired-old/` 独立运行证明相同修复状态/key旧版可读；上一轮完整旧恢复演练仍是恢复模型证据。

## 实际操作配方与复用边界

下列代码是本轮主代理实际执行的host-only操作配方，不是产品功能或生产迁移。执行前核对root0700、非软链、精确候选/旧hash、已安装cryptography和既有namespace runner。均应通过现有SSH认证在受保护主机运行；不要把数据库/key下载本地。第一次检查只在目标主机处理原locator。任何新派生目录已存在则停止，不覆盖历史结果。

1. [inspect-record.py](inspect-record.py)：从原失败locator查一个目标、同ID9个上下文和关联，独立认证；原ID只写私有manifest，公开只返回AC-01/related别名。
2. 有限备份inventory使用 `os.walk(capture-2/state/backups, followlinks=False)`，移除目录软链，检查SQLite头后仅查询同Credential／Upstream。回执和索引保留；没有对未匹配数据解密。
3. [derive-repair.py](derive-repair.py)：已有snapshot/key复制新目录，只从匹配直接父解密并fresh-nonce重新绑定child；54表精确差异检查及全46材料认证。
4. [native-read-probe.py](native-read-probe.py) + [run-native-checks.py](run-native-checks.py)：复用上一轮的 [namespace runner](../cpar-signed-state-rehearsal-20261002/private-namespace-runner.py)，4个独立副本绑定原ELF、key和private target。所有CLI/serve stdout/stderr留私有日志，公开仅HTTPcode/断言。首次probe成功后停止进程。
5. [history-and-live-check.py](history-and-live-check.py)：依赖本轮已保留的private backup index，只读比较两个有意义历史来源、精确生产父子记录、6key和old runtime；三份原状态全表HMAC核验。
6. [validate-migration-and-rollback.py](validate-migration-and-rollback.py)：基于修复pristine做一次实际原生store迁移；不吞掉随后CLI compile失败；在另一副本验证单字段CAS、原始逻辑回退和stale拒写。

原报告全部FAIL及本轮原native失败预期保留；此次不存在诊断误报修复，也没有改原AEAD探针、产品源码或签名候选。配方不应被盲目改路径用于生产；具体生产申请另见对应文件，当前没有执行许可。
