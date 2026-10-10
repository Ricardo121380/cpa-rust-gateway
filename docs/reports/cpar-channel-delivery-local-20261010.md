# CPAR 渠道软件缺口收敛与交付前置补齐（2026-10-10）

本批交付 **PARTIAL**：新增T12合成既有数据升级／一致状态恢复验收，并复现、修复了真实旧Usage读取／重放兼容缺陷；完成原生语义查证、独立历史Spec补审、后继组合台账和下一轮真实验收计划。Kiro原生硬上限、Grok Web原生多轮／Messages cap仍有具体语义与软件阻断。**冻结候选 NOT_MET**。本地、fixture、公共HTTP、既有EgoLite、真实Provider及交付层分别给结论。

## 基线、范围与提交

实际起点为分支 `codex/prism-v4-delivery`、HEAD `d3d2e3c5bf0d9521f3fb0d830344b34053f62155`；最后前批行为与驱动源码 `36079c1fad666c2bbb9c15bd8f8a59518c970b42`，中间为报告增量。实时工单正文、唯一进度评论与原生依赖已读取：#39前置全部关闭；#49前置全部关闭；#50仍有OPEN #39；#57/#9仍OPEN。历史检查点不是回退指令。

本地实现提交有两笔：`611949b6faef9788d04b3283cc4c7ad45c6dcf38` 增加T12分支及驱动；`c1b304d02e61fe1c75653a32fba920093462add6` 修复 `crates/gateway-store/src/event_store.rs`、澄清 `docs/contracts/BC-OBS-002-append-only-sqlite-event-writer.md` 并收紧T12的合法legacy隔离断言。新增分支复用原公共HTTP/TLS harness及生产进程，不引入工作流引擎、第二调度器、依赖或通用迁移框架。

Store修复落实既有ADR-0100的旧证据保持unknown义务；没有新架构决策、schema或事件模型变化，因此不新增ADR。BC-OBS-002明确严格旧编码接受边界；OpenAPI、生成客户端、UI及shared surfaces无改动，不需要跨边界日志/trailer。用户AGENTS.md及原5817个未跟踪条目保留；核对方法为原路径的lstat size/mtime_ns/mode和AGENTS字节SHA，见[保存核对](assets/cpar-channel-delivery-20261010/preservation-check.json)，没有声称所有未跟踪文件逐字节hash校验。

[源码绑定](assets/cpar-channel-delivery-20261010/source-binding.json)区分初始源码、两个真实历史二进制、当前验收源码及制品哈希；[执行索引](assets/cpar-channel-delivery-20261010/execution-index.json)记录完整argv、cwd、UTC、source_before/after、退出码、raw SHA和归档SHA。gzip解压后与原raw逐字节相同，没有把失败重标或改写历史来源。

## A／B：原生语义与严格拒绝

详见[原生语义报告](cpar-channel-native-semantics-20261010.md)及[版本化一手来源索引](assets/cpar-channel-delivery-20261010/primary-source-index.json)。只下载公开原始定义／参考作者源码，没有真实授权或推理。

- Kiro：所查AWS IDE/Q CLI生成定义没有可追溯的原生cap字段/范围/计数/命中终态；旧Q SDK不能证明当前闭源Kiro CLI服务。当前Social IDE/CLI uncapped Chat／Responses工具多轮通过，合法Messages cap与全部24 cap行仍BLOCKED。当前特定凭据/地区/profile fixture不能外推所有组合。
- Grok Web：冻结参考展示WS与REST两种续接方言及不同conversation/parent来源；当前生产new-conversation builder/decoder未接入可信parent、owner和成功推进。六个文本多轮行继续BLOCKED；Messages额外缺原生cap。未拼接历史、猜parent或移除preflight。Web工具12行单列已接受不适用。
- 两个公共Actix handler→production factory定向测试均PASS_HANDLER_AND_FIXTURE。Kiro32次受控transport接收、16个cap拒绝零新增Attempt/接收；Web四种非法／不等价输入均400、Attempt0/DNS0。它们没有宣称真实Kiro/Web网络服务或三协议正例闭环。
- E-05已批准并本地验证；仅严格同授权Grok Build正常刷新revision例外。手动替换、重新授权及其他归属变化仍拒绝。没有重新请求政策选择。

