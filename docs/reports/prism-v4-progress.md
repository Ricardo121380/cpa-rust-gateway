# Prism V4 实施进度

启动：2026-09-09；分支 `codex/prism-v4-delivery`；原始 HEAD
`1ba4340b56e9f4214c98912f78dc477fdc817ea2`。总 Goal 正在执行，尚未完成。

| 里程碑 | 状态 | 剩余 |
|---|---|---|
| M0 | 已完成 | 无；提交见 git 历史 |
| M1 | 已完成 | 视觉与现有能力接线通过；新增后端功能仍属 M2/M3 |
| M2 | 进行中 | 已核对现有模型授权与配置事务；开始 BE-FE-01/02、B3 |
| M3 | 待实施 | B1/B2/B4、真实账本与运营状态 |
| M4 | 待实施 | 全栏目视觉/E2E、相关后端门禁、本地真实 gateway、最终报告 |

本地提交：M0 `190b57c`；M1 壳体与资源 `986e9a7`、对象详情 `b149ec6`、
动作/读取状态与收尾 `9366493`。均带本轮跨边界记录与 trailer，未推送。

M2 已通过 OpenDesign MCP 新建并迭代 [资源管理补充设计](../handoffs/prism-v4-resource-flows.md)。
已核对后端现有模型授权和 SQLite 配置读取事务；后端实现、最终契约与 API 测试尚未开始。

## M0 已完成

- 权威契约已同步，三个 schema 相关 DTO、账号权益及目录状态消费更新。
- 请求绑定会话 generation 与版本选择 generation，拒绝晚到成功/错误/正文；同版本 revision
  单调前进，支持超出 JS Number 精度的序号；非版本响应不污染所选草稿。
- 会话失效集中锁定、移除秘密、取消请求、清空查询/ mutation 缓存、卸载受保护页面；
  换版本关闭旧表单；配置冲突与运行时冲突保持分离，不重放写入。
- 日常 check 现在直接核对权威 OpenAPI；[新增能力 CR](../change-requests/CR-PRISM-V4-001.md) 已记录。
- 当前类型检查及 245 项单测通过，包含 13 项新增 API 所有权回归；这不替代 M4 真实验收。
- Chromium 定向 E2E：session ownership + provider pools 7/7，最终 session ownership + smoke 8/8。
  权威 SPA 门禁通过（99 operations、CSP、四文件、双构建一致）。

## 受保护的旧功能基线

既有 12 页和主路由以 [旧计划 §3.0](../08-management-frontend-development-plan.md) 及
[正式计划的栏目矩阵](../handoffs/prism-v4-execution-plan.md) 为基线。保留所有现有
`web/prism/e2e/` 用例及操作：版本创建/校验/发布/回滚、上游/端点/凭据/绑定/OAuth、
公开模型/Route/候选创建、访问组授权/Key 签发/启停、价格导入/回退/策略、用量六类 token、
失败/账本分页/请求 attempts、账号恢复/冷却、Explain/Channel Pin、三类出口、审计/备份预检、
外观/语言/锁定。runtime 的 query 深链继续兼容；不能因为新增 accounts/catalog 删除已有子视图。

历史 12 个明确未接操作继续保留原边界：9 个无须重复预读的单资源 GET，加秘密导出、
恢复预览与在线恢复。本轮不以调用率取代功能验收。

## M1 首批实现

- `navigation.ts` 与 `AppShell` 实现 14 项分组导航、当前位置、窄屏可收起菜单、外观切换、
  栏目搜索入口；`#/` 保留，`#/overview` 是兼容入口。
- `tokens.css` 合并 V4 灰阶、Apple 蓝、低饱和环境与四层阴影；保留透镜数学与三面材质。
  `v4.css` 管理实底画布、指标组合和数据面板；`modal.css` 改为实底表单/检查面板，移除旧玻璃弹窗配方。
- 新增 `accounts/AccountsPage`：真实分页池读取、Provider/认证/调度筛选、已加载范围搜索、
  三类详情、权益证据、凭据配置与失败深链；独立于顶栏配置版本的读取语义不变。
- 新增 `catalog/CatalogPage`：四态目录、可选字段与 failure-only 证据、筛选与 target 详情。
  当前没有实现 BE-FE-01 有效模型投影；该项仍必须在 M2 完成。
- 原有 12 页继续使用现有操作；凭据、绑定及请求 attempts 详情采用右侧检查面板。
  编辑与确认居中。其余对象（价格、上游、模型/路由、组/Key、出口等）的 V4 详情仍待补齐。
