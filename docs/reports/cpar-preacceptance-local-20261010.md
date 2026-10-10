# CPAR 验收前置收口与首批真实验收准备（2026-10-10）

本批独立本地工作已落实：补齐 Web／Console stock 嵌入页定向回执，修正正式执行器的 3／2／4 次场景与发送前隔离门槛，形成 Grok 首批真实请求清单及 Kiro／Grok Web 具体取证任务。**真实验收仍 BLOCKED／NOT_RUN；全项目规格保持 PARTIAL。** 独立评审和 GitHub 写后核验以本报告后置终态为准。

## 基线、工作区和归属

实际起点为 `codex/prism-v4-delivery` 的 `0258bcfd3378572fd009c6b8978cbaf5983c3b75`，产品行为及既有正式 Full 基线为 `c1b304d02e61fe1c75653a32fba920093462add6`。GitHub 起始 #10–57 共48票、35 CLOSED／13 OPEN；#16 CLOSED；#39／#49／#50／#57／#9 OPEN。已读取适用 AGENTS、工单约定、领域／ADR、跨边界日志及实际正文／评论／原生依赖，不把历史检查点当回退指令。

本批仅改 backend-owned acceptance 脚本／测试／模板和报告，不改 `web/prism/**`、产品 Rust、权威 OpenAPI、生成客户端、schema、配置或部署。没有触碰对方拥有的路径，不需要跨边界修改记录。E-05 已批准的同授权 Build 正常刷新例外、T12 71项和#57 AC02 PASS_LOCAL_BOUNDED、#26／#55 已补审范围均按来源复用。

保留基线覆盖25,704个 preexisting git-visible modified/untracked文件，以 lstat size/mtime_ns/mode 和 AGENTS字节SHA 核对，不能声称全部文件逐字节hash。ignored nested artifacts不在枚举范围。[保存基线](assets/cpar-preacceptance-20261010/baseline.json) 指向本地完整清单及其hash；不会提交无关 AGENTS、CONTEXT、docs/agents、其他output/evidence或未跟踪文件。后置保存核对记录实际差异和提交范围。

## A：Web／Console各自页面证据

历史 [补审终态](assets/cpar-channel-delivery-20261010/review-historical-supplement.md) 的 B74 是 shared production management HTTP：Web和Console分别201、observed identity、native存储重开、403／409保护；旧日志与receipt的hash已经独立核实。它没有各自专属页面receipt。旧缺失记录保持 PARTIAL，不改写成找到了旧证据。

此次用生产 `gateway serve` 的实际 `/admin-ui/#/accounts`、真实生成管理客户端和管理HTTP，合成SSO、受控TLS身份服务及独占SQLite完成 **13项 PASS**：[原始页面回执](assets/cpar-preacceptance-20261010/page/web-console-final.json)。EgoLite专用空间41；独占服务PID99768、origin `http://127.0.0.1:59749`，已经停止；空间已finish，不接管共享浏览器或服务。

| 各自定向行为 | Web | Console | 实际证据 |
| --- | --- | --- | --- |
| 入口／材料分类 | `Grok Web` → SSO，合成 `grok_web_sso` JSON | `Grok Console` → SSO，合成 raw SSO | 从账号页真实按钮操作；不是OAuth官方授权。POST body.channel分别 `grok.web`／`grok.console` |
| 保存成功 | PASS | PASS | `/admin/native-accounts/import` 201，created=1、identity_state=observed、runtime_applied=true；native库+1，无ordinary Credential／binding |
| 身份服务暂不可用 | PASS | PASS | 受控身份GET503，管理201仍保存、identity_state=unavailable；页面“渠道暂未返回身份”，不冒称已知身份 |
| 导入真实响应丢失 | PASS | PASS | CDP只丢已经真实201的response，先确认SQLite提交。页面结果未确认；点击核对仅GET `/admin/native-account-imports/{batch_id}`，channel/account/saved精确匹配原写入，无重复POST |
| 应用真实响应丢失 | PASS | PASS | 丢真实200 apply response；apply按钮禁用，先GET `/admin/system`；服务可接收请求不冒充先前write已确认，页面仍说明结果未确认 |
| 关闭与重载 | PASS | PASS | 完成关闭dialog；重载按真实会话规则重新登录；native库快照保留，无ordinary/binding增长；实际目录继续显示Grok授权 |
| 隐式推理 | 0 | 0 | 受控上游身份GET共6；页面最终快照额度POST15次，服务停止后合计18次，分别单列；任何其他native POST均记为unexpected inference并拒绝。推理receiver=0，Request/Attempt/Usage/ledger事件表为空 |