## C：独立Spec补审

[独立历史终态与逐项来源映射](assets/cpar-channel-delivery-20261010/review-historical-supplement.md)记录两个只读agent。原139+45份log实际SHA与184份receipt归属一致；#26资格/expiry/health/quota/capacity/once/no-fallback和#55原global admission场景、hash、组件/driver一致性均COMPLETE。HTTP12、worker子场景、410原断言及223/223接收逐项映射完成。

**历史Spec覆盖PARTIAL，已查hard0**：旧报告的Web/Console专属页面回执未定位；共享HTTP已定位，不扩大为两条各自页面执行。本轮复用原浏览器执行及原时间，不称动态重跑，不新增任意TTL，不机械重开已关闭票。Standards不由这些Spec探子代替，最终两轴另审。

## D：T12升级、旧读者与一致恢复

历史二进制 `24142a9008e58fbc87d0b5791bbb166b45234850` 实际经公共管理/推理HTTP建立schema32数据；`08812db2373583b8a725b80071efe75b69f9d7ae` 是schema34且早于Attempt级Usage identity的真实旧读者。当前进程升级32→34。起点覆盖0033 ledger provenance、0034 Attempt sidecar以及无schema变化的ADR0103；不扩为所有历史版本迁移。

最终验收通过71项：18个集合升级前全部非空，按原列排序/哈希逐行核对升级后完全一致；包括账号、native显示身份、普通及native停用意图、endpoint连接、绑定、自定义路由、access group/Client Key权限、Usage、账本、管理/资源审计与stored Responses。旧JSON/SSE保存根由原owner读取，同组foreign Client Key404、受限模型权限404，零额外上游接收。

旧ledger新增provenance/input accounting保持unknown；两个合法编码但归属冲突的legacy Usage由当前生产worker标为invalid_lineage，保持未解决并显式排除，没有补造ledger或Attempt sidecar。新Usage ID严格为usage-attempt:SHA256(AttemptId)并与原Request/Attempt关联。旧schema32二进制明确拒绝未知schema34；schema34旧Usage汇总仍可200，不能据此称安全回退。在保留原source event的独立副本中，仅重置派生ledger/checkpoint，旧production billing worker消费新Usage时记录invalid_event、不产生该ledger，原source payload不变、零上游请求。

T12收紧隔离类别时出现真实产品RED：当前Store对没有provenance/input_accounting字段的历史Usage进行带新字段的重序列化比较，误报InvalidPersistedGatewayEvent；严格重放也误报冲突。先保留公共worker RED和实际SQLite Store读取RED，再修复共享比较函数：只额外接受已知的旧Usage字段顺序、六项nullable计数与原optional Attempt编码，且两个新证据值均须unknown。读取／重放不重写原payload或event ID，不补Attempt/归属/证据。计数改变、Attempt补入、未知字段及非规范格式仍拒绝；新测试验证原字节不变及幂等重放0新增行。最终T12从真实公共管理HTTP与生产worker验证隔离类别恢复正确。

保留key1并用key2写新stored Response，旧/新加密根全部可读，旧/新key_version分别1/2。历史配置经无review rollback409、重新核对review后成功，保留disabled意图。停止并flush进程后保留完整post-state+credentials+master-keys，再恢复pre-state：18个非空集合完全等于原schema32快照，真实旧binary读回成功。随后恢复post-state及两份精确key文件，重新读取四个旧/新会话根和Client Key权限，全部通过。

**回退结论：binary-only rollback UNSAFE；coherent restore PASS_LOCAL_BOUNDED。** 恢复pre-state期间后续写入不在活动状态；必须保留post-state，不能删除或拼接不同时间的DB/密钥。验收只操作自身私有临时合成目录；harness停止自身进程后保留pre/post快照、原始DB及合成credentials/key文件，方便本地复核。这些副本不进入仓库或GitHub，不能当作已归档的发布恢复套件；归档仅为非秘密计数/哈希/结果。这里没有验证生产恢复或发布制品。

