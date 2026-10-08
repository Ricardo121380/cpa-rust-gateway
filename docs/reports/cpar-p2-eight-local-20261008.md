# CPAR 八项 release-readiness P2 本地批次 · 2026-10-08

本批次 B-05、A-02–A-08 八项全部 **RESOLVED_LOCAL**：实现、必要验收与 Standards/Spec 独立评审均完成。最终运行源码为 `a98bfd5e01ea51d3894cce4afcbc0324e9142702`，基线 `3e5fe6f5bea803b09f95726c95e6f80b531e11b1`，分支 `codex/prism-v4-delivery`。原始 13 项 P2 至此已全部本地解决；项目、#57 和发布继续保持各自边界，发布仍 **BLOCKED**。

[任务卡](assets/cpar-p2-eight-20261008/task-card.md)在实质探索前记录 L 等级、三个实施组、工作预算与权限；[逐项证据索引](assets/cpar-p2-eight-20261008/evidence-index.json)给出每项多个 RED/GREEN、提交与定向回执；[源码 manifest](assets/cpar-p2-eight-20261008/source-manifest.json)固定全部 45 个变更文件的 Git blob 与 SHA256。发现编号并非 CPAR 工单编号。

| 发现 | 本地状态 | 实现提交 | 目标 RED | 最终源码操作证据 | 修改 |
|---|---|---|---|---|---|
| B-05 | RESOLVED_LOCAL | `63bfc0e0` | [b05-process-red-receipt.json](assets/cpar-p2-eight-20261008/b05-process-red-receipt.json) | [accepted-b05-process-receipt.json](assets/cpar-p2-eight-20261008/accepted-b05-process-receipt.json), [accepted-b05-process/receipt.json](assets/cpar-p2-eight-20261008/accepted-b05-process/receipt.json) | 外部版本化 master-key ring 与 active version；管理侧和运行侧共用加载语义，旧单密钥保持 v1。 |
| A-02 | RESOLVED_LOCAL | `f553b9d8`, `7c6e68d7` | [a02-oauth-red-target-receipt.json](assets/cpar-p2-eight-20261008/a02-oauth-red-target-receipt.json) | [accepted-a02-oauth-receipt.json](assets/cpar-p2-eight-20261008/accepted-a02-oauth-receipt.json), [accepted-a02-import-receipt.json](assets/cpar-p2-eight-20261008/accepted-a02-import-receipt.json), [accepted-a02-api-receipt.json](assets/cpar-p2-eight-20261008/accepted-a02-api-receipt.json), [accepted-a02-import-config-receipt.json](assets/cpar-p2-eight-20261008/accepted-a02-import-config-receipt.json), [accepted-a02-api-config-receipt.json](assets/cpar-p2-eight-20261008/accepted-a02-api-config-receipt.json) | 保存、后续读取、连接与应用分别结算；保留 completed 回执的精确 saved ID 和读取错误。 |
| A-03 | RESOLVED_LOCAL | `f553b9d8` | [a03-browser-red-actual-receipt.json](assets/cpar-p2-eight-20261008/a03-browser-red-actual-receipt.json) | [accepted-a03-close-receipt.json](assets/cpar-p2-eight-20261008/accepted-a03-close-receipt.json) | 发布回执立即更新 active 并结清同 ID/revision pending，保持结果窗口所属代次。 |
| A-04 | RESOLVED_LOCAL | `f553b9d8` | [a04-selected-red-target-receipt.json](assets/cpar-p2-eight-20261008/a04-selected-red-target-receipt.json) | [accepted-a04-selected-empty-receipt.json](assets/cpar-p2-eight-20261008/accepted-a04-selected-empty-receipt.json), [accepted-a04-tracked-empty-receipt.json](assets/cpar-p2-eight-20261008/accepted-a04-tracked-empty-receipt.json), [accepted-a04-nonempty-receipt.json](assets/cpar-p2-eight-20261008/accepted-a04-nonempty-receipt.json) | 依据与 active 的完整差异确认空草稿；非空清单、所有权和发布 CAS 保留。 |
| A-05 | RESOLVED_LOCAL | `f553b9d8` | [a05-kiro-red-target-receipt.json](assets/cpar-p2-eight-20261008/a05-kiro-red-target-receipt.json) | [accepted-a05-kiro-receipt.json](assets/cpar-p2-eight-20261008/accepted-a05-kiro-receipt.json), [accepted-a05-kimi-receipt.json](assets/cpar-p2-eight-20261008/accepted-a05-kimi-receipt.json) | 管理员会话内稳定保留原 session/task 非秘密标识，关闭及跨路由后可回读同一回执。 |
| A-06 | RESOLVED_LOCAL | `96c0a96b` | [a06-materials-red-target-receipt.json](assets/cpar-p2-eight-20261008/a06-materials-red-target-receipt.json) | [accepted-a06-materials-receipt.json](assets/cpar-p2-eight-20261008/accepted-a06-materials-receipt.json) | 按真实 authentication 材料分类提示替换 API Key 或重新授权，保持停用和服务阻断优先级。 |
| A-07 | RESOLVED_LOCAL | `96c0a96b` | [a07-pending-red-target-receipt.json](assets/cpar-p2-eight-20261008/a07-pending-red-target-receipt.json) | [accepted-a07-pending-receipt.json](assets/cpar-p2-eight-20261008/accepted-a07-pending-receipt.json), [accepted-a07-error-receipt.json](assets/cpar-p2-eight-20261008/accepted-a07-error-receipt.json) | 即时与历史回执使用同一 current guard，refetch pending/error 将缓存 true 标为历史。 |
| A-08 | RESOLVED_LOCAL | `96c0a96b`, `a98bfd5e` | [a08-stale-red-target-receipt.json](assets/cpar-p2-eight-20261008/a08-stale-red-target-receipt.json) | [accepted-a08-stale-receipt.json](assets/cpar-p2-eight-20261008/accepted-a08-stale-receipt.json), [accepted-a08-current-receipt.json](assets/cpar-p2-eight-20261008/accepted-a08-current-receipt.json) | 运行行投影明确 credential revision 和普通配置归属；旧行不覆盖新授权，保留观察时间、快照及连接范围。 |