授权保存、runtime应用、推理成功是三种不同事实：导入201的runtime_applied不等于推理成功；丢失apply时只保留未知；native账号已保存也不等于已配置推理接口。页面原有“配置待核对；这不代表真实调用已验证”和“授权保存与接口接入是独立步骤”可见。截图辅助可读性：[Web回读与未知应用](assets/cpar-preacceptance-20261010/page/web-console-final-web-loss.png)、[Console保存](assets/cpar-preacceptance-20261010/page/web-console-final-console-success.png)，行为判定来自管理HTTP／持久化／接收断言。

runtime apply明确失败、global admission阻断与read-first恢复、无／唯一／多配置匹配、权限、停用和同身份更新按旧shared fixture／stock lifecycle／B74及#55来源复用，不机械复制全部笛卡尔积。此轮没有新增这些产品控件或改变原来的层级。独立历史覆盖评审另核对这些关联；stock新证据不冒充原fixture的真实Provider层。

实际stock binary由 `c1b304d0` 干净source离线重建，SHA256为 `4648f651c58d1a8e1e0ff63bf720b7ad7546711859ce60478bb173ff10dc1f7e`，见 [build receipt](assets/cpar-preacceptance-20261010/stock-build-receipt.json)。页面driver在0258工作区带本批变更执行；原`sourceSha`是driver checkout HEAD，**不是binary源码**，公开binding明确两个角色，并记录实际shim hash。

stock固定 `grok.com` identity策略拒绝loopback DNS；测试interpose提供合成global DNS标记，随后connect只改送本机TLSfixture。OS sandbox对外部socketdeny，经本机连通／TEST-NET外连PermissionError验证；TLS保留原hostname/SNI和临时CA，不关闭证书验证。该机制仅加载进本批gateway子进程，没有改生产egress策略，没有连接真实Grok。

## 原失败及归因

| 原执行 | 原状态 | 归因／修正 |
| --- | --- | --- |
| web-console-first | FAIL | fixture让DNS返回loopback，stock admission正确拒绝，identity=unavailable；fixture使用global marker + loopback connect interpose。不是产品RED |
| web-console-second | FAIL，已有web保存PASS | driver误以为reload后管理session仍在；stock要求重新登录；按真实登录流程补driver。不是产品RED |
| web-console-third | FAIL，12个页面检查已PASS | fixture误把 `/rest/rate-limits` POST全部当推理；事件表为空。按真实path拆开额度与推理计数。不是产品RED |
| formal-scenario-red | FAIL，六protocol/mode子例均7≠9 | 正式执行器的旧2／2／3请求不满足首批3／2／4要求；属于 acceptance executor RED，非产品Rust／UI缺陷 |
| final页面／formal-green | PASS | 页面13项；formal oracle32 tests；全部原失败receipt/log保留，未回标成功 |
| af81独立Spec／stale-count-red | FAIL，P2 | 已执行且观察过期的历史bundle在预检提前返回时误报sent_requests=0；定向RED为0≠7。66330dc6把原计数移到预检前，v2七次／v3九次回归PASS；原Spec失败与RED分别保留 |

没有发现需要改变产品Rust、UI或管理契约的新缺陷。三个驱动失败见 `assets/cpar-preacceptance-20261010/page` 原回执；formal RED的 [阅读log](assets/cpar-preacceptance-20261010/formal-scenario-red.log) 保留六个7≠9，GREEN另存，不拼接为一次成功。原始RED日志含行尾空格，66330dc6的docs门槛因此FAIL；原字节已压缩保存在 [原始归档](assets/cpar-preacceptance-20261010/formal-scenario-red.raw.log.gz)，SHA256仍为 `119f647eff9291b9b6c18e33e3bb320cb3c1cc35549df8e75e7e54b0b95f8f25`。阅读副本仅去除行尾空格，转换和原索引解析方式见 [归档回执](assets/cpar-preacceptance-20261010/formal-red-archive-receipt.json)，没有改写原FAIL或原execution-index。

