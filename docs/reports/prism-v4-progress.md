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

## 环境边界

仅本地代码与合成数据。未访问 SSH/生产，未执行真实 Provider 调用。已有未跟踪设计、
计划、报告和辅助脚本均保留，未清理或并入代码批次。