最终源码前端 67 文件、480 项单元/组件测试通过，适用类型检查、构建与权威契约同步检查通过。A 组最终 16 个 EgoLite 场景全部操作实际桌面页面，以合成 fixture/API 响应触发真实状态链路，保存 ID、原 session/task、交换/import/Pin 次数均有断言。A-07 每场景只有明确点击产生的 1 次合成 Pin；refetch 没有额外推理。真实 Provider 请求为 0。

B-05 使用测试生成密钥、临时 SQLite 与 loopback TLS 上游，实际 serve 进程停止/重启，以公开 Responses GET/DELETE、stored continuation 与 compact 验证所有权和密钥版本；管理新增 credential 使用 active key，缺失旧 key 的两种启动/读取边界分别测试。未使用手工 Store 重开替代进程验收。

初期验收脚本的 CLI 参数、缺失必需管理头、portal DOM 选择器、HMR 模块 URL 及 fixture 同协议 endpoint 冲突属于脚本/fixture失败，均不计行为 RED。行为 RED 仅采纳命中用户问题且完成原操作的目标回执；所有运行保留其真实 HEAD、dirty 源码散列与退出状态。A-02 补充配置读失败有实际失败计数断言，非仅设置未触发的模拟开关。

[保留检查](assets/cpar-p2-eight-20261008/preservation-checkpoint.json)对初始 4,324 个文件逐个比较 SHA256，包含用户 AGENTS.md 与全部初始无关未跟踪内容，无更改或缺失。跨界前端文件分别在同一提交的 cross-boundary log 和 trailer 中记录。后端权威 OpenAPI 单向同步；生成客户端重新生成但操作描述 blob 无变化。

S-01、E-01–E-05、O-01 保持独立未解决；#9、#57 及原始工单的最新只读快照均仍 OPEN，本轮没有工单写入或关闭。ADR-0103 的 B-04 新 Usage 身份兼容限制继续成立：旧 binary 不识别它们，后续回滚仍需兼容代码或授权的受控状态恢复与历史保全。Full 的本地 PASS 不证明旧 binary 直接回退安全。

[最终 Full](assets/cpar-p2-eight-20261008/accepted-full-steps.md)：45/45 PASS，361.617 秒，1,488 项 Rust 测试通过；checkout 前后均干净。相同源码和 binary 上 [B-05](assets/cpar-p2-eight-20261008/accepted-b05-process/receipt.json) 40/40 PASS，[跨组 HTTP](assets/cpar-p2-eight-20261008/accepted-b-crossgroup/receipt.json) 410/410 PASS。源绑定门禁摘要见 [batch-runtime-summary.json](assets/cpar-p2-eight-20261008/batch-runtime-summary.json)。