## B：正式入口修复与首批准备

复用既有 `cpar-batch-c-live.py --new-build`，不新增替代harness。新run使用 `basic_scenario_version=3` 的3／2／4共9次；旧无版本或version2 bundle保留2／2／3历史scope，不将旧七次receipt升级为新九次。文本完整历史、两工具周期真实ID与结果配对、strict terminal、实际runtime和source、精确Attempt及原生Usage／ledger检查沿用。

新增发送前检查：GET inventory fresh／complete／同active configuration；route max_attempts=1；仅一个enabled candidate；frozen endpoint/model；该endpoint池仅一个账号身份并匹配已冻结lease revision／eligibility。因为credential_scope只有all_active，单candidate本身不能证明单账号。额外候选／账号／重试或unknown状态时在任何HTTP推理前拒绝，不等事后发现错账号才停。

live还需用户明确批准、exact process/artifact/source/target/URL/场景绑定、9次请求及USD预算、包含input／visible／reasoning／cache／request fees的完整费用上界证明。proof由操作者审核真实材料，脚本检查字段／绑定／算式，不能替上游认证协议语义或替用户批准。

上述门槛核对的是命令开始时的冻结观察，不是原子调度隔离或每次发送前的新快照。逐轮actual归属在响应后核验；真实执行必须保持专用实例冻结、不并发修改配置／账号。否则第一处漂移可能已经发生一次付费请求，不能声称所有运行中漂移都由脚本事前阻止。费用proof还必须单列function/tool output覆盖，因为官方visible-only字段同样不限制这部分。

已核实用户可选模型为4.5／4.6／4.7；建议主批Official + 4.7，Build作可选第二渠道。当前实际账号／host／目录／capabilities全部UNKNOWN；[授权清单](cpar-first-grok-authorization-20261010.md) 包含12份保护性JSON草案，每渠道一个模型、6组合，基本54次，缺Pin时另6次，目录最多1次，额度／OAuth／重试／扩展／故障／重跑0。本批实际真实请求0。

