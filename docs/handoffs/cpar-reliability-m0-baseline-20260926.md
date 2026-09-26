# M0 行为、迁移与验收基线

2026-09-26，实施基线 `213171acb7fdc295c3a258a1d0c79e2d93a73625`，分支 `codex/prism-v4-delivery`。已跟踪工作树干净；原有设计、截图、output、test-results 等未跟踪文件保留。本文固定范围，不表示下列业务已经在本轮通过。

## 参考修订和适配边界

沿用已读源码的固定修订，避免跟随上游 HEAD 漂移：

- CPAMP `e19d8267a52ca146c43bdb86d185bb7baed7ff38`：`apps/web/src/router/MainRoutes.tsx`、账号与提供商任务；[源码](https://github.com/seakee/CPA-Manager-Plus/tree/e19d8267a52ca146c43bdb86d185bb7baed7ff38)。
- CLIProxyAPI `ac02da6c05e18f465aa7e3ed5b0a65a2f060917d`：`internal/api/server.go` 的协议与管理入口；[源码](https://github.com/router-for-me/CLIProxyAPI/tree/ac02da6c05e18f465aa7e3ed5b0a65a2f060917d)。
- 官方 Center `f4b304365142bc9dd151e79539a409b9a05bfa70`：Provider 表单、ModelDiscoveryPanel、MainLayout；[源码](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/tree/f4b304365142bc9dd151e79539a409b9a05bfa70)。本轮网页重取组件失败，使用仓库已有固定修订审查，不声称重新完整审计。

前期证据见 [功能审查](../reports/prism-cpamp-functional-audit-20260911.md)、[模型操作审查](../reports/prism-model-workspace-audit-20260913.md)。其中旧 CPAR 缺陷不能直接当作当前缺陷；本表逐项以当前代码复核。CPA 没有的金额硬预算不纳入；插件/新增渠道不移植。M1 的持久化保证是已确认的 CPAR 要求，不冒称 CPA 的现成行为。

## 八工作区操作台账

HTTP 路径省略共同 `/admin`；页面在 `web/prism/src/features/`，Rust handler 在 `crates/gateway-http-actix/src/`。测试均为后续要运行的定位，不是本轮通过记录。

| 用户操作/参考任务 | CPAR 页面与代码 | 后端接口/符号 | 状态、用例与依赖 |
| --- | --- | --- | --- |
| 登录、改密、锁定 | unlock/UnlockPage.tsx | POST auth/login、auth/password | 已有；M3-03 会话回归；真实登录另验 |
| 仪表盘时间范围→请求/费用 | overview/OverviewPage.tsx、RequestOverview | GET requests/summary、operations/billing、provider-account-pools | 已有；M1 确保请求记录，M3-04 指标深链；metrics 测试 |
| 请求搜索、分页、详情、导出 | monitoring/RequestHistory.tsx、MonitoringPage.tsx | GET requests、requests/{id}/attempts、requests/summary | 需修复后页错误遮挡；M3-02/04，RequestHistory.test.tsx、export.test.ts |
| 用量筛选、账本、价格导入/确认 | usage/UsagePage.tsx、billing/BillingPage.tsx、billingTask.ts | GET operations/usage、billing、billing-processing；POST billing/catalogs；PUT billing/routing-price-policy | 已有；20×100 页前端上限有截断提示；M4 查询成本；历史 allow_partial 不改 |
| 按渠道添加、授权、导入、详情、启停 | accounts/AccountsPage.tsx、AddAccountDialog.tsx、useAccountDirectory.ts | GET accounts/inventory；POST native-accounts/import、upstreams/{id}/account-import、native-account-authorizations | 已有分派；M2 补生命周期，M3-05 批量结果；AddAccountDialog/inventory 测试 |
| 提供商及协议/连接维护 | upstreams/UpstreamsPage.tsx、ProviderDialog.tsx、SubresourcePanel.tsx | upstreams、endpoints、credentials、credential-bindings CRUD | 已有；M3-05 冲突/保存重读；providerResourceTask/subresourceModel 测试 |
| 真实目录→选择开放→多来源/别名 | catalog/UpstreamModelBrowser.tsx、models/ModelsPage.tsx、connectModel.ts | GET catalog/models、models/effective；POST catalog/refresh；public-models/routes/candidates CRUD | 已有；连接不等于开放，发现不扩权；M2-04；浏览器/connectModel 测试 |
| 签发受限 Key、修改权限、停用/撤销 | access/IssueKeyDialog.tsx、KeyPermissionsDialog.tsx、AccessPage.tsx | client-keys、access-groups、access-group-routes | 已有模型/有效期；秘密一次显示；M3-05；keyPermissionSummary 测试；不加预算/RPM |
| 设置、配置生效、诊断、审计 | settings/SettingsPage.tsx、versions、runtime、egress、audit | GET system、runtime/availability、routes/{id}/explain、audit-events；POST config-versions/{id}/validate/publish、operations/runtime/apply | 已有高级子路由；M3-01 错误兜底/M3-05 未保存保护；不把版本操作加回日常接入 |

`src/App.tsx` 与 `src/app/navigation.ts` 保留八工作区、14 条内容路由及解锁页；V2 不改方向。

## 渠道能力矩阵

共同证据：`management_resources/account_channels.rs` 的能力目录、`account_inventory.rs` 的操作投影；授权/导入成功不等于真实推理通过。各渠道完整操作维度如下。

| 渠道 | 首次/重新授权、导入及更新 | 身份、目录、额度 | 启停、刷新、冷却、诊断；差距与用例 |
| --- | --- | --- | --- |
| API/兼容上游（含 Krill、Kimi API、Grok 官方 API） | API Key/文件导入；凭据更新；没有 OAuth 承诺 | 只采用输入/可靠元数据身份；models_path 真实目录；额度按实际能力未知 | CRUD 启停；无通用 token refresh；已有 Explain/失败；M2-01/03/04；Krill 不是原生渠道 |
| Codex/ChatGPT | AuthCode 首次与 replace_existing；OAuth JSON 导入 | 授权身份、Responses；原生目录；权益/额度有来源时间 | Codex refresh worker 已装配；保留禁用状态；M2-01/02/05 核对轮转续接 |
| Claude | AuthCode 首次/替换；OAuth JSON 导入 | 可靠身份；Messages；兼容 models_path；无证据额度不补零 | 启停已有；定时 refresh 装配缺口待 M2；不可把导入当首次授权通过 |
| Kimi Coding | Device 首次/替换；JSON 导入 | 真实身份；Coding 协议/兼容目录；与 Kimi API 分开 | Kimi refresh worker 已装配；M2-01/02/05 故障分类/续接 |
| Kiro | OIDC Device 首次/替换；JSON 导入 | Provider 有模型/订阅类型，未接通通用目录 worker | 启停已有；定时刷新/目录装配待 M2；M2-02/04/05 |
| Grok Build | Device OAuth；原生导入、重新授权 | 原生身份与 Responses；真实 Build 目录和 token worker | 原生启停/刷新、池冷却/诊断；M2-01/02/03/04/05 |
| Grok Console | SSO 导入/替换；无首次 OAuth 承诺 | 探测身份，缺失如实显示；额度类型已有；目录来源有能力限制 | 原生启停；不伪造 refresh token；冷却不得提前重置额度；M2-01/03/04 |
| Grok Web | SSO 导入/替换；无首次 OAuth 承诺 | 身份/额度按证据；不能套 Build 目录 | 原生启停/冷却/诊断；不伪造自动登录；M2-01/03/04 |

定位：`apps/gateway/src/credential_refresh.rs`（Build/Codex/Kimi），`runtime.rs::catalog_refresh_targets` 周边（Build/Codex/兼容 models_path）；`management_resources/{codex_enrollment,kimi_device,kiro_device,native_accounts}.rs`；`gateway-control/src/provider_account_pool_service.rs`。不同身份/授权上下文不凭邮箱合并。

## 生产只读基线（本轮实取）

目标：现有 `new-vps` 的 `cpa-rust-gateway.service`，仅 systemctl/readlink、loopback healthz、SQLite mode=ro + query_only 聚合。无改配置、重启、推理、登录和复制秘密。

- active，PID 1701957；current 指向 `b15439636e3c3c0467d7735f480f4ea7b7cfdba7`，`/healthz` 返回 ok。
- `schema_migrations.max(version)=28`（`PRAGMA user_version=0` 不是本仓库 schema 版本）。
- 原始事件 2,356；账本 699；配置版本 35；全版本普通凭据 33；原生 Grok 账号 5；Client Key 29；访问组路由关联 79。全版本行数不能冒充当前受管身份数；上次受管投影 7 是历史证据。
- 权限非秘密列排序 SHA256：client_keys `724e5aa95f71a840446b9a60d7173f60721484fac679814aaba78f1d90635e34`；access_group_routes `30dd39824400c5fb545c106eaf10136be79184e011c35086e4d84b8b8902643f`。
- billing_materializer_failures 87 是历史失败表总行数，不能叫 87 条待处理；保留此前 11 条隔离/4 组歧义语义。本轮 M1 离线副本再验证。
- 这不是生产登录态或真实渠道验收；M1 不改变线上 schema。未来升级先离线演练 schema29→28 回退，事件新可选关联字段需要兼容处理，不能仅换旧二进制。

## 预先冻结的测量和故障条件

机器 Mac16,10 / 16 GiB / 10 logical CPU / macOS27.0 26A428。SQLite 使用仓库 WAL/外键配置；本地固定目录与单 writer，不让并行构建污染性能计时。

- M1：队列容量 1/2/4 做饱和测试；SQLite busy/full/不可写注入；确认等待最多 2 秒；必需入队失败/超时不能触发上游。正常排空沿用 5 秒上限，不把 abort 记成功。每个逻辑边界用独立进程 kill 后重开；已确认 Request 必须可找到，未确认终态保持 unknown；重放不重复增账。
- M1：稀疏事件不为凑 batch 等待；每次请求按 Request/Attempt/Usage/终态确认，禁止逐 SSE chunk 写入。记录本机同环境 mock 请求耗时五次中位数；退化同时超过10%和50ms需定位，不靠删记录换通过。
- M4 数据冻结：100,000 与 1,000,000 外部请求，固定 seed=20260926，30天均匀分布；20模型、8渠道、100账号、100 Key；10%失败、5%取消、5%历史未知，独立 attempt/usage/ledger 数量可计算。查询窄窗5分钟、24小时、全30天及模型/账号/Key组合；页100，跨页快照。
- M4 先预热一次，再各五次取中位数与最大值；相同数据独立计算精确总数/P50/P95。100万窄窗/单页目标中位数≤250ms、单次≤1s；全历史精确聚合≤5s、峰值额外RSS≤256MiB。页查询 SQL 不随页行数线性增加（≤8条），窄窗有索引计划，不接受先全表拉进内存再筛选。不满足保留失败并优化，不改阈值凑通过。
- M4 调用登记固定0/12：优先 Grok Build 客户端四次（初轮、续接、工具调用、工具结果），其余最多8次用于可用现有渠道冒烟/失败诊断；每次发送前填 exact ID/协议/候选，最多512输出 token；关闭网关及客户端自动重试，结果不明也计数。缺登录不拿其它渠道顶替通过。

M0 出口：本台账、生产只读基线、性能/故障预定义、[M1 CR](../change-requests/CR-20260926-required-event-durability.md)。M2–M4 保持未实施，不以本表代替功能验收。
