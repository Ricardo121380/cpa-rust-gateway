# #57 本地证据归集：固定签名制品与生产状态演练

[CPAR-48 / #57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57)仍OPEN；父[#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)及C均未关闭，没有远端issue写入。[上轮本地证据](../cpar-execution-evidence-20261002/issue-57-evidence.md)保留原时点事实；[本轮报告](../../cpar-signed-artifact-and-production-state-rehearsal-20261002.md)为后续签名/演练证据。

source80f5838902eb56be6dccc3ce047eb227d6c18e6c；ARM64 hash542a3c30aae7f09cac80b7cce33f3a153a87a93fe1e842cd6d8c8e355a930a87；[workflow36956561884](workflow-run.json)、[完整制品](download-and-file-inventory.json)、[密码学签名及严格回执](signed-verification.json)。

| #57层次 | 本轮实际结果 | 剩余 |
| --- | --- | --- |
| 精确源码release与签名 | native两平台SUCCESS，OIDC crypto+require-signature/receipt PASS | 不外推为真实Provider/生产验收 |
| ARM64目标制品 | ELF/ldd/help/startup PASS；同hash410checks PASS | 当前生产仍old，切换未执行 |
| 一致生产副本 | 两库/key稳定窗口在线副本PASS，54旧表+6key | 切换时必须fresh inventory/full backup |
| 迁移/完整恢复 | 31→34旧列/Unknown PASS；另一完整旧状态/key/old ELF/schema31启动PASS | historical归档1份AEAD失败在baseline/migrated/restored/live相同，材料全正常FAIL |
| 发布就绪 | **BLOCKED**：[就绪回执](release-readiness.json) | 归档材料诊断/恢复及重验，随后明确生产切换授权 |
| 真实新build验收 | NOT_RUN，real_provider_calls0 | 部署后actual pin/scoped capability，Krill gpt-6-sol后Build grok-4.5；各拟1pin+7basics，预算未授权 |
| 全C收口 | 未关闭、冻结标准不变 | 三协议JSON/SSE、声明扩展/受控边界、Kiro cap、Grok Web native多轮/cap及unknown relay egress |

旧runtime新字段缺失是部署后核验事项；不要求切换前取得新构建真实请求证据。[具体切换与完整回退申请](../cpar-execution-evidence-20261002/release-and-first-acceptance-plan.md)绑定source+artifact、单服务、fresh整套backup/keys和失败现场保留，当前阻断未解除不得执行。
