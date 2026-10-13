# CPAR-38 / #47：受控 Chat JSON 诊断批次在正式 epoch 前停止

本批结论为 **BLOCKED / STOPPED_BEFORE_FORMAL_EPOCH**。第一次只读观察遇到本批资格驱动的 API 版本契约错配，按新契约立即停止。已完成证据定位、脱敏离线 RED→GREEN 修复和只读退出；没有上传、创建远端 epoch、启动实例或发送 Pin/Chat。旧502根因仍为 UNKNOWN。

## 基线和复用

- 实际初始 HEAD：`810d210f7217c479ebc1376150fb3a45609b7759`，分支 `codex/prism-v4-delivery`。历史检查点未用于回退。
- 产品 source：`bf822b14a62c21cf582a7539e3635f69bd8b7677`；固定资格驱动来源：`c23d2c356ec0253d93e41ca94aac821ad167fcd6`。当前产品与构建输入逐项匹配；未重新安装依赖或构建 binary。
- binary SHA256：`cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e`；manifest：`655c037df68c5657b250105bbb07bed665210a7f6b20ea8d8b88aab488157a56`。manifest两行、metadata及大小已核验。
- Task42/42、Phase45/45与完整历史两轴审查按来源和压缩/原始日志摘要复用，未重跑未变化的全套门槛。已有具体拒绝原因、Request-ID及已解码 Usage 产品修复未重复实施。
- 本批新契约独立于旧契约。旧58、10月11日未用2、此前10月13日未用2均保持终止。

证据见 [复用核验](assets/cpar-build-chat-json-diagnostic-20261013/reuse.json)、[新契约](assets/cpar-build-chat-json-diagnostic-20261013/batch-contract.md)、[授权范围](assets/cpar-build-chat-json-diagnostic-20261013/user-authorization.json)。

## 本批准备和修复

实际受保护配方经一次只读取回，完整组装为36项argv；唯一临时unit/binary/自有路径，唯一Environment属性保留LD_PRELOAD guard、审计出口，仅追加三个诊断选择字段。copy-runtime-options=[]、后台禁用、安全属性、代理环境清除均保持。此证据只证明离线组装。

准备审查先后发现并修复四处本批驱动缺口：观察消费未经校验的公共副本、本地状态缺少互斥、启动管理GET对认证错误重试、缺少实际监听socket/PID归属检查。两组实际控制边界测试均保留RED2→GREEN2，失败审查与PARTIAL不覆盖。最终准备两轴完整52/52 PASS；这没有授予账号、进程或Pin资格。

资格观察失败导出只保留闭合状态、源码行号、结构、摘要和已取得的新鲜运行基线。原生响应、凭据和会话内容未进入Git、GitHub或模型。本批没有取得新的原生诊断样本。准备日志中33个文件权限在首次观察前收紧到0600，目录0700；历史权限未修改。

见 [完整argv](assets/cpar-build-chat-json-diagnostic-20261013/assembled-launch.json)、[初审](assets/cpar-build-chat-json-diagnostic-20261013/review-initial.json)、[第二轮](assets/cpar-build-chat-json-diagnostic-20261013/review-round2.json)、[准备最终审查](assets/cpar-build-chat-json-diagnostic-20261013/review-pre-execution.json)。

## 第一次观察和停止

只读观察于2026-10-13 **11:56:24（Asia/Shanghai，03:56:24 UTC）**返回。只用1/7次，未进入WAITING_FOR_QUALIFICATION，也未消耗60分钟等待窗口。观察包含2次管理GET，无Provider、刷新、重新授权或远端写入。

| 观察事实 | 结果 |
|---|---|
| 批准账号的非空同身份、key版本与授权连续性 | PASS |
| provider/auth/worker状态 | build / active / 无worker claim |
| native revision及连续范围 |144，floor77/current145；仅正常刷新字段变化 |
| source pool | available，active_leases=0 |
| 材料剩余期 |16998.23秒，约4小时43分18秒 |
| 驱动读取的source pool摘要revision |null，资格断言拒绝 |
| 只读观察整体 |QUALIFICATION_REJECTED；本地永久stopped=true |

本次不是“正常同身份但仅有效期不足”，因此没有再次观察、等待续期或恢复。准备门槛仍为 `expires_at_ms > now_ms + 3600000`。正式epoch从未创建，不伪称它已有远端停止预算。

见 [原观察回执](assets/cpar-build-chat-json-diagnostic-20261013/observation-1.json)、[停止结论](assets/cpar-build-chat-json-diagnostic-20261013/stopped-batch.json)。

## 已确定的驱动缺陷及离线修复

新鲜生产target为 `b0cf36e387b774439acc23830221389eb3ac74d0`。该版本真实handler直接序列化的 `ProviderAccountPoolItemResponse` 与OpenAPI没有 `credential_revision`；候选版本定义该字段为可缺省/null的Option。`.get()`把旧API缺失字段投影为null。本批资格驱动错误地要求这个摘要字段等于连续材料revision。独立探查和主代理对实际DTO、handler、converter、snapshot/lease符号的定点读取建立了此缺陷。

最小修复：仅在source摘要提供非null revision时核对一致性；同身份、key版本、连续floor/current、允许刷新字段、正常池、active_leases=0和严格60分钟准备门槛均保持。正式验收实例的实际pool revision与fresh Pin真实lease检查保持强制，不能从材料revision推算lease。