- 用量、请求与运行诊断的数据口径改为可展开说明；总览不再把空账本断言成没有消费。
- 设置补充内存辅助外观、栏目搜索；深色和系统偏好继续有效。修复 StrictMode 下 Escape
  回收焦点，以及跨版本读取复用无版本 query key 的旧凭据缓存问题。
- 2026-09-09 通过既有 OpenDesign MCP `get_artifact` 再次读回 V4，SHA-256 与本地一致
  (`c60cc5fe67354892277a54061e56a4e59385d395a22c211160ac5cf15b9aae07`)。
  当前会话实施，无内置生成器、外部模型或模型 CLI。
- Chromium 三种尺寸各走通全部 14 个栏目；新账号详情 520px / 手机两侧 12px，
  Missing 目录、权益、失败深链、返回筛选和辅助偏好已验证。逐页截图由 E2E
  生成于临时输出，不将其当作真实 gateway 验收。确切测试结果随本批收尾更新。

本批收尾：类型检查、251 项单测、Chromium 全套 109 项 E2E、权威 SPA 门禁均通过；
窄屏专项 2 项通过。门禁确认 99 个 generated operations、CSP、四文件和双构建一致。
本批构建为 index.html 874 B、main.js 479281 B、vendor.js 141061 B、index.css 65986 B，
合计 687202 B（未压缩；相对审查基线 663932 B 增加 23270 B）。

## M1 对象详情补充批次

上游、公开模型、路由、价格目录及六类费率、访问组、Key、出口策略和审计事件均新增
右侧对象检查面板；编辑仍进入居中表单。统一组件仅接受调用方明确选择的安全字段，
不任意序列化对象、不重新读取已有列表中存在的数据，不接收一次性密钥。
出口策略页也接入现有 Provider egress/session/clearance 三域读取；原运行诊断入口保留。

本批类型检查与 16 项定向 Chromium/窄屏回归通过，包含新增 4 项对象检查流程，覆盖
只读版本、从详情进入编辑、价格六类费率、审计/路由标识与 Key 元数据边界。
所有测试使用合成数据，仍不是 M4 真实网关证明。

## M1 收尾通过

账号详情使用复用的 `PoolActionSheet`，冷却/恢复保留版本要求、精确目标、确认、输入边界与
四态结果；目标快照冲突只重读，不重放，也不误报配置被修改。运行诊断默认聚焦矩阵、
Explain 和 Channel Pin；旧账号/目录/出口入口折叠并按需挂载，旧 account_id 深链仍可达。
常规配置页增加统一读取状态和重试入口；已有结果在后台读取失败时明确标为上次结果。
超长模型 ID 的手机检查面板已经回归，标题和字段可以换行。

最终收尾运行：类型检查、251 项单测、119 项 E2E（Chromium 117 + 390px 专项 2），
以及权威 SPA 门禁通过。长模型名用例最初发现标题被后加载的旧 flex 规则覆盖，修复
选择器优先级后重新运行全套 119 项通过。所有结果仍仅是本地合成数据与真实浏览器证明。
新增组件格式整理后再次构建，并重新运行全部 119 项 E2E，通过；未改变项目依赖或锁文件。

M1 完成不表示总 Goal 完成：M2/M3 后端能力仍未实现，M4 真实管理/数据 listener 与
loopback Provider 未运行。接下来完成 BE-FE-01/02 和 B3，再推进物化、有界查询与 TTL。

## M2 候选持久化基础

已增加 Candidate 更新/删除的 Store 与 ManagementMutationService 方法。更新保留所属
Route，删除保留 Route 与 Access Group grant；数据、revision 和审计在同一事务提交。
新增回归验证可编辑字段持久化、旧 revision、错误 Route、无效 Endpoint 的原子回滚，
以及删除后路由校验报告缺少有效候选。管理 mutation service 本批 9 项测试通过，
`cargo fmt --all` 与 `git diff --check` 通过。

后续 HTTP 批次已接入 `/admin/routes/{route_id}/candidates/{candidate_id}` 的 PATCH/DELETE，
定稿权威契约与 CR 并运行 sync-contract（101 个生成操作）。扩展真实 Actix 测试，验证
鉴权拒绝、缺失 If-Match、成功更新字段、新 ETag、旧 revision 冲突、错误归属、body/path
ID 不一致、独立删除及父 Route 保留。13 项契约测试和路由 HTTP 生命周期回归通过；
前端类型检查及权威四文件/CSP/双构建门禁通过。

BE-FE-02 尚未完成：版本一致的有界枚举及正式前端交互仍待接入。BE-FE-01、B1–B4
与 M4 仍待完成；本批测试不替代真实 gateway 联调或磁盘重启验收。

## M2 完整枚举