T12实际公共loopback TLS推理接收5次，fallback0；全部进程受macOS sandbox-exec non-loopback拒绝策略约束，并验证localhost可达/TEST-NET出站EPERM。没有真实Provider请求。

## 验证与原始失败

Store新测试PASS1，既有legacy相关回归PASS6；修复后的工作区T12 PASS71，Kiro/Web定向各1 test PASS。最终干净T12与正式门槛、独立两轴状态见下述终态；早先68/70项通过未验证合法legacy的精确隔离类别，仅保留为历史，不能替代最终71项。

| 原始失败 | 归因与处理 |
| --- | --- |
| current-directed-build，exit101 | 历史与当前checkout共用Cargo target，出现旧crate接口；源码字段实际存在。改独立target构建通过，未改产品代码 |
| t12-directed-01，exit1 | 驱动误用relative expires_in作持久化导入；按实际absolute expires_at契约修正 |
| t12-directed-02，exit1 | 驱动误认旧Usage summary会拒绝新source ID；实际200。改验证旧生产账本消费边界，分别记录两层结果 |
| t12-directed-03，exit1 | 驱动对archived配置误用publish；改现有rollback入口及active revision/review契约 |
| clean-spa-dependencies，exit1 | 在worktree checkout尚未完成时启动npm ci，lockfile尚不可见；等待checkout终态后原命令通过，未升级依赖 |
| clean-full，exit101 | Full命令传入GATEWAY_RELEASE_REVISION，开发身份测试要求development；移除该覆盖、定向复核及正式门槛重跑。未修改测试或产品 |
| t12-directed-canonical-legacy，exit1 | 产品RED：合法旧Usage被Store误报格式错误，生产worker隔离为invalid_event；收紧类别断言真实复现 |
| legacy-usage-store-red，exit101 | 产品RED：历史编码的实际SQLite读取报InvalidPersistedGatewayEvent；同根因严格重放比较也需兼容 |
| legacy-usage-store-green，exit101 | 测试准备错误：试图UPDATE append-only表；改用独立损坏样本库，保留append-only保护 |
| legacy-usage-store-clippy，exit101 | 新局部类型位置及字段后缀触发2项lint；调整声明位置并记录冻结旧字段名的局部理由，后续Clippy通过 |
| final-full，exit1 | c1b干净Full的Rust1500/0/12及前44步通过；最后RustSec advisory-db拉取遇网络IO错误。单独cargo audit原命令重试通过，再运行完整Full，不拼接为一次成功 |

所有失败raw/receipt保留；两项产品RED由c1b修复，其他为驱动／测试准备／构建环境问题。没有本批已证实且未修复的产品RED；cap/续接阻断属于已知软件缺口与语义证据不足，不冒充未失败即通过。

## 后继台账与真实验收计划

[当前后继台账](assets/cpar-channel-delivery-20261010/issue57-successor.json)保留原矩阵path/hash及历史24 cap/12 metadata行；198新行明确audit_source与execution_source(null)不同。88故事、F01–F08、T01–T15和AC01–10都有历史pointer及本批局部增量；T12及AC02为PASS_LOCAL_BOUNDED，不自动关闭整个#57。186适用真实行仍BLOCKED，12个Web工具行NOT_APPLICABLE_ACCEPTED；49项actual_declared=null保持未知，不等于未声明。

[下一轮真实验收计划](cpar-next-real-acceptance-plan-20261010.md)列出组合、3/2/4轮场景、账号/模型/配置/路由/egress/实际声明冻结、证据格式、授权与预算。9渠道162行可独立准备；本轮不发真实请求。558次推理和64单位候选上限只是满足原生计数/单Attempt条件后的估算；价格、实际limit及货币授权仍null，扩展和授权查询另计。

## 正式门槛、两轴与GitHub终态

