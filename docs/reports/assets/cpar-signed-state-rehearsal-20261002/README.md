# 固定签名候选与生产私有副本演练证据

[主报告](../../cpar-signed-artifact-and-production-state-rehearsal-20261002.md)、[发布就绪](release-readiness.json)、[具体方案](../cpar-execution-evidence-20261002/release-and-first-acceptance-plan.md)。source80f5838902eb56be6dccc3ce047eb227d6c18e6c；最终ARM64 binary542a3c30aae7f09cac80b7cce33f3a153a87a93fe1e842cd6d8c8e355a930a87。**BLOCKED于1条preexisting archived credential AEAD_INVALID_TAG。**

## 证据索引与口径

- [先行状态](baseline-check.json)、[issue57只读状态](issue-57.json)、[最终ref](final-remote-ref.json)、[branch runs](final-branch-workflows.json)。源码/报告HEAD分离；只有1次branch push、1次release dispatch。
- [CI](workflow-run.json)、[checkout/toolchain原文](checkout-and-toolchain-excerpts.log)、[GitHub artifacts](github-artifacts.json)、[所有9文件hash](download-and-file-inventory.json)、[实际cosign与严格receipt](signed-verification.json)。linux-arm64与linux-amd64保存manifest/SBOM/metadata/identity/signature/bundle/receipt；binary/OCI/ZIP留output，不是省略制品。
- [目标兼容](target-compatibility.json)、[同hash410回归](signed-binary-regression.json)、[完整410回执](native-http/receipt.json)、[source脚本和operator hash](target-inputs.json)。所有Provider fixture都是loopback合成，真实调用0。
- [前核验](production-before.json)、[Online Backup稳定窗口](backup-receipt.json)、[SQLite守卫探针](sqlite-consistency-guard-probe.json)、[31→34及原列HMAC](migration-receipt.json)、[AEAD可读性](crypto-before.json)、[old/new AAD源码比较](legacy-crypto-source-comparison.json)。HMAC随机钥匙仅在host，不公开原行/原值/低熵逐行hash。
- [候选启动](candidate-startup-receipt.json)、[完整恢复](restore-receipt.json)、[旧启动](old-restored-startup-receipt.json)、[archive/属主/模式/恢复依赖](complete-private-restore-audit.json)、[汇总](rehearsal-receipt.json)。54旧表原列保持；schema_migrations对原31行校验，新32–34另外断言；legacy新字段只记录Unknown/default，不能发明执行或更新事实。
- [生产后核验](production-after.json)、[最终私有审计](final-private-audit.json)、[活跃生产逻辑漂移](production-logical-comparison.json)、[漂移与失败密文仍存在](production-state-drift-assessment.json)。该活跃库不冻结；future cutover需fresh full backup。恢复步骤PASS与历史AEAD失败分别保留。
- [#57本地归集](issue-57-evidence.md)、[状态](issue-57-evidence.json)、[交付静态](delivery-static-receipt.json)、[docs gate](delivery-docs-check.md)。生产未部署、C/#57/#9未关闭。

## 私有主机对象与可重复步骤

主机task根 `/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884` 为root0700，只有root及被明确挂载到独立namespace的本机受保护进程能使用副本/key。所有生产DB、6key、HMACkey、原始row baseline、私有CLI/gateway日志、failed archived locator和unit原文仅在host。

`artifact-542a3c30aae7/`是完整9文件已验证集合；`artifact/`是超时partial，不能采用。`capture-2/`是一致升级前集合；`migration-entry-probe/`是真实31→34后的pristine；`restored-pristine/`是另一目录的完整升级前恢复集合。各startup目录为其派生copy，worker更新只在派生copy；pristine与失败现场保留。

本目录Python是本次实际执行的operator配方，不是产品功能或生产unit。主代理自己执行和验证；使用它们前须重新核对task root不是软链、root0700、精确old/source/hash及当前生产状态。新操作不可把旧已完成对象盲目覆盖。当前BLOCKED receipt不能通过重复执行自动晋级。

1. [remote-readonly.py](remote-readonly.py)：metadata/计数、实际PID/exe/版本、原监听/身份/必要路径；不输出凭据值。
2. [online-private-copy.py](online-private-copy.py)：两库Online Backup+跨库/key/archive稳定窗口，失败留现场；生成private oldrow/HMACkey/key/old ELF/unit/current依赖。有效成功副本复用，不重复触发生产副本捕获。
3. [private-crypto-audit.py](private-crypto-audit.py)：准确source AAD、真实AEAD和native格式解析，aggregate成功/失败；HChaCha公开vector自校验。plain/key只供本机进程，输出不含它们、IDs或key摘要。
4. [private-namespace-runner.py](private-namespace-runner.py)：root先unshare mount/net/PID/IPC/UTS、mount rprivate、仅绑定task文件与public runtimes、chroot后setpriv为UID995/GID986，CapEff/CapBnd0、NoNewPrivs；只有namespace loopback，无external route。
5. [target-rehearsal.py](target-rehearsal.py)：严格最终artifact/receipt/hash/ELF/libc检查，复用已成功410regression；真实store初始化迁移并检查alloldcolumns/Unknown；另一个完整prestate/key/old ELF恢复，派生copy启动和GET，历史crypto failure阻断总判定。
6. [isolated-startup-probe.py](isolated-startup-probe.py)：实际serve、healthz、8类GET、管理header/投影、source/hash/schema/uid/net/caps；不pin、不真实推理，成功或失败均终止process并留private日志。
7. [complete-private-restore-audit.py](complete-private-restore-audit.py)：只读原archive与private copy bytes/软链/metadata核验；仅修正private copies中copytree引入的owner差异，补齐恢复依赖。没有生产chmod/chown。
8. [recheck-isolated-startup.py](recheck-isolated-startup.py)：仅重验失败startup检查，已成功的migration/restore/410对象复用；新copy避免覆盖失败现场。
9. [final-private-audit.py](final-private-audit.py)：old/current/unit/drop-in/key/原监听不变、taskprocess退出；确认archived失败为AEAD_INVALID_TAG，私有定位留host不导出。生产逻辑漂移另行只读比较记录。

## 已保留的失败与纠正

| 失败 | 回执 / 纠正 |
| --- | --- |
| staging父目录尚不存在 | [准备失败](backup-preparation-failure.json)；只在授权staging创建独立受保护目录 |
| 历史archive软链被copytree跟随 | [软链准备失败](backup-symlink-preparation-failure.json)；copytree symlinks=True，不遍历其生产目标 |
| 大包source .git传输超时/OCI partial | [传输超时](target-transfer-timeout.json)；只传所需exact-source scripts，完整signed artifact另传unique目录后逐项hash验证 |
| 首轮AEAD遇到历史异常 | [首轮失败](first-crypto-probe-failure.json)；保留FAIL并聚合定位，无exception豁免 |
| native Build被误当UTF8/JSON | [格式纠正](native-format-probe-correction.json)；按oauth persisted binary source解析，native5/5通过 |
| 沙箱缺公共openssl.cnf | [fixture准备失败](synthetic-openssl-preparation-failure.json)；只补public config只读挂载，410回归成功后复用 |
| namespace UID映射导致无法开DB | [身份失败](migration-sandbox-identity-failure.json)；改为private namespaces+chroot后真实host UID，失败schema31副本保留 |
| 猜测的bwrap --share-user不支持 | [unsupported option](unsupported-sandbox-option-failure.json)；读实际bwrap帮助并停止使用该选项 |
| 预先drop host UID后无法配置loopback | [loopback失败](sandbox-loopback-preparation-failure.json)；root仅在private namespace完成准备，再drop全部caps和service UID |
| CLI audit迁移后无capability view | [迁移回执](migration-receipt.json)记录exit1；empty CLI view限制不当作整体成功，另外实际serve/GET成功 |
| 管理GET缺X-Config-Version | [header准备失败](startup-probe-header-preparation-failure.json)、[first candidate](first-candidate-startup-receipt.json)、[first old](first-old-restored-startup-receipt.json)；新派生copy核对active后带正确header |
| 管理rev-N被直接同整数比较 | [投影准备失败](startup-probe-management-revision-preparation-failure.json)、[第二次完整回执](second-rehearsal-receipt.json)；只验证已文档化管理投影，不用于execution identity |

以上纠正均是操作配方，candidate/workflow未改；最新new/old启动PASS，历史archived AEAD FAIL仍阻断。两份pristine状态和全部private失败现场保留，不能用migration down或删除历史凑通过。