M2 完整枚举的存储基础已落地：Route/Candidate/Alias 各自按稳定键执行 SQL keyset
读取，单页 1–200，查询最多 limit+1 行；版本元数据与资源行属于同一 SQLite 事务。
续页要求 revision，版本变化明确拒绝；读取不依赖 Access Group grants，涵盖未绑定草稿，
不加载 Credential/Client Key。全图加载复用原解码逻辑，保持既有编译路径。
新增 206 条资源回归验证三种分页与 revision 冲突；16 项存储回归、10 项管理服务回归、
两 crate 的 lib Clippy（-D warnings）与 diff 检查通过。

HTTP 后续批次已接入三个 GET 枚举接口及资源类型/配置 ID/revision 绑定游标，定稿 CR、
权威契约并同步 104 个生成操作。新增页对象及 RouteListItem 保留旧策略的真实标签；
旧写策略边界不变。真实 Actix 回归验证两页候选不重叠、结束游标、错误游标类型、
旧 revision、未知/重复/超限参数。13 项契约测试、路由生命周期回归、旧策略序列化单测、
HTTP lib Clippy、前端类型检查及权威四文件/CSP/双构建门禁通过。

前端 DTO/fixtures/完整维护交互与磁盘重启和真实 gateway 验收仍待完成，BE-FE-02
仍未标为完成。

## M2 前端枚举接入

正式 RouteWorkbench 已接入 RoutingInventory 的 Route/Candidate/Alias 标签页、分页、
已载入数量与错误后重读。新建空候选路由也能列出，不再依赖运营 inventory。新增 DTO
与 fixture 页响应，修改/新建资源后清理旧分页；删除过期“无枚举接口”提示。
类型检查、6 项定向 Chromium 回归通过，包括新建未绑定路由和候选枚举。候选编辑/删除
UI、Access Group 完整路由选择、旧策略对象详情和大页浏览器冲突回归仍需继续，未据此
宣称 BE-FE-02 或 M4 完成。

## M2 候选维护 UI

候选列表现在提供编辑和删除：居中表单回填所有可变字段，ID 只读，Route 由原对象固定；
删除通过居中确认，保留父 Route。写成功后重读资源分页并校验路由；错误保留表单且不重放。
6 项定向 Chromium 测试通过，覆盖权重/优先级/transform/能力覆盖修改、重开确认持久值、
删除候选后父 Route 保留并校验失败；类型检查与权威 SPA 门禁通过。
访问组路由选择、旧策略详情、大页/冲突 UI 回归与真实 gateway 仍需继续。

## M2 访问组路由选择

Access Group 授权表单已使用完整 Route 分页，与配置资源列表复用同一 hook；提供已加载
数量、加载更多和重新读取，保留 exact-ID 输入。写成功后清理旧分页。11 项定向 Chromium
回归通过，新增从创建未绑定草稿 Route 到授权建议可见及授权成功的流程；夹具拒绝不存在
的 Route 引用。类型检查与 SPA 门禁通过。旧策略详情、大页/冲突 UI 和真实 gateway
验收仍待完成；BE-FE-01、B1–B4 没有移出当前 Goal。

## BE-FE-01 serving snapshot 投影基础

RouteSnapshot 新增 effective_models_for_access_group，复用数据面的 exact 模型枚举和
唯一解析，返回真实 exact ID、公开模型/Route 与硬准入 Candidate provenance。不存在的
访问组和合法但无可见模型的访问组分别表达。Key ID 上下文通过共享的生命周期谓词检查
启用/到期，再解析所属组；不接收秘密，也不把管理查询当作数据面认证。

13 项 route_snapshot 回归、6 项 Client Key 回归及两 crate lib Clippy 通过；验证访问组
隔离、歧义排除、到期/撤销/禁用、发布后的新旧快照语义。此批只有核心模型投影，尚未暴露
HTTP 或接入正式模型目录；目录硬过期仍由后续 B3 补齐，不据此宣称 BE-FE-01 完成。

## BE-FE-01 运行时接入基础

真实 SnapshotManagementRuntimeFacade 已实现 effective_models：固定一次 serving snapshot，
按 Access Group/Key ID 上下文调用核心授权投影，输出 exact ID、公开模型、Route、Candidate、
Endpoint、Upstream、协议与编译目录准入类别。DTO 不携带 URL、Credential、Key secret/digest；
运行时未装配时默认拒绝。空模型列表、上下文缺失和非 serving 配置分别表达。