最终clean T12已在c1b干净检出及精确源码编译binary上PASS71，实际loopback上游接收5、fallback0。最终干净Full **45/45 PASS**，Rust **1500通过／0失败／12忽略**，包含类型检查、双构建、Clippy及正式安全／依赖门槛；原网络失败与单步恢复另行保留。frontend70文件/513 tests在clean611上PASS且web/prism的611→c1b差异为空，可静态复用。详情见[机器验证摘要](assets/cpar-channel-delivery-20261010/verification-summary.json)及完整执行索引。

实现证据提交 `85f43c2f28ee0a9bbaea3916abaec302b2cedcac` 固定两轴评审输入，范围d3…85f。独立[两轴原文](assets/cpar-channel-delivery-20261010/review-final.md)及[机器终态](assets/cpar-channel-delivery-20261010/review-final.json)：Standards PASS／hard0／heuristic1（非阻断重复恢复步骤）；当前切片Spec PASS／hard0；历史完整覆盖PARTIAL／已查hard0。没有把后置评审归档称为其自身已被固定diff审查。Spec确认用户限定T12范围满足AC02，台账同步PASS_LOCAL_BOUNDED，GitHub仅勾该条。

五票已有进度评论更新已逐次独立回读PASS；#57正文只有AC02从未勾改为已勾，其余正文／AC／评论ID和数量／OPEN状态／原生依赖保持。#39/#49完成后，第一次同步在读取#50依赖时遇EOF，尚未写#50；新只读快照确认已应用与未应用状态，再只处理#50/#57/#9，没有重复重放已完成写入。最终全量五票回读与48票数量复核PASS，仍35 CLOSED／13 OPEN。见[GitHub机器核对](assets/cpar-channel-delivery-20261010/github-sync-summary.json)、[后置执行索引](assets/cpar-channel-delivery-20261010/closeout-execution-index.json)以及以下唯一评论：[#39](https://github.com/Ricardo121380/cpa-rust-gateway/issues/39#issuecomment-6081202309)、[#49](https://github.com/Ricardo121380/cpa-rust-gateway/issues/49#issuecomment-6081219176)、[#50](https://github.com/Ricardo121380/cpa-rust-gateway/issues/50#issuecomment-6081221061)、[#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、[#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490)。文档静态门槛在终态提交的干净检出上执行，代码／Full来源仍是c1b，报告不触发再次Full。

真实Provider=0；新浏览器行为验收NOT_RUN（UI无改动，已有EgoLite按原来源复用）；remote CI、候选发布制品、push、发布、部署、共享服务重启及生产均NOT_RUN。所有核心票保持OPEN；候选冻结条件尚未达到。

## 剩余项与最小下一步

| 阻断层 | 当前证据与最小下一步 |
| --- | --- |
| Kiro语义／软件 | 当前IDE/CLI原生cap字段、范围、单位、reasoning计数、cap终态及凭据/地区/profile适用性未证明。先取得可追溯当前定义或另行授权原生抓包，再建立公共边界RED并实现等价映射；24历史cap行继续BLOCKED |
| Grok Web语义／软件 | 当前REST/WS方言、可信parent成功推进、并发/取消失效及Messages cap仍未完整绑定。先冻结原生协议证据，再装配现有owner/account/revision/egress原语；文本6行BLOCKED、工具12行已接受不适用 |
| 历史Spec覆盖 | 旧Web/Console各自专属页面回执未定位。取得原回执；若确需该层级证明，在具体页面行为范围补EgoLite场景。该缺口不等于已发现产品缺陷，不机械重开#16 |
| 真实验收 | 186个基础适用行与49项实际扩展声明需要精确账号、环境、模型/声明冻结、请求/货币预算和用户授权。9渠道162行可独立准备，当前actual_declared=null不删除义务 |
| 交付 | 当前源码未push，无对应远端CI/候选发布制品。后续先取得具体上传／候选制作授权和可审查结果，再按部署授权执行；本地Full与合成恢复不替代这些证据 |

E-05不在当前阻断清单中；其批准范围保持严格同授权Grok Build正常刷新，不扩大为手动换凭据或重新授权。