脱敏离线回归机械还原本次classifier调用前的输入：只还原失败handler改写的status并去除拒绝专用字段，其余账号/池/身份/连续性/时间/历史计数全部保留；原回执不修改。RED含null断言和缺字段KeyError，GREEN9/9，连同此前未变化控制器2和启动2的证据为13项。

修复source `5e8768c485dda02b8b84672a7603a97d49f85a1f`只作为后续离线准备参考。本批原拒绝包freeze `c30d2688d0b182f4a97d3a30216e0b7421ff8c0a6a360d6c71eeb145b8ec4545`、tar和停止state保持；没有重冻结包或进行修复后真实复测。后续独立批次必须从修复source派生新的冻结对象，不能沿用此停止包。

见 [接口契约证据](assets/cpar-build-chat-json-diagnostic-20261013/pool-contract-evidence.md)、[脱敏回归](assets/cpar-build-chat-json-diagnostic-20261013/legacy-pool-checks.json)。旧502是否为软件缺陷或严格语义契约允许的拒绝仍UNKNOWN；无NOT_REPRODUCED结果，无旧原生响应重放，无产品保真规则例外。

## 真实执行、Usage和计数

| 项目 | 本批结果 |
|---|---|
| 远端上传/正式epoch |NOT_CREATED |
| 临时实例、PID/start/exe/argv/env/maps/端口归属及65秒静默实证 |NOT_STARTED / NOT_RUN |
| 新鲜Pin |NOT_RUN |
| 原合成Chat JSON |NOT_RUN |
| 预定协议与上限 |POST /v1/chat/completions，grok-4.5，stream=false，max_completion_tokens=4096；未改 |
| 预定原合成请求摘要 |245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062；本批未复制/发送新的请求字节 |
| 本批Request-ID / Request / Attempt / Finished / validated Usage / ledger / native capture |NOT_CREATED |
| 本批新推理意图 |0/2；Pin0，Chat0；未用2随本批停止终止 |
| 本批网关/Pin确认发送 |0 |
| 历史及累计意图/确认发送 |35 /34，未增加 |
| Provider独立接收数、实际费用 |UNKNOWN，未推算成0 |

已有Usage/账本修复及历史来源保留。没有本批请求、Attempt或账本来源事实可供创建，旧502的缺失原生材料仍缺失。

## 退出、工作区与GitHub

2026-10-13 **11:57:38（03:57:38 UTC）**只读退出PASS：远端目录/上传/审计文件均不存在；unit MainPID0、not-found；临时端口无监听、无竞争进程，5秒静默；分别NOT_CREATED / NOT_STARTED，未执行伪造停机。

新鲜生产PID499070、start、unit/binary/current/Caddy摘要与active config前后一致；active config为本次实际观察的kimi-binding…rev-1，没有恢复旧报告中的配置。334个历史冻结文件、14个旧上传载荷核验，保护根363文件保持，私有0700/0600及3个历史0644 bootstrap回执保持；远端写入0、生产写入0，退出管理GET1，无Provider。

工作区初始48307普通文件的byte/mode与10条symlink target核验无变化，AGENTS摘要保持。初始另47个untracked目录条目没有递归字节基线，这部分不记为全字节验证。所有提交只包含本批报告/资产，不修改web/prism、Rust产品、生产或旧材料；不push、CI、发布或部署。

见 [退出证据](assets/cpar-build-chat-json-diagnostic-20261013/no-launch-exit.json)、[工作区核验](assets/cpar-build-chat-json-diagnostic-20261013/dirty-preservation.json)。

最终交付两轴审查均为PASS：Standards与本批Spec分别完整覆盖73/73，固定范围为 `3d44caf8265343338778ee46f69672a40be5fd54` → `d39fc9aec4744d5760afff23c65ebf3c621903e7`；23个变化对象及hunks、3份gzip全文完成，原52对象中50个摘要核对后复用、2个变化对象完整重读。结论与逐项摘要见 [最终审查](assets/cpar-build-chat-json-diagnostic-20261013/review-delivery.md) 和 [审查清单](assets/cpar-build-chat-json-diagnostic-20261013/review-delivery.json)。此结论只覆盖本地改动及停止交付，不构成真实启动或Chat通过。

GitHub写前新鲜核验#47/#57/#9正文、progress和依赖与初始快照一致；已于2026-10-13 04:25 UTC逐条追加既有progress，并以9次独立GET读回progress、正文与依赖。三条完整正文等于冻结的历史前缀加已审查追加稿，marker各一次；正文/AC、标题、标签、分配、milestone、锁定、依赖与OPEN状态保持，依赖仍8/23/0，没有新建评论或关闭工单。见 [同步及独立读回](assets/cpar-build-chat-json-diagnostic-20261013/github-sync.json)；既有progress：[\#47](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242)、[\#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、[\#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490)。

本批只建立本地修复和一次只读资格拒绝/退出证据。单次真实Chat诊断、六组合矩阵、扩展、CI、发布、部署和生产验收均NOT_RUN。诊断与保真规则缺口尚未取得样本消除，尚不进入基础矩阵条件收口；矩阵最多60新意图及扩展仍属后续独立授权。