新增 gateway 运行时回归通过，覆盖缺失上下文、空授权组、缺失 Key 与非 serving 版本；
gateway bin Clippy（-D warnings）和 diff 检查通过。测试初次使用相同版本发布被注册表拒绝，
已改用新版本发布后验证。此批没有暴露 HTTP endpoint，目录快照时间/版本证据及 B3 仍待
补齐；最终有效模型目录与验收未完成。

## BE-FE-01 管理 HTTP 接入

`GET /admin/models/effective` 已接入真实 runtime facade，新增 listEffectiveModels 与三个
闭合 schema，按权威契约生成客户端。只接受一个既有 Group ID 或 Key ID，不接受 secret。
分页默认 100、最大 200，投影指纹绑定上下文和模型/来源内容；跨页变化返回 409。
不发送配置 revision ETag。4 项运行时 HTTP 测试和13 项契约测试通过，覆盖鉴权、互斥
上下文、未知/重复/secret 参数、两页完整读取、上下文切换和投影更新冲突。
正式前端目录、目录 snapshot 时间/版本证据、B3 和真实 gateway 验收仍在当前 Goal 内。

## BE-FE-01 正式目录接入

正式 CatalogPage 已接入 EffectiveModels：既有 Group/Key ID 选择、独立 query 上下文、
分页/重读、无效身份与空态，右侧详情展示 exact ID 与 Candidate/Endpoint/Upstream/协议/
编译目录准入。无秘密输入。来源到诊断链接预填 Route 和 exact 请求模型。
6 项定向 Chromium 测试、类型检查及权威 SPA 门禁通过，验证两组模型隔离、Key 查询、
撤销错误清理和诊断深链。首轮测试误选到了 Channel Pin 的同名字段，改为定位 Explain
“请求模型”后通过，未修改断言目标值。
模型选择到草稿维护的接续、目录 snapshot 观测证据与 B3 硬过期、完整本地 gateway
验收仍待完成；本批浏览器使用合成 fixture，不是 M4 证明。

## B3 目录硬过期：快照与租约第一批

SnapshotCredentialCatalog 现在携带 durable version、observed/stale/expires 时间，真实
publish_durable 将存储中的证据随候选按 Credential 固定到 snapshot。新增显式时间的
allows_credential_at/is_hard_eligible_at；新租约、pin、continuation 和 quota recovery 的
时间参数路径检查硬过期，不在热路径读取 SQLite，也不修改已持有的候选/lease。
旧无时间参数入口对带期限目录拒绝准入；无期限配置保留原行为。

13 项 snapshot 与16 项 credential scheduler 回归、gateway bin Clippy 通过。新增证明：
两个 Credential 分别在100/200过期；99 时可取得租约，释放后100时即便容量空闲仍被拒绝，
无时间入口不能绕过期限。此批没有把 B3 标为完成：有效模型/Explain 的时间视图、刷新完成
时间纠正、真实 gateway 验收及更完整在途场景仍需接续。B1/B2/B4 仍是当前必需工作。

## B3 模型列表与诊断的时间视图

新增 exact 模型枚举/唯一解析/来源投影的显式时间入口，复用同一过滤与歧义判断。
数据面的 SnapshotAuthenticatedClient 固定认证时钟结果（认证只取一次时间），模型列表和
exact 解析使用该时刻；管理 facade 使用查询时刻。Explain 同时排除整个过期候选以及
候选内未列出模型/硬过期的 Credential，增加 CatalogIneligible 原因。

177 项 Router lib 回归通过，随后新增认证边界回归单独通过：99 时模型可见，100 时新
认证模型列表为空/解析 Absent，旧认证对象仍保留99时视图。gateway bin Clippy 与 diff
检查通过。刷新完成时间纠正、目录证据 HTTP/UI 投影和真实在途网关验收仍需完成。

## B3 刷新时钟与目录证据

刷新 worker 不再复用整个 pass 的起始时刻；每个账号准入、网络完成后的成功/失败记录、
最终快照发布分别读取当前时钟。来源投影新增同一 serving snapshot 的逐 Credential
目录版本/observed/stale/expires 和当前目录准入布尔值；模型详情显示这些证据，缺失时
标记“未观测”。权威契约已同步，source 的证据也参与分页投影指纹。

本批通过：已有 catalog status runtime 回归、13 项契约测试、4 项 runtime HTTP 测试、
gateway bin Clippy、前端类型检查、包含 v7/硬过期/准入展示断言的 Chromium 流程及
四文件/CSP/双构建门禁。未用这些测试冒充慢 discovery 的真实 loopback 回归或完整 M4。
B3 仍需最终真实网关/在途验收；计费 B1/B2 与 B4 维护仍为当前待实施项。

## B1 坏记录的持久重试基础