公开资料新增一个实质性计费限制：官方 [Responses Markdown](https://docs.x.ai/developers/rest-api-reference/inference/responses.md) 的max_output_tokens只限制visible output，排除reasoning/function calls。128不能证明总费用上限。主批USD1.00是建议待批准总额，费用proof仍NOT_PROVEN；第一Pin之前停止。Official API价不能套给Build／Console套餐，已冻结来源hash及实际UTC。

后继矩阵原文件不改：198基础行中186适用仍未真实执行、12个Web工具accepted NA、其余9渠道162行可独立准备；49项actual_declared=null保持unknown。此次新增page和driver证据是local增量，不能填成当前真实combination或已声明扩展PASS。

## C：原生具体取证任务

[Kiro／Grok Web任务](cpar-native-evidence-tasks-20261010.md) 已逐项列出当前来源／证明范围、需要的当前client版本或event样本、账号／请求／费用权限、最小观察及字段、脱敏／停止条件、解除阻断后的最小实现和public验收。

Kiro K01–K05覆盖native字段／范围／单位、reasoning计数、cap-hit终态、endpoint/credential/region/profile；Grok Web W01–W06覆盖当前REST或WS、可信conversation/parent成功推进、并发／取消／失败／过期、owner/account/revision/egress、Messages cap及材料取得。最小live观察建议分别Kiro4次、Web最多10次，均另批且未授权；未知失败方式或全计费上界时发送0。不重复下载旧SDK，不猜parent，不用提示词预算／下游截断／删必填参数替代原生契约。

#39／#49／#50保持OPEN；各自native语义、全部承诺真实组合和依赖尚未满足。Web工具及Messages精确计数既有限制保持原范围，Build E-05不扩到Web。

## 验证、复用与分层终态

| 范围 | 结果及证据 |
| --- | --- |
| 当前stock页面 | PASS_PUBLIC_MANAGEMENT_HTTP_AND_BROWSER_LOCAL；13 assertions，6 identity GET、15 quota POST（最终页面观察时）、inference0；最终停止后的raw接收数量另保留 |
| 正式oracle定向 | 首次GREEN为PASS_LOCAL_SIMULATED；32 tests；六protocol/mode各9次，共54个positive loopback receives；8种first-response故障×6=48次各只发1；11种pre-send阻断×6=66项 actual receive0；[run receipt](assets/cpar-preacceptance-20261010/formal-green-final-run.json) 与6个 [组合回执](assets/cpar-preacceptance-20261010/formal/responses-json.json)。后置费用tool_output缺失增加第12类pre-send负例，按修订后不可变来源另记录，不能覆盖原GREEN |
| 静态／文档 | 后置不可变commit的定向语法／docs门槛及hash核对记录在终态资产；不把未运行项写成PASS |
| 产品Full | PASS_LOCAL_REUSED，仅产品／未变门槛范围：c1b干净Full45/45，Rust1500/0/12；[历史摘要](assets/cpar-channel-delivery-20261010/verification-summary.json)。本批变动formal oracle另重跑，旧Full不能冒称测试了新driver |
| 前端／T12／E-05 | 未改相关源码；沿用明确已有70文件513测试、T12 71、strict same-grant例外及#26/#55来源；不重新全站验收 |
| Standards／当前diff Spec | 0258→3f初审、3f→af81补审、af81→663补审分别归档。最终Standards PASS（hard0）；Spec PASS（hard0），af81原P2已独立确认resolved，原失败未删除。初审重复fixture低优先级建议由af81移入共享helper解决 |
| 真实Provider／远端CI／发布制品／部署／生产 | 全部NOT_RUN；源码未push，没有当前源码的GitHub blob/commit链接 |

本批产品源码未变，定向页面、executor oracle和静态门槛与风险相称；不重跑不相关整套Rust/前端/T12。没有把原Full网络失败、driver失败或历史dirty归属改成clean／PASS。

## 后置收口

独立终态已归档：历史Web／Console明确页面缺口及其原映射范围 **COMPLETE、hard0**，不声称重审全项目88个映射；全项目规格仍 **PARTIAL**、真实验收 **BLOCKED／NOT_RUN**。当前diff的Standards、Spec分别以初审和两段delta组成最终PASS，见 [初审Standards](assets/cpar-preacceptance-20261010/review-standards-initial.md)、[初审Spec](assets/cpar-preacceptance-20261010/review-spec-initial.md)、[af81 Standards](assets/cpar-preacceptance-20261010/review-standards-af81-delta.md)、[af81原Spec失败](assets/cpar-preacceptance-20261010/review-spec-af81-delta-failed.md)、[663 Standards](assets/cpar-preacceptance-20261010/review-standards-663-delta.md)、[663 Spec](assets/cpar-preacceptance-20261010/review-spec-663-delta.md)、[历史页面补证](assets/cpar-preacceptance-20261010/review-historical-page-closeout.md)。评审不包含后续纯报告／归档／GitHub读回记录，也不冒称这些已独立代码审查。

最终executor代码66330dc6在独立干净worktree通过33 tests：六protocol/mode正向54个实际loopback接收，8种first-response故障×6各仅1次，12种发送前拒绝×6均接收0；旧v2和新v3过期观察仍保留已发送7／9次。见 [不可变执行回执](assets/cpar-preacceptance-20261010/verification-663-with-docs-failure.json) 和 [原oracle log](assets/cpar-preacceptance-20261010/final-clean-oracle.log)。同次docs前六项PASS、原RED日志行尾空格使第七项FAIL，原结果保留，修正归档后另验不覆盖失败。

GitHub随后只维护#16／#39／#49／#50／#57／#9既有进度评论；独立GET读回成功后记录终态，正文／已勾AC02／state／评论数量及原生依赖均需核对。保存核对已确认25,704条基线metadata与AGENTS字节SHA未变，见 [核对回执](assets/cpar-preacceptance-20261010/preservation-before-closeout.json)；提交范围与GitHub终态另补机器证据。

下一步无需真实授权即可审阅上述本地证据与具体取证／首批清单；真实执行必须等待入口、隔离host、单账号范围、逐动作次数和货币上限的明确输入／批准，以及完整费用proof。不得以模型选择代替这些授权。
