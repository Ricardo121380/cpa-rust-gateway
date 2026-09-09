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

## 环境边界

仅本地代码与合成数据。未访问 SSH/生产，未执行真实 Provider 调用。已有未跟踪设计、
计划、报告和辅助脚本均保留，未清理或并入代码批次。