新增 migration 0022 billing_materializer_failures：按 materializer_id/event ordinal 去重，
记录封闭错误类别、首次/最近失败时间、尝试次数和修复时间，不存事件正文或秘密。
重试读取下推 SQL，最多1024条，按最近尝试时间/ordinal 排序；成功修复后保留历史。
这为后续正常 checkpoint 推进与失败记录独立重试提供基础，当前物化器尚未调用新方法。

63 项 gateway-store lib 测试与 Clippy 通过，包含迁移上下行、重启可追踪、失败重试不增加
重复行、有界查询与修复记录保留。初次迁移清单断言因新增表失败，已按实际表名增加
expected entry，未删除断言。物化器坏记录处理、serve worker 与安全状态仍待实施。

## B1 物化器失败隔离与修复

物化器已使用持久失败表：每批新事件和到期失败重试分别受 max_events≤1024 限制，
重试间隔至少30秒。无效链路/时间/计价溢出/坏事件记录封闭原因后可推进正常 checkpoint；
存储故障与全局批量错误仍停止推进。修复成功保留失败历史并从待重试集合移除。
新增 billing-only 宽容源读取保留坏 payload 的 ordinal，不返回原正文；普通读取仍拒绝
坏记录，不放宽原消费者约束。

3 项物化器回归、16 项事件存储回归及 control lib Clippy 通过。新增验证坏 Usage 位于
正常事件之前仍能入账，30秒前不重试，迟到链路补齐后自动修复且再次运行不重复；坏 JSON
不吞掉后续事件、严格读取仍失败。serve worker 与安全状态仍待接入，B1 未完成。

## B1 serve worker 接入

新增 apps/gateway/src/billing_worker.rs，双 listener 绑定成功后由 serve 启动单一 owner；
每批256条新事件，SQLite/计价进入 spawn_blocking，积压按批推进且批间让出执行。
停止时先完成事件日志排空，再通知计费 worker 完成当前/最后有限批次，最多等待30秒；
未消费的持久事件保留 checkpoint 供重启恢复，不要求关机时无界清空历史积压。
serve 未定义账本自动删除时长，因此使用明确无自动过期策略（存储最大期限），没有新增
账本清理。原显式 retention API 保持兼容。

真实临时 SQLite worker 回归、3 项物化器回归及 gateway bin Clippy 通过；验证自动生成
unpriced 账本、停止、重启追上新事件及去重。该测试未经过 HTTP listener/mock Provider，
不是 M4 整体验收。安全处理状态与运营接线仍待完成。

## B1 处理进度的存储投影

新增 materialization_progress，以单条 SQLite statement 同时读取源最高 ordinal、checkpoint、
checkpoint 时间及未修复失败数量，不加载事件正文。源为空为0，未运行 checkpoint 保留 None；
checkpoint 超过源水位时失败关闭。未修复失败独立于追平进度，避免“checkpoint已追平”
掩盖坏记录。6 项账本存储回归及 store lib Clippy 通过。
该投影还需接入 worker 状态与管理 HTTP，不能据此把处理状态界面标为完成。

## B1 worker 状态与管理接口

新增共享 BillingProcessingMonitor 并由 serve 实际注入 worker/管理 state；blocking 批次结束
后发布一致水位与观察时间，读取只复制内存。状态区分 disabled、starting、current、catching_up、
needs_repair、failed、stopped；失败保留上次成功观测，停止失败显式标为 failed。
新增 GET /admin/operations/billing-processing，权威契约同步到106个生成操作，未观测保持 null。

状态机回归、真实 SQLite worker 回归、5项 runtime HTTP 与13项契约测试、gateway Clippy、
权威 SPA 门禁通过。前端展示尚待接入，B2/B4 与真实 HTTP/Provider 本地验收仍未完成。

## B1 正式前端状态展示

计费、总览、用量、请求与失败页共享 ProcessingStatus，展示跨版本消费状态、水位、待修复
数量和上次成功观测；失败保留旧数据标记，null 显示“未观测”，空账本不等于零消费。
默认5秒读取，读取错误后停止轮询并提供手动重试；复用 M0 会话清理。修复计费未选版本
时的提前返回，使处理状态仍可查看。
新增跨页面 Chromium 回归及23项既有计费/用量/失败页回归、类型检查和 SPA 门禁通过。

继续核对发现 BE-FE-01 的待修复点：数据面认证在有 scheduler 时读取 scheduler snapshot，
而管理 snapshot_for 当时仍读 registry；下述 serving 来源修复已消除该差异。
B2/B4、模型到草稿接续、全页面复查与 M4 仍待完成。

## BE-FE-01 serving 来源修复