首次 Spec 在 `96c0a96b` 发现 A-08 的正常 OAuth 刷新缺口：材料已从 revision 1 装载为 2，但响应仍使用不可变 descriptor 的 revision 1。主代理补充实际认证管理 HTTP [RED](assets/cpar-p2-eight-20261008/a08-refresh-lineage-red-actual-receipt.json)（执行 1 项、响应 1/预期 2、exit 101），从同份运行池诊断快照投影 revision 后转为 [GREEN](assets/cpar-p2-eight-20261008/a08-refresh-lineage-green-receipt.json)。该 GREEN 保留原 HEAD/dirty 身份，修改文件 SHA256 与最终提交一致；最终干净 Full 再次执行该回归通过。回归覆盖四类 OAuth 刷新后的健康与当前未授权观察，使用真实管理 HTTP 处理链和合成健康标记，未调用真实 Provider。invalid_grant 未装载新运行材料时保持旧 runtime revision，当前管理 Unauthorized 事实仍可见。

0 项的 `a08-refresh-lineage-red-receipt.json` 是筛选名错误，不是行为 RED。首次评审与旧 `final-*` 回执均保留各自源码；本报告最终验收仅引用重新执行的 `accepted-*` 回执，没有把旧运行改标为新源码。

## Standards

**PASS，确定标准违反 0，smell 0。** 首次完整读取 45 文件全部 hunk/必要上下文，最终两处后端增量完整复核。其余 43 文件的 Git blob 与 SHA256 未变，承接首次完整读取；合并覆盖 **45/45**，未读变更文件 **0**。

## Spec

**PASS，八项分别 PASS；最终 missing/partial、scope creep、implementation error 各 0。** 首次 A-08 FAIL 保留，实际复现、修复、补验后已复核 CLOSED。完整覆盖同样由 43 个未变文件加两处完整增量组成，未把七项 PASS 直接扩为八项。

两轴的发现、覆盖来源、限界及最终 blob 见 [评审报告](assets/cpar-p2-eight-20261008/review-final.md)、[45 项逐文件覆盖](assets/cpar-p2-eight-20261008/review-final.json)和[增量证明](assets/cpar-p2-eight-20261008/review-delta-proof.json)。探子只核验已有证据，最终测试执行归主代理。23 份最终回执的源码、前后 clean、exit 与原始日志 SHA256 校验见 [验证索引](assets/cpar-p2-eight-20261008/accepted-receipts-validation.json)。

| 验收层次 | 结果 | 依据与适用范围 |
|---|---|---|
| 本地最终 Full | PASS 45/45 | 干净隔离 checkout，361.617 秒，Rust 1,488 PASS；包含原 A-01/B-01–B-04、所有权、失败用量、合法重试等回归 |
| 前端类型、测试、构建与契约 | PASS | 67 文件/480 测试；type/check 及 Full 的 deterministic SPA 构建、一致性检查 |
| EgoLite 桌面主流程 | PASS 16/16 | 实际页面操作，本地合成 fixture 和可控响应 |
| B-05 隔离实际进程重启 | PASS 40/40 | 测试密钥、临时数据库、公开请求路径 |
| 跨组实际网关 HTTP | PASS 410/410 | 与 B-05 相同最终源码和 binary，合成 loopback 上游 |
| Standards / Spec | PASS / PASS | 各 45/45 合并覆盖，首次完整读取与最终增量证明明确 |
| 真实 Provider/真实账号 | NOT_RUN | 本轮授权范围为本地合成验收，请求数 0 |
| 远端 CI | NOT_RUN | 没有 push 或远端运行 |
| 部署、生产验收 | NOT_RUN | 没有生产状态操作 |
| 真实网关 `/admin-ui/` 的全前端完成条件 | NOT_RUN | synthetic EgoLite 不证明开发计划第 447 行的该条件 |
| #57 全范围收口与发布 | BLOCKED / 未完成 | 独立 S-01、E-01–E-05、O-01 及全部规格/真实环境门槛仍需分别核对 |

最终测试 binary SHA256 为 `96345a80c871e47dda30efe7db8a3077a0d90c28bca9a4615f520a0ebb2e5bf7`。这是本地测试 binary 的证据，不是签名发布产物或生产部署证据。

[本地 #57 补充草稿](cpar-p2-eight-issue57-supplement-20261008.md)已经准备，仅供审查。文档收口只改变报告、任务卡和证据产物；相对最终运行源码的路径与 blob 核对见 [文档增量证明](assets/cpar-p2-eight-20261008/docs-closeout-delta.json)，文档检查结果另见 [docs 检查表](assets/cpar-p2-eight-20261008/docs-closeout-steps.md)。本轮没有未完成的八项 P2 实施/验收；范围外独立门槛是后续工作的具体边界。
