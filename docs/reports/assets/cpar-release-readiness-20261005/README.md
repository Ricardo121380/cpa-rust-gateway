# 2026-10-05 发布就绪复核附件

[主报告](../../cpar-release-readiness-and-controlled-deployment-preparation-20261005.md)与[发布门槛](release-readiness.json)是本轮结论。候选source80／signedhash542；本轮报告不是发布源码。当前BLOCKED，没有生产改变或推理。

- [审查覆盖表](review-coverage.md)、[精确范围manifest](review-scope-manifest.json)、[两轴结论](review-results.md)、[结构化结论](review-results.json)、[主代理补齐回执](review-static-closure.json)。
- [GitHub只读回执](github-issue-receipt.json)、issue-sources/的#9及#10–57完整公开body、[实时workflow](workflow-live.json)、[远端ref](release-ref-live.json)、[制品API](workflow-artifacts-live.json)。
- [源码／工作区输入](local-baseline.json)、[当前库存](current-environment.json)、[生产元数据](production-readonly.json)、[主机只读审计](host-readonly-audit-reconciled.json)、[20:34 CST最后只读复验](final-site-readonly.json)。first-projection-failure与首次成功回执保留，第一次不是SSH失败。
- [制品一致性](artifact-verification.json)、linux-arm64／linux-amd64-artifact-readonly.json两平台完整文件hash。密码学PASS复用同字节旧cosign，不重签。
- [恢复及门控依赖](recovery-and-admission-dependencies.json)、[恢复工具hash](recovery-tool-hashes.json)、[完整操作配方](controlled-deployment-runbook.md)、[将来批准窗口工具](approved-window-operations.py)。新工具只在repo落盘，未上传或执行维护阶段；[host-readonly-audit.py](host-readonly-audit.py)本轮以SSH stdin执行，只读无Provider。
- [首批前置](first-acceptance-preconditions.json)、[#57归集](issue-57-evidence.md)、[结构化归集](issue-57-evidence.json)、[静态交付验证](delivery-static-receipt.json)、[docs门槛7/7日志](docs-check.md)、[操作附件固定hash](operator-attachment-hashes.json)。

秘密、原始DB、私有identity、Key及解密内容留主机。正常续期及刷新差异逐字段解释，未要求运行整库字节不变。完整offline最新pre-upgrade只能在获准停服后取并恢复验证。