管理 snapshot_for 已与数据面认证采用相同优先级：有 route scheduler 时读取 scheduler
实际快照，否则读取 registry。这样同版本的 discovery 物化模型/证据也能在管理投影中看到，
不会把配置注册表中不同的内容误认为 serving 内容。
新增回归验证同版本不同内容、不同版本拒绝旧上下文、无 scheduler 回退，并检查实际 Arc
身份；相关2项 management runtime 回归及 gateway Clippy 通过。
B2 有界运营读取、B4 TTL、剩余前端接续和 M4 真实网关仍待完成。

## B2 账本整行批读

SqliteBillingLedger::list_bounded 已改为单条 SELECT 读取完整行，不再先查 ID 后逐行 load_entry；
单行写后回读与列表复用同一解码器，保留 fingerprint、六类 token、置信度与时间字段检查。
新增多行回归验证 (occurred_at_ms, ledger_id) 顺序、分页上限、null/0、未定价和整行一致性。
7 项账本存储回归、3 项物化器回归与 store Clippy 通过。
此批仅消除 N+1；全局100000上限、筛选/snapshot/cursor下推与完整聚合仍待实施，B2未完成。

## B2 账本筛选、snapshot 与游标下推

生产 DeploymentManagementUsageFacade 的账本读取已改用存储 query_page，移除该路径的
全局100000条拒绝。时间、Provider/Channel/Account/model/status、snapshot ledger上界及
keyset位置均作为SQL参数；页面最多100行、LIMIT+1。完整汇总在同一读事务流式读取匹配
记录的两列标量，以常量内存保留全范围计数和checked u64费用总和，null与零不混淆。

8项账本存储回归、11项计费相关control回归及gateway Clippy通过。新增100007条样本：
窄窗6条分3页、完整汇总一致、页间新插入被旧snapshot排除、全范围汇总不受旧上限影响。
用量/失败仍有旧路径，HTTP有界blocking读取也尚需接线，B2未整体完成。

## B2 HTTP 有界 blocking 读取

用量、账本和失败 facade 改为共享 Arc（原 trait 已要求 Send+Sync），读取通过 web::block
执行，共享4个并发名额。permit 随blocking任务释放，取消HTTP不会绕过上限。超额新增
ReadCapacityExceeded → 503 / management_operations_busy，原存储错误语义保持不变。

3项管理运营HTTP回归及HTTP lib Clippy通过；新增慢查询并发测试确认4个任务在执行、取消
一个HTTP后第5个仍被拒绝、独立计费状态接口继续返回200。gateway组合编译通过。
用量/失败的全局历史读取仍待修复，B2尚未整体完成。

## B2 失败查询下推

生产失败读取已改用 failure_events_page/read_failure_feedback_page：SQL先限定Attempt失败、
Provider/Channel/Account与旧页ordinal，再读取最多limit+1条并复用原归因和cursor-fingerprint
校验。源水位与行属于同一SQLite读取事务，结果按原顺序交给投影；不再先读取全局100001条。

新增100006条事件存储回归通过（100005条无关Request + 1条目标失败），验证过滤、空结果
水位和旧页位置；2项failure-feedback回归改为验证真实存储读取并通过，gateway Clippy通过。
用量读取仍需修复，B2尚未整体完成；B4与完整M4仍待实施。

## B2 用量流式关联基础

新增 visit_usage_lineages：在一个读事务中固定事件ordinal上界，关联每个Usage的Request与
最新Attempt，将时间/身份过滤和分组keyset位置交给SQL，按分组键有序回调。完整筛选范围的
observed-through独立计算，不因续页位置缩小；回调可在收集足够分组后停止，不创建全历史
事件向量。缺失/冲突链路失败关闭，保留原有语义。

新增大历史回归通过：100005条无Usage的请求历史不影响两个有效用量链路，分组顺序、
时间过滤、续页全局观察时间、新增事件被旧snapshot排除均核对；孤立Usage被拒绝。
store Clippy通过。当前仍是存储基础，生产用量聚合与HTTP游标快照接线需继续，B2未完成。

## B2 用量生产接线

生产用量已调用流式关联与分组聚合，不再加载全局100001条。只保留当前分组和最多limit个
已完成分组，原六类 token checked 累加与置信度逻辑复用。新opaque cursor固定source ordinal，
旧cursor仍可解码；完整筛选snapshot的观测上界不因分页而改变。权威说明已同步。

聚合与旧实现等价、partial token、页间新增999 token被旧snapshot排除、旧/新cursor往返回归
通过；另3项运营HTTP、13项契约、gateway Clippy及SPA门禁通过。B2三条生产读取路径及
blocking边界已接通，仍需M4真实大样本验收。B4、模型到草稿接续、旧策略详情和全页面复查
仍未完成；费用展示继续按账本和用量各自证据核对。

