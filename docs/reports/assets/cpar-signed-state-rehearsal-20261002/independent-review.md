# 脱敏证据与操作配方的独立核验

两名干净上下文只读探子各一轮完成；没有读取生产原始数据库、keys或私有日志，没有执行生产写入，最终验收和发布判断由主代理负责。结论为限定范围PASS，发布状态仍BLOCKED。它们不替代固定source80此前的Standards/Spec源码评审，不代表C或真实Provider完成。

## 公开制品链：PASS

`/root/verify_public_artifact_chain` 核验两平台实际checkout/ref/run/Rust；独立解析bundle叶证书source/ref/issuer/SAN/run OID；只读重算output两平台全部18文件SHA256/size及公开归档同名7文件，manifest5payload/receipt8文件全部匹配。独立OpenSSL叶公钥SHA256验签exit0、Verified OK，detached/bundle/Rekor签名及manifest payload hash互相一致；报告没有把unsigned hashbd2e46ed与signed hash542a3c30混用。

关键出处：checkout-and-toolchain-excerpts.log两段实际git log；signed-verification.json及两份artifact-manifest.sigstore.json；download-and-file-inventory.json；final-remote-ref.json；release-readiness.json。该探子未刷新远端/ZIP或重复完整Cosign/Fulcio/Rekor信任链；主代理实际cosign信任链检查仍以[准确命令/版本回执](cosign-command-receipt.json)和实际日志为依据。

## 一致性、隔离和恢复：PASS（证据边界）

`/root/verify_isolation_and_restore_evidence` 核验Online Backup/read transaction、同连接data_version/key/inotify稳定窗口实际回执；私有namespace、rprivate mount、task-only绑定、chroot真实UID995/GID986/zero caps/NoNewPrivs；新旧独立监听、无external route、原状态/key不可见和process退出。

独立逐项比较backup/migration/restore：53个控制旧表+1认证表的完整集合、count/旧列HMAC全一致；schema31→34与原31条schema_migrations口径准确；新execution/native receipt为空、Usage保持Unknown；ordinary40/41/native5/5及archived1失败在迁移/恢复前后一致。另一个完整prestate/key/old ELF/schema31及8类GET启动有实际receipt，archive bytes/ownership/modes和2条symlink审计明确。生产3表漂移保留，未宣称活跃生产整体冻结。

发布方案先关闭AEAD阻断、未来fresh完整backup/old restore、失败保存新state/ledger再整套恢复，旧runtime缺新字段归入部署后核验。探子仅核验配方与脱敏回执，未SSH或重跑演练；主代理已实际执行的receipt承担最终运行证据。
