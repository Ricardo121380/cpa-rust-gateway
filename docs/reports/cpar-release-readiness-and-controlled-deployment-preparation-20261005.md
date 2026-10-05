# CPAR 发布就绪复核与受控部署准备

日期：2026-10-05，Asia/Shanghai。**复核和可独立进行的部署准备已经交付；当前候选仍 BLOCKED，不能归结为只差部署授权。** A／B 静态审查覆盖已补齐，但当前源码有13项P2规格问题及若干验收／契约缺口。AC-01历史密文门槛保持PASS；签名、CI及制品绑定PASS。Krill首批Key已到期，另需针对具体权限对象处理。

本轮产品代码、生产数据、服务停启、current切换、迁移、配置／账号／权限、真实推理、GitHub写入均为0。报告与附件只作本地交付；用户已有AGENTS.md修改和所有无关内容保留。[结构化判定](assets/cpar-release-readiness-20261005/release-readiness.json)、[#57归集](assets/cpar-release-readiness-20261005/issue-57-evidence.md)。

## 身份与证据对应关系

| 层 | 当前已核对事实 | 边界／来源 |
| --- | --- | --- |
| 工作区输入／报告HEAD | codex/prism-v4-delivery @98230abc9ccf0b7478a1e4e7d042828c506d4bdf | [输入基线](assets/cpar-release-readiness-20261005/local-baseline.json)；后续本轮报告提交另计，不能用来构建候选 |
| 候选源码 | 80f5838902eb56be6dccc3ce047eb227d6c18e6c | HEAD与candidate间仅报告文件；本轮无产品差异 |
| 签名ARM64 binary | SHA256 542a3c30aae7f09cac80b7cce33f3a153a87a93fe1e842cd6d8c8e355a930a87，19,500,320bytes | [制品复核](assets/cpar-release-readiness-20261005/artifact-verification.json)；旧unsigned hashbd2不能混用 |
| release workflow | [run36956561884/attempt1](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36956561884)，两个native架构success | [实时workflow](assets/cpar-release-readiness-20261005/workflow-live.json)；专用ref仍source80 |
| 两个平台集合 | 各9文件与原验签集合逐字节一致；5payload各hash/size、manifest、receipt、ZIP digest一致；GitHub未过期，至10月16日 | [GitHub制品](assets/cpar-release-readiness-20261005/workflow-artifacts-live.json)；host ARM64完整集合也一致；密码学结果复用原exact-byte cosign，未重签 |
| 历史candidate执行 | source80 Full45/45；signedARM64隔离410；54旧表迁移／完整旧状态恢复／旧新原生18操作 | [execution报告](cpar-execution-evidence-and-arm64-candidate-20261002.md)、[签名演练](cpar-signed-artifact-and-production-state-rehearsal-20261002.md)、[AC01 fresh-copy](cpar-archived-credential-production-repair-20261002.md)；不覆盖本轮新发现的未测分支 |
| 现场2026-10-05 20:01 CST快照 | new-vps / instance-20260726-2136 / PID499070；旧sourceb0cf36e387b774439acc23830221389eb3ac74d0，schema31 | [现场完整脱敏复核](assets/cpar-release-readiness-20261005/host-readonly-audit-reconciled.json)；未部署候选 |
| 现场binary/current | SHA2560b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b；current和/proc/exe均旧release | [只读现场](assets/cpar-release-readiness-20261005/production-readonly.json) |
| 最后只读复验2026-10-05 20:34 CST | 同PID499070、旧hash、schema31、NRestarts=0；Caddy实际UID999；维护nft表不存在；9个ARM64路径均普通文件 | [最后现场回执](assets/cpar-release-readiness-20261005/final-site-readonly.json)；此项不冒充再次完整两库比较 |
| GitHub规格与工单 | #9、#10–57共49项仍OPEN，无后续评论／实施更新；最新body/updatedAt已取证 | [GitHub读取回执](assets/cpar-release-readiness-20261005/github-issue-receipt.json)，公开完整body在issue-sources；没有远程写入 |

按实际读取了项目AGENTS、issue-tracker、相关最新跨界记录、指定七份报告、AC01 release-readiness、原发布方案及基础领域／ADR。父规格[#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)、[#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57)和涉及工单逐条对照。基础与交接／发布文档由主代理亲读。现有报告没有后续新候选或已部署事实，未重复已完成签名／Full／演练。

## 审查覆盖与本轮两轴结论

完整固定点、实现／修复提交、原partial遗漏、后续修复覆盖范围及本轮精确范围见[审查覆盖表](assets/cpar-release-readiness-20261005/review-coverage.md)；各diff/file/commit hash见[manifest](assets/cpar-release-readiness-20261005/review-scope-manifest.json)。

| 批次／工单 | 原始与后续链 | 已有证据与原遗漏 | 本轮结果 |
| --- | --- | --- | --- |
| A／#10–26 | 9fc4ceb4→24142a90；13类原修复包含同提交，再核对A文件到candidate80 | 原Standards complete复用；Spec两轮partial，只读关键seams，不是96文件完整覆盖 | 原实现／修复／候选关联范围96/96已静态覆盖；Spec **FAIL**，8项P2。不是因真实Provider未执行而把源码审查标partial |
| B／#27–38 | A→b8cba0f4初评；b8→f6c37ef0修复检查点；f6→45a315a6报告交付；再至candidate80 | 原Standards大型diff截断；Spec codec分支、错误／隔离矩阵及进程轮换未穷尽 | 73/73完整静态覆盖；Standards **FAIL**（1P3），Spec **FAIL**（5P2）；执行／合同证据另保留PARTIAL |
| D／#51–56 | ff23f2d8→7478d276→d837056b | 已有限定完整两轴与4文件文案增量PASS | 复用其准确范围，不重复D全审，不扩大到A/B/C |
| C／execution切片 | f027e0bf→9499db78→c90656ab→2e43ea4b；09e70e2→ff77a5f→c94b35e→179cf325→80f5838 | 对各自修复／执行身份切片的两轴PASS | 复用限定结论，C/#57/#9仍未完成 |

b8和f6是可解析的历史检查点，**不是最终45a的祖先链**，merge-base都是A。原Skill三点比较与修复双端点比较分别保存；不能用三点diff伪称b8→f6修复增量。b8→f6两端点11文件，f6→45a仅3个报告／回执文件且产品树相同。

适用code-review流程按Standards／Spec分开只读独立审查。探子均generic、fork_turns=none、单轮，派发后主代理只等待收敛。原A前端探子无终态被中止，未计覆盖；四个新轻范围组和主代理9个契约文件补齐。B证据组错误schema名导致终止，主代理完成剩余真实schema／生成契约／trailer和helper检查；B相邻runtime遗漏由一个新轻范围组填齐。[终态、来源和主代理核定](assets/cpar-release-readiness-20261005/review-results.json)。

**已完成审查 ≠ 软件符合规格 ≠ 真实验收完成。** 静态发现的触发条件可由源码确定，本轮没有新增执行复现或修改产品；每项最小修复、文件行号和必要回归都在[完整发现报告](assets/cpar-release-readiness-20261005/review-results.md)。

| 编号／定级 | 确定的源码问题 | 直接影响 |
| --- | --- | --- |
| A-01／P2 | legacy Codex callback→persist_oauth_credential_if_revision无old/new身份guard；management_mutation_service.rs:1288，management_resources.rs:6923 | 可用他人／未知身份的有效回调覆盖原凭据，原bindings仍保留；首授guard不覆盖这条路径 |
| A-02／P2 | AuthorizationRecovery.tsx:24及AddAccountDialog.tsx:228–240将已确认receipt与后续读取绑定 | 后续503能抹掉已确认保存事实 |
| A-03／P2 | configurationTask.ts:117／ProviderDialog.tsx:86未结清已发布pending draft | 误示当前服务未变，下一个接入拒绝now-active pending ID |
| A-04／P2 | configurationTask.ts:54按“存在draft”禁用autoApply | 无旧修改的空draft也不能按规格自动应用 |
| A-05／P2 | KiroDeviceDialog.tsx:53／KimiDeviceDialog.tsx:161–172关闭unknown仅卸载 | 丢精确receipt恢复键，未标记持续待确认任务 |
| A-06／P2 | accountStatus.ts:6缺材料类别 | API Key失效仍提示重授权 |
| A-07／P2 | AccountModelsDialog.tsx:75只判断retained true | refetch／读取失败期间仍宣称当前身份调用观测 |
| A-08／P2 | AccountRuntimeSummary.tsx:39使用失效retained authProblem | 新授权事实被旧expired快照覆盖 |
| B-01／P2 | protocol_transform.rs:950–963不允许Responses后续text part | 合法native多part输出不能回传下一轮 |
| B-02／P2 | Responses upstream_response.rs:623–629无条件忽略done事件 | 冲突终态／part确认被丢弃 |
| B-03／P2 | 同文件:1143只比ID集合，native_output.rs:322按start序输出 | terminal改变item顺序仍成功，顺序被静默重写 |
| B-04／P2 | runtime.rs:6060 bootstrap解码失败，:4857在source返回前丢弃 | 首chunk合法Usage后坏帧仍丢Usage；原修复覆盖已建立source的另一边界 |
| B-05／P2 | deployment.rs:471–476只装一个version1 master-key | 生产serve无法表达旧＋active keyring，store单元测试的轮换能力不能在进程入口使用 |

Standards文档P3：新ADR-0100未满足docs/adr/README.md:12–24必填结构，尤其Validation and rollback；2项维护建议仅是不用参数／重复protocol名称，不是新发布阻断。

仍缺的**执行证据**：A四API预设、设备成功保存后丢响应，Kiro fixture材料分类错误；B公开隔离矩阵缺部分实际HTTP单元格。另#36字面revision变化拒绝与既有BC-RESP-003 proven same-grant例外有明确冲突，保留PARTIAL_SPEC_CONFLICT交规格负责人处理。本轮没有新增例外／删要求，也没有把fixture错误写成生产错误。

## 最新现场、材料、恢复依赖与漂移

唯一服务仍active/running，启动时间2026-09-28 09:23:42UTC，无自动重启；User/Group cpa-gateway、UID995/GID986、data和management仅127.0.0.1:18180/18181，unit与60-prism-https-origin.conf内容和historical capture相同。六持久源key与LoadCredential及历史capture一致，两库完整性/FK通过。unit SIGTERM／control-group／45秒停止截止，实际无established服务连接的快照不证明未来窗口无在途。

20:01 CST的只读比较以10月2日capture-post为固定基线，没有要求运行整库字节不变：

| 对象 | 当前变化与核对 | 判定 |
| --- | --- | --- |
| AC-01 | 目标／父／active配置关系及所有列与post修复相同，ciphertext等于保留repaired-envelope；普通41/41，archived32/32、active3/3、draft6/6，native5/5精确AAD认证 | PASS；不是再次修复 |
| 47个控制表＋认证库 | configuration、ClientKey／AccessGroup／route／binding／egress、RequestRecording、gateway_event_log、Usage/ledger和所有历史账号关联保持；管理认证库逻辑相同 | PASS |
| 活跃Kimi普通材料 | revision1004→1627，只有ciphertext/revision变化；623条credential_oauth_rotated对应相同精确身份，revision增量等于审计数，latest11:59:42UTC | 正常续期；非AC01／父 |
| refresh_state两行 | Kimi credential_revision/observed_at匹配；既有Codex失败123→196，只改retry_after/failure_count/observed_at，材料／revision/错误类别不变、无claim | 正常续期／失败退避，不冒称该Codex可用 |
| Grok Build一行 | 同账号opaque5beff1…，revision100→112，仅材料、刷新时间、revision／updated_at／continuation-current变化；identity、key_version、enabled、auth_status保持 | 正常刷新；generation148→160恰好+12；当前continuation_current不等于native revision，未把它当可用连续性证明 |
| 四catalog targets | snapshot20→23、5→9、19→22、20→23及观察／freshness时间推进；模型／failure rows不变，目标集合未增减 | 正常目录刷新 |
| 已到期Krill Key | Key行／grant字段未改变，但到期时间已过；新gpt6-sol route只授予这把Key，长期Key只有旧disabled gpt5.5 route | 首批验收BLOCKED；时间推进不等于库被修改 |

第一次定向读取因为native availability在旧构建里为bool而非dict发生本地projection错误，SSH已连接；修正为类型安全脱敏projection后读取成功。失败与成功[原始回执](assets/cpar-release-readiness-20261005/live-inspect-receipt.json)均保留，未误判主机不可访问。凭据、密钥、原始数据库、解密材料和私有身份内容始终留在受保护主机内；附件仅含版本、hash、opaque标识及必要的脱敏状态。

完整9文件ARM64实际payload与本地／manifest／签名回执一致；历史capture-2、restored-pristine、migration-entry-probe、AC01capture-pre/post及post-migration-pristine仍在root0700 task目录。4个恢复工具内容与已演练本地工具同hash，python/sqlite/cryptography、namespace、setpriv/bwrap、nft/ss及旧ELF依赖可用。state约1.37GiB，空闲约28.3GiB；8份state＋old制品＋2GiB余量约13GiB，满足当前配方预算。state、keys、backup的实际文件系统device均2049，可在私有窗口内原子移动保全；将来若跨文件系统，操作附件会停止，须先修订方案。这个检查不产生新的offline快照。

## 发布门槛与具体切换、回退

逐项状态／证据／层次在[release-readiness.json](assets/cpar-release-readiness-20261005/release-readiness.json)。主要区分：

| 门槛 | 状态 | 结果含义 |
| --- | --- | --- |
| A/B审查静态覆盖 | COMPLETE | 原PARTIAL范围补齐，软件判定仍FAIL |
| 13项源代码规格问题 | FAIL / BLOCKED | 先修复并完成必要定向回归，重新绑定候选 |
| B ADR／验收矩阵／revision合同 | FAIL／PARTIAL | 各自最小任务，不能用Full或real预算替代 |
| 源码／签名／CI／payload | PASS | 仅source80/hash542；产品改变后须重建身份链 |
| 旧现场／AC01／密钥／漂移 | PASS | 绑定这次观察时间，切换前再核对 |
| 历史完整迁移／恢复／signed回归 | PASS_REUSED | 原固定候选证据；没有重跑或删除失败日志 |
| 操作配方／恢复工具／空间 | COMPLETE_STATIC_ONLY / PASS | 准备交付；实际维护操作未执行 |
| 新鲜离线pre-upgrade／恢复验证 | NOT_RUN，窗口内执行 | 不在审批前停服，不用旧快照冒称最新 |
| 首批Krill Key | BLOCKED | 另需具体Key有效期／独立新Key授权，不能扩已有共享Key全部模型 |
| 新构建实际execution字段／真实组合 | NOT_RUN，部署后检查 | 旧构建缺字段不构成循环前置 |
| C／#57／#9 | BLOCKED / OPEN | 全范围尚未满足，保持真实渠道独立缺口 |

[完整可审查操作配方](assets/cpar-release-readiness-20261005/controlled-deployment-runbook.md)已经明确唯一对象、安装路径／hash、维护影响、零在途条件、SIGTERM实际机制、新的临时data-port准入表、最新整目录state/auth/六key/old-release/部署元数据备份、一致性边界、恢复验证、31→34及启动后数据／权限核验、停止条件与完整回退。

[操作附件](assets/cpar-release-readiness-20261005/approved-window-operations.py)只在将来明确获准窗口内运行：capture→verify→保留门控切换→post核验；失败先preserve整套post现场，再根据root0600数据决策恢复完整pre-upgrade。归档SQLite始终不直接打开，先派生复制，避免只读WAL操作修改SHM。恢复保留displaced-live，不删除新增记录；只改binary/current或migration down不构成回退。

若候选已经写入Request／Attempt／Usage／账本、stored／compact、执行sidecar、native import或后台授权材料，全部保留；不自动合并／重发／重算账。由数据／授权负责人明确决定旧状态恢复和后续对账，未知写入不能当零。临时nft表及业务开放时点必须包含在具体部署授权中；本轮没有实施，其他service／unit／Caddy不变。

## 首批真实验收范围

仍按Krill / gpt-6-sol / Responses JSON，然后Grok Build / grok-4.5 / Responses JSON。每组1次protected Pin＋7次formal basics，最多8次，两组合计最多16次；本轮0。精确账号／route／candidate／Key opaque ID／egress policy与状态见[前置回执](assets/cpar-release-readiness-20261005/first-acceptance-preconditions.json)，现有freeze／preflight／execute命令仍在[修订的原方案](assets/cpar-execution-evidence-20261002/release-and-first-acceptance-plan.md)。

Krill账号、Responses连接和新route active/available，但唯一Key于2026-10-04 12:02:20.553CST到期，长期Key没有新route；不能发请求，也不改成旧gpt5.5。Build的目标opaque5beff1… active/available；两个其他Build账号过期保留，拟用Key7b7960…无expiry且有精确Build权限。native revision112不当成lease revision，正常后台轮换仍要求执行前fresh冻结。

实际请求须绑定已部署binary/source／PID、active config及actual revision、同一lease账号／credential revision、candidate/upstream model、实际出口、ClientKey范围与完整serving_snapshot声明。先部署后GET新字段，再获批Pin取得真实execution identity，再freeze/preflight；管理ETag、native revision及policy direct不能代替actual attempt事实。任何unknown／drift／unsupported／authorization／Usage-ledger不符停止剩余预算，不自动换账号或重试。

Kiro输出上限、Grok Web原生多轮／输出上限及未知relay onward继续BLOCKED；其他协议JSON/SSE与声明的reasoning／parallel／store／continuation／compact／WS扩展保留原准确状态。基础JSON成功不关闭扩展要求或#57，不关闭能力声明来取得PASS。

## 最小后续任务与责任

1. 产品后端：A-01及B-01–05，按完整发现的最小路径修复与定向负例，保护既有修复和不可近似／丢语义规则。B-05需生产进程层面的ring能力与实际重启证明。
2. Prism负责人：A-02–08的receipt／pending／身份事实／恢复键分支；需另行明确这次frontend修复范围，当前任务只有只读审查与报告。
3. 文档／验收负责人：S-01及E-01–04；规格负责人处理E-05既有合同冲突。不要将执行缺口塞进“真实Provider才能证明”的范围。
4. 运维／权限负责人：在针对具体Key的授权下修复Krill首批前置；本轮不改过期Key、不扩大长期共享Key权限、不重新授权账号。
5. 候选／发布负责人：产品修复后重新冻结source与签名制品，绑定适度定向／要求门禁、审查和依赖配方。满足所有软件与现场停止条件后，才能申请runbook末段明确的maintenance／backup／switch／rollback／admission范围。真实16次预算与业务开放另行明确。
6. #57归集继续逐项连接88故事/F01–F08/T01–T15与A/B/C/D、兼容、真实、部署证据，C/#57/#9不关闭。

当前交付是“复核完成，发布仍BLOCKED”，**不是“部署准备完成，只等授权”**。本轮没有向用户重复申请已完成准备工作；也没有把AC01修复授权扩大成部署授权。

## 本轮验证

产品未变，不重跑Full、完整构建、远端workflow、签名或真实推理。仓库docs门槛通过7/7，见[检查日志](assets/cpar-release-readiness-20261005/docs-check.md)；新报告／JSON／操作附件AST、链接／格式、脱敏及限定diff与最终工作区核对见[静态交付回执](assets/cpar-release-readiness-20261005/delivery-static-receipt.json)。操作附件实际维护执行NOT_RUN，明确保留为将来窗口内门槛。