## 环境边界

仅本地代码与合成数据。未访问 SSH/生产，未执行真实 Provider 调用。已有未跟踪设计、
计划、报告和辅助脚本均保留，未清理或并入代码批次。

## B4 既有 TTL 维护接线

serve 在两个 listener 成功绑定后启动 Stored Response / compaction 维护 worker，
每分钟各最多清除 256 条到期记录；使用独立 SQLite 连接和 blocking 任务，不解密内容，
不扩大既有 30 天 TTL 或账本/事件历史删除策略。停止信号会中断等待，等待当前批次完成，
与计费 worker 并行进行有界关闭。错误只记录固定安全状态。

本批 7 项 stored_response 回归通过，包括独立维护连接、双表限额与保留可解密的有效续接；
新增真实文件 worker 回归通过，验证启动清除过期响应和 compaction、保留未到期记录及停止。
gateway Clippy 通过。B4 运行装配已实现，完整真实 gateway 验收仍在 M4。

## M2 旧策略路由详情

完整路由列表中的 round_robin / priority_failover 现在直接使用已加载记录打开右侧详情，
保留原策略、模型归属、尝试数和超时，并可进入候选列表。避免调用不支持旧策略的 getRoute。
清理诊断页中「没有 listRoutes」的过时提示。新增两种旧策略、键盘关闭回归，7 项路由
Chromium E2E、类型检查和 SPA 双构建门禁通过。模型到草稿接续和完整 M4 仍待完成。

## M2 授权模型到草稿接续

来源详情的「用于草稿候选」携带 exact 模型、Endpoint 与来源版本，切换草稿后保留。
可先创建公开模型/路由，或在已有路由添加候选；仅新增表单使用选择，已有候选编辑保留原值。
清除选择会移除接续参数。写入仍需显式保存并通过当前草稿校验，未自动复制 serving 配置。
新增表单的错误展示也移入表单内。

两项授权模型 E2E（上下文隔离、草稿切换与候选预填/清除）、七项路由 E2E、类型检查与
SPA 双构建门禁通过。新增测试初次草稿 ID/表格定位有误，修正后重跑通过。
真实 gateway 的授权选择→保存/校验/发布→重读/审计仍需 M4 验证，不以 fixture 替代。

## M4 真实 serve 验收基础

新增 `scripts/prism-v4-local-acceptance.py`，使用已构建的真实 gateway 二进制、随机 loopback
端口、独立临时状态与随机合成凭据，经真实同源管理 HTTP 创建配置和路由，不直接写数据库。
进程在 finally 中停止，状态与安全 evidence.json 保留供审查，秘密不输出。

本批重新构建 gateway，并运行脚本通过 7 项：空状态管理读取、同源鉴权创建草稿、持久化
重读、未绑定草稿路由完整枚举、无候选路由校验失败、计费处理状态、正式 `/admin-ui/`
嵌入与 CSP。首次脚本入口路径与 revision token 拼接错误已按当前代码修正后重跑。
证据：`/var/folders/tk/90cjjmks0h1b2l13fry36ccm0000gn/T/prism-v4-acceptance-on7up_9k/evidence.json`。

此脚本当前覆盖真实管理初始化，尚未覆盖 mock Provider 请求、计费结果、发布/重启、全页
浏览器与大样本验收。M4 未完成，继续扩展同一验收工具，不以本批 smoke 代替完整交付。

## M4 管理创建新 Endpoint 的真实发布修复

真实 serve 从空状态运行后，发现启动时固定的 Endpoint 能力表无法识别后续管理创建的
Endpoint，导致整份配置校验/发布返回 409。RouteCompiler 新增可信 Adapter 能力表模式，
按每份配置当前 Endpoint 的 adapter_id 查能力；未知 Adapter 失败关闭。serve 注入当前构建
支持的保守能力表，保留原静态 Endpoint 证据模式给既有调用方。

新增 Endpoint / 未知 Adapter 回归、各版本独立 Adapter 能力回归、15 项 RouteCompiler
回归与 gateway Clippy 通过。真实验收脚本扩展 TLS loopback mock（临时自签 CA 仅由测试
进程 SSL_CERT_FILE 使用，不安装到系统）和上游、绑定、候选 PATCH、Access Group / Key
管理写入，随后真实校验与发布通过。无 discovery 场景显式启用已有 allow_unlisted_model，
目录证据/硬过期另需验收。共 9 项真实管理/嵌入检查通过，mock 请求/账本尚未执行。
证据：`/var/folders/tk/90cjjmks0h1b2l13fry36ccm0000gn/T/prism-v4-acceptance-lx2z1wap/evidence.json`。

当前 runtime 装配文档明确：空启动后发布需重启本地进程以装配凭据池和数据面。接下来需
验证该真实重启路径、有效模型、Provider 请求和计费；不能把已发布误报为数据面已切换。

## M4 真实请求 → unpriced 账本

同一临时配置通过真实管理 API 发布，正常停止后重启 gateway，GET 有效模型包含 exact ID，
数据 listener 的 `/v1/responses` 经 TLS loopback mock 成功返回，后台 worker 自动物化出一条
账本。断言输入 10 / 输出 3 token，其他四类 null，金额 null、confidence=unpriced。
现有验收脚本共 12 项通过；叶证书由临时 CA 签发，CA 仅注入该 gateway 进程。
最新证据：`/var/folders/tk/90cjjmks0h1b2l13fry36ccm0000gn/T/prism-v4-acceptance-l78gups6/evidence.json`，
同目录 `ledger.json` 保留安全账本投影。

修复两处旧 Staging 装配冲突：出口 shape 允许显式 host-sized `127.0.0.1/32` / `::1/128`
例外，仍要求 HTTPS、host/port 白名单、禁重定向，宽 CIDR 仍拒绝；公开模型能力声明交由
已完成的 RouteCompiler 验证，移除「必须为空」的重复拦截。出口 shape 回归、streaming 能力
接受且未支持 vision 编译拒绝回归、gateway Clippy 通过。

仍需处理：管理路由超时接受到 120000ms，而 runtime 旧装配上限 15000ms，前端默认30000ms；
本批合成配置显式用15000ms跑通数据面，这不算解决正式默认值的兼容问题。该冲突保留为
本轮待修复项。另需 priced 账本/重放重启、真实大样本/TTL/目录过期、全页浏览器验收。

## M4 默认路由参数、定价与重启幂等

运行装配的路由边界与权威 RouteInput 对齐：1–16 次尝试、1–120000ms。默认 30000ms
合成路由现已通过真实发布/重启/请求；尝试记录上限同时跟随16，保持有界且不遗漏允许的
重试。新增上下界/default 回归、2 项 attempts 记录回归、Clippy 和 gateway build 通过。
本项解决上一记录的默认30000ms启动冲突。

脚本增加 `--priced`，经真实管理 API 导入价格目录；输入10×1 + 输出3×2 = 16 microunits，
其余 token 维度未观测，账本保留 partial。无价格模式仍为 unpriced/null。两种模式均完成
第二次真实进程重启，等待 billing state=current，账本整页与重启前完全一致，无重复行/金额。
带价14项、无价13项检查通过。状态字段首次写成phase，按权威state修正后两种模式重跑通过。
带价证据：`/var/folders/tk/90cjjmks0h1b2l13fry36ccm0000gn/T/prism-v4-acceptance-fhluiy15/evidence.json`；
无价证据：`/var/folders/tk/90cjjmks0h1b2l13fry36ccm0000gn/T/prism-v4-acceptance-sfla4mqf/evidence.json`。
仍需真实大样本/TTL/目录过期、全页浏览器验收及其余 M4 门禁和交付报告。

## M4 真实大样本与 TTL

验收脚本新增 `--large`，仅向本次创建的临时 SQLite 追加合成历史：账本与事件总量分别
达到 99999、100000、100001、100005。每一档都通过真实 HTTP 断言窄窗账本、用量与按账号
筛选失败记录保持基线结果，全量账本摘要 records 等于完整总量，limit=1 不截断摘要。
受控 Provider 503 经真实数据面形成失败事件，管理失败投影可读取；初次脚本遗漏该端点的
配置版本 header，补齐后整个场景重跑通过。

第二次重启前在临时库放入已过期/未过期 Stored Response 与 compaction 合成行，真实 serve
维护 worker 启动后删除过期行并保留未过期行。这里验证运行接线和到期筛选；有效内容解密
与单批限额由前述存储回归证明，未将 zeroblob 合成行当作有效续接内容。

`python3 scripts/prism-v4-local-acceptance.py --priced --large` 本批17项检查通过。
证据：`/var/folders/tk/90cjjmks0h1b2l13fry36ccm0000gn/T/prism-v4-acceptance-gmly4b41/evidence.json`；
同目录 `large-sample.json` 记录四档样本，`ledger.json` 记录真实请求账本。进程正常停止。
仍需目录硬过期/在途语义、完整浏览器视觉与交互验收、最终仓库门禁和交付报告。
