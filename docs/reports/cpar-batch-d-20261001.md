# CPAR D：管理体验与证据交付（2026-10-01）

## 范围与完成边界

本文保留 D 首次交付的实现、历史审查与验证记录。固定起点复核后的问题修复、补充产品验收及最终两轴结论见 [D 评审修复报告](cpar-batch-d-review-fixes-20261001.md)。历史 PARTIAL 审查不改写为最终独立 PASS。

**D / CPAR-42–47（#51–56）：本地实现与本次适用门槛 PASS。**父规格为 [#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)。用户明确授权“授权 Codex 完成本次 D 的前后端”。本次没有远程 push、工单写入、真实 Provider 调用或生产部署。

- 分支：`codex/prism-v4-delivery`；固定比较起点 `ff23f2d8b838b8e5189222d4433acf53c92308e4`。
- 主实现 `26f63c6137652b6c8f2e47fcd05d58721be068b9`；审查修正 `36530ff2df10aa4264c911325352c859309880bc`。
- 最终代码／验收脚本提交 `353c0ec4b9aeaefc11b8b4dd6425b5fe696356b7`；其相对 `36530ff` 只增加等待筛选渲染的一行验收代码，`web/prism` 无差异。
- 已有管理契约、可选 Usage 来源／输入口径、读模型、CAS 与游标足以支撑 D；没有修改权威契约、生成客户端、后端源码或 schema，没有数据迁移。
- 原有用户 `AGENTS.md` 改动以及不相关未跟踪文件保留，未纳入任务提交。前端跨归属修改的精确文件及授权记录在 [cross-boundary log](../cross-boundary-log.md)，对应代码提交含 trailer。

## 实际变化

| 切片 | 用户可见行为与保留边界 |
|---|---|
| #51 / CPAR-42 | 登录失败／会话失效后仍返回原页面和 query；保留 `/overview` 旧入口并统一页名／导航身份。窄屏导航进入工作区，Escape 回到菜单按钮。密码修改回执不因后续登录失败消失。共享读取错误显示原 code/message、上次成功读取时间，重试期间禁用按钮；503 不再推断投影未接线。 |
| #52 / CPAR-43 | 概览、请求、失败及账本复用准确读取反馈；成功快照继续保留。筛选清除／历史导航更新可见输入。请求详情分别展示用量来源、输入缓存口径、数值和计价置信度，保留请求／尝试追溯入口。 |
| #53 / CPAR-44 | 消费 B 已有 `provenance`、`usage_provenance`、`input_accounting`：区分实测／估算／未知／混合来源，旧记录不冒充实测，缺来源保留未知。汇总与 JSONL 保留该证据；费用翻页失败保留已载入来源／summary，并重试同一 opaque cursor。checkpoint 未观测时不以零计算积压；价格目录未读到不称没有生效目录。 |
| #54 / CPAR-45 | 目录只读重读保留有效选择和上次结果；软陈旧说明、硬过期接入门槛继续独立。刷新丢响应先重读，失败核对仍关闭写入门槛。高级访问组授权读取的权限拒绝可见，不等同空授权。模型／路由／Key 边界保持原实现。 |
| #55 / CPAR-46 | 运行投影错误可恢复并保留旧矩阵；Provider 账号池通过 cursor 继续读取，空首片不冒充无账号。池动作、恢复和 Pin 丢响应后先读回，再允许明确的新动作；没有自动重放。恢复回执保留目标身份，目标不再可恢复或消失后仍展示历史结果。出口各来源独立反馈，引用未知时阻止删除；表格保持独立可用键盘进入的滚动区域。 |
| #56 / CPAR-47 | 校验结果不会被后续版本读取失败抹掉，显示“校验前读取修订”，不冒充 CAS 证明。历史差异视图支持初始焦点、Escape、返回触发按钮。审计空首片保留后续读取；预检失败保留有时间的上次成功结果，预检不等于备份工件。系统信息刷新失败保留旧准入状态；运行应用丢响应必须先核对服务状态，读取成功不制造先前应用回执。 |

## 冻结页面与公共外壳

源码注册为 `web/prism/src/App.tsx:23–46`：15 个工作页面组件、1 个登录组件、17 个 path（包含 overview 别名）。这是当前已连接入口的冻结清单，没有增加业务页面或重做信息架构。

| path | 页面组件 | 主入口／高级工作区 |
|---|---|---|
| `/`、`/overview` | OverviewPage | 仪表盘；旧书签继续可达 |
| `/accounts` | AccountsPage | 账号管理 |
| `/oauth` | OAuthPage | OAuth 授权 |
| `/upstreams` | UpstreamsPage | AI 提供商 |
| `/models` | ModelsPage | 模型管理 |
| `/catalog` | CatalogPage | 模型管理 → 上游目录 |
| `/access` | AccessPage | API 密钥 |
| `/monitoring` | MonitoringPage | 请求日志；requests／ledger／failures |
| `/usage` | UsagePage | 用量与费用 |
| `/billing` | BillingPage | 用量工作区 → 价格目录 |
| `/settings` | SettingsPage | 设置 |
| `/egress` | EgressPage | 设置工作区 → 出口策略 |
| `/runtime` | RuntimePage | 设置工作区 → 运行诊断 |
| `/versions` | VersionsPage | 设置工作区 → 配置版本 |
| `/audit` | AuditBackupPage | 设置工作区 → 审计与备份 |
| `/unlock` | UnlockPage | 正常登录、强制首次改密、主动改密 |

公共外壳仍有9个主导航入口，相关高级目的地通过 WorkspaceTabs 接入。全局搜索、窄屏菜单、配置 bootstrap、待应用 dock／生命周期 host，以及两个路由注册的 RouteRecovery 均保留。RouteRecovery 实际注册在 `App.tsx:24,28`，不能只在 AppShell 检索没有命中就说不存在。

## 冻结详情／表单与可达条件

以下为实际 rendered caller 的**静态可达性**，按同一入口家族合并其详情、表单、确认与回执状态；不把未连接 helper 或同名文件算新入口。表中的源码路径统一加 `web/prism/src/` 前缀。此清单不代表每个字段、渠道或表单变体都已在浏览器逐一验证，浏览器覆盖见后面的证据表。

| 路由／家族 | 实际入口与条件 | 源码 caller |
|---|---|---|
| accounts 关联授权／接口连接 | “管理授权”（身份组>1）、普通账号连接列 → Sheet | features/accounts/AccountsPage.tsx:148,165,216,227 |
| accounts/upstreams 已保存账号详情与原生维护 | 普通凭据 → CredentialSheet；原生账号 → NativeAccountDialog，含 SSO／启停／删除 | features/accounts/AccountsPage.tsx:234,238；features/upstreams/SubresourcePanel.tsx:951；features/upstreams/NativeProviderAccounts.tsx:44 |
| accounts 运行账号详情／动作 | `view=runtime`、auth/runtime 过滤；选运行连接 → AccountRuntimePanel、PoolActionSheet；凭据须精确映射 | features/accounts/AccountsPage.tsx:38,43；features/accounts/AccountRuntimePanel.tsx:513,567,579 |
| accounts/upstreams 添加与导入 | `add=account/api-key/import`、“导入账号文件”、“添加账号” → AddAccountDialog | features/accounts/AccountsPage.tsx:171,237；features/upstreams/UpstreamsPage.tsx:282 |
| accounts 模型与客户端权限 | “模型” → AccountModelsDialog；已选 Key 和非空模型 → KeyPermissionsDialog | features/accounts/AccountsPage.tsx:245；features/accounts/AccountModelsDialog.tsx:54,60 |
| accounts 普通凭据维护 | “更多” → “更新凭据” → CredentialUpdateDialog | features/accounts/AccountsPage.tsx:135,243,244 |
| accounts 单份／批量生命周期 | “删除”、“批量管理” → 启用／停用／删除确认与结果；普通或原生 targets | features/accounts/AccountsPage.tsx:180,192,246 |
| oauth/accounts/upstreams 授权 wizard | “授权…”／“重新授权”：Codex/Claude code callback、Kimi/Kiro device、Grok Build device，其余普通 OAuthWizard；渠道条件分支 | features/accounts/OAuthPage.tsx:22,25–28；features/accounts/AccountsPage.tsx:235,239–242；features/accounts/AddAccountDialog.tsx:302,307 |
| upstreams 服务与接口接入 | `add=provider`、“添加提供商”；新增账号内“配置渠道接口”／创建服务／为现有服务加接口 → ProviderDialog InlineWorkspace | features/upstreams/UpstreamsPage.tsx:114,252；features/accounts/AddAccountDialog.tsx:310,321,323 |
| upstreams 提供商详情／编辑／移除 | “更多” → ObjectInspector、draft 编辑／移除确认及 ProviderReceipt | features/upstreams/UpstreamsPage.tsx:271,284,293,294,373 |
| upstreams 接口／绑定工作区 | `upstream_id=…`、“接口与账号” → SubresourcePanel；新建／编辑接口、连接账号、核对绑定 | features/upstreams/UpstreamsPage.tsx:112,269,278；features/upstreams/SubresourcePanel.tsx:902,916,936 |
| upstreams 高级凭据与删除 | “添加原始凭据”、“高级编辑凭据”、删除接口／账号 → AccountSheet／确认 Sheet／结果 | features/upstreams/SubresourcePanel.tsx:604,822,843,856,926,939 |
| catalog 目录批量接入 | 选择有效目录的1–20个模型 → CatalogConnectDialog；过期／错误／分页证据不一致不能接入 | features/catalog/UpstreamModelBrowser.tsx:80,110,115 |
| catalog 手动接入 | “手动批量接入” → models?add=model&from_endpoint；连接／刷新期间不可用 | features/catalog/UpstreamModelBrowser.tsx:103 |
| catalog 目录目标详情 | “目录状态与诊断” → 行详情；endpoint/credential query 可展开，selected 行 → inspector Sheet | features/catalog/CatalogPage.tsx:75,160,186 |
| catalog 客户端有效模型 | “检查客户端可用模型” → EffectiveModels；选择模型 → ObjectInspector | features/catalog/CatalogPage.tsx:74；features/catalog/EffectiveModels.tsx:188,209 |
| models 接入与连接 | “接入模型”、add=model、合法来源参数 → ConnectModelDialog；已有模型“管理连接” → ModelConnectionsDialog | features/models/ModelsPage.tsx:138,158,217,245,246 |
| models 详情与模型维护 | “详情”、更多→编辑模型／管理别名／配置路由／删除模型；对应 target 存在，配置路由只在 draft | features/models/ModelsPage.tsx:219,238,251,254,256,258,260 |
| models 高级路由／候选／协议转换 | “高级路由、候选与别名” → RouteWorkbench、CandidateDialog、RouteDialog、ObjectInspector；候选表单内协议转换为现有选项，无独立 Transform CRUD 页面 | features/models/ModelsPage.tsx:243；features/models/RouteWorkbench.tsx:248,312,337；features/models/CandidateDialog.tsx:78 |
| access Key 创建／详情／权限／吊销 | “创建客户端密钥”、已有 key 详情／编辑／吊销；吊销要求 active 与 context | features/access/AccessPage.tsx:235,278,289,407,419,422 |
| access 高级访问组 | 高级组新建／编辑／删除、按组签发；维护／签发要求 draft，详情要求记录 | features/access/AccessPage.tsx:312,349,402,414,415,417 |
| access 组路由授权 | 组行“路由”／“查看授权路由” → GroupRoutes；授权需 draft、grants 成功且未刷新 | features/access/AccessPage.tsx:107,142,356,381,404 |
| egress 出口策略 CRUD | draft 新建／编辑，已有记录详情；删除另需引用读回成功 → InlineWorkspace／ObjectInspector／Sheet | features/egress/EgressPage.tsx:177,195,317,329,350,358 |
| egress 兼容池／节点／绑定 CRUD | draft 且对应来源已确认时新建／编辑；删除需完整引用来源；PoolSheet/NodeSheet/BindingSheet、结果 InlineWorkspace、删除 Sheet | features/egress/CompatibleProxyPanel.tsx:588,598,609,620,623,637,700,785 |
| runtime/egress Provider 出口观察 | ProviderEgressCard 三域分页；session/clearance 凭据 → CredentialSheet；域独立滚动和来源提示 | features/egress/EgressPage.tsx:22；features/runtime/RuntimePage.tsx:1080,1094,1114,1172 |
| overview 指标／趋势详情 | “更多请求指标”、“趋势数据与时间范围” → RequestOverview、Trend；summary 成功后 | features/overview/OverviewPage.tsx:301；features/monitoring/RequestHistory.tsx:112,155 |
| overview 资源／处理／管道详情 | “资源与处理状态”、进程事件、Token／管道详情；部分要求 metrics.data | features/overview/OverviewPage.tsx:120,142,169,311 |
| overview 启动引导 | 没有 active／资源为零时：提供商→开放模型→创建客户端密钥，沿用现有 query 入口 | features/overview/OverviewPage.tsx:300 |
| usage 筛选／分组／口径 | 时间范围、筛选表单、分组选择、UsageCosts、“聚合维度与观测口径”；有筛选时自动展开 | features/usage/UsagePage.tsx:139,205,233,245,250,362 |
| monitoring 请求／账本／失败筛选 | tab=requests/ledger/failures；RequestHistoryPanel 与 FilterForm；失败归因要求 scope | features/monitoring/MonitoringPage.tsx:507,530,549；features/monitoring/RequestHistory.tsx:190–203 |
| monitoring 请求详情 | 请求行详情 → RequestDetail；用量存在时 Token disclosure，另有标识详情 | features/monitoring/RequestHistory.tsx:164,167,169,207,209 |
| monitoring 尝试与诊断追溯 | request_id → AttemptsSheet／AttemptTimeline；“诊断此绑定” → 带目标 query 的 runtime | features/monitoring/MonitoringPage.tsx:142,289,329,389,457,463 |
| billing 价格详情／条目 | 目录行 → CatalogInspector；条目折叠 → CatalogPricePreview | features/billing/BillingPage.tsx:80,102,104,106 |
| billing 目录导入／复制／恢复 | CatalogImportDialog／CatalogRestoreDialog；要求可编辑、目录已读和数量限制 | features/billing/BillingPage.tsx:84,95,102,107 |
| billing 高级路由价格策略 | 设置／更换目录、清除策略 → PricePolicyDialog；policyKnown，绑定另需已确认生效目录 | features/billing/BillingPage.tsx:88,92,108 |
| versions 变更／历史差异 | review=draft、查看变更／校验应用 → PendingChangesWorkspace；历史差异 → ConfigurationDiff | features/config-versions/VersionsPage.tsx:52–62 |
| versions 草稿接续／创建 | 编辑当前／接续待应用、创建空草稿、resume=id → DraftSelectionDialog／DraftCreationDialog | features/config-versions/VersionsPage.tsx:28,37,46,47,60,65 |
| versions 校验／回滚／发布 | lifecycle host；回滚要求当前 active，校验指定版本；确认、恢复账号审阅及回执沿原约束 | features/config-versions/VersionsPage.tsx:48,61；features/config-versions/ConfigurationLifecycleHost.tsx |
| runtime 可用性／恢复 | 目标 query、“查看全部绑定”；RecoveryCard 只对合格目标开放操作，结果可独立保留 | features/runtime/RuntimePage.tsx:415,463,475,1532,1586,1591 |
| runtime 账号池／目录／出口资源 | “相关资源状态”，account_id 可自动展开；无 scope 只显示 Pool | features/runtime/RuntimePage.tsx:1471,1508,1595 |
| runtime 决策解释与 Pin | “解释”→ExplainCard；“发一次真实请求”→ChannelPinCard；有 scope，受 pending/review 门槛约束，原始 prompt 无入口 | features/runtime/RuntimePage.tsx:675,732,767,1336,1391,1605 |
| audit 生命周期／资源审计／预检 | 行详情 → ObjectInspector；“查看修改记录”需 scope；“源库备份预检” → 预检结果，无恢复上传 | features/audit/AuditBackupPage.tsx:101,113,123,127,138；features/audit/ResourceAudit.tsx:29,102,124 |
| settings 偏好／会话／系统／维护 | 主题、语言、辅助开关、主动改密、SystemInformation、高级维护链接；未知运行应用结果先核对 | features/settings/SettingsPage.tsx:60,67,76,85,90,91；features/settings/SystemInformation.tsx:14 |
| settings 搜索与技术详情 | focus=search 自动展开／聚焦；无匹配有清除恢复；渲染／构建详情 | features/settings/SettingsPage.tsx:22,97,103 |
| unlock 密码表单 | 普通登录、强制首次改密、change-password=1 主动改密；后两者要求已解锁且 changing 条件 | features/unlock/UnlockPage.tsx:22,78,84 |
| 全局 shell 与路由恢复 | 菜单／搜索／bootstrap／待应用 dock/lifecycle；RouteRecovery 的重新打开／回工作区／重新登录 | app/AppShell.tsx:45,53,99,129,137,185,200；App.tsx:24,28；app/RouteRecovery.tsx:25 |

共享 Sheet／InlineWorkspace／ObjectInspector 仍承担各自已接入家族的焦点、关闭、表单及结果容器；D 没有新增整套未接入组件库。旧 advanced 与 query 入口按上表保留。

## 本地验证与产品证据

所有下面的验证结果均已实际执行，不包含真实 Provider 或生产验收。

| 层级 | 状态 | 固定提交、命令／环境与结果 |
|---|---|---|
| 适用前端单测 | **PASS** | `36530ff`，`npm --prefix web/prism test`，exit 0，66 文件／469 项；最终 `353c0ec` 的生产和单测源码 byte-identical，差异仅是验收脚本的一行等待。见 [unit metadata](assets/cpar-batch-d-20261001/frontend-unit.json)。 |
| 类型检查 | **PASS** | `npm --prefix web/prism run type-check` 在实现及审查修正后均通过；最终 Full Management SPA 步再次覆盖类型检查与双构建。 |
| EgoLite 实际产品 UI | **PASS** | `353c0ec`，13 场景／120 检查，exit 0，独立 TaskSpace32，本地 Vite+synthetic management HTTP seam。见 [browser summary](assets/cpar-batch-d-20261001/browser-summary.json)、[完整结果输出](assets/cpar-batch-d-20261001/browser.txt)。 |
| 仓库正式 Full | **PASS** | `353c0ec`，44/44；Rust 1,432 passed／0 failed／12 ignored；Darwin27/arm64，2026-09-30 19:38:55–19:42:39 UTC（北京时间2026-10-01 03:38:55–03:42:39），约224秒。完整命令见下方及 [不可变提交 metadata](assets/cpar-batch-d-20261001/full-summary.json)。 |
| 实际 gateway loopback 回归 | **PASS** | Full 的 Agent multi-turn/stream regression 12 组，通过受保护管理面和实际 gateway 进程，Provider 为自有 loopback TLS mock，`real_provider_calls=0`。decode failure、Usage/账本及来源边界由该层核验；不能等同真实 Provider。 |
| 最终报告静态检查 | **PASS** | docs 7/7、798个Markdown文件、107个契约引用、计划状态、密钥扫描与Git空白检查；见 [docs回执](assets/cpar-batch-d-20261001/docs-check.md)。新增最终回执链接另经doc-links复核。 |
| 本次临时环境收尾 | **PASS** | TaskSpace32 已 finish；只终止核对命令匹配的本次 Vite PID86316，127.0.0.1:5188 不再监听。见 [cleanup receipt](assets/cpar-batch-d-20261001/cleanup.json)。 |

```bash
CARGO_NET_OFFLINE=true CHECK_REPORT_PATH=docs/reports/assets/cpar-batch-d-20261001/full-check.md bash scripts/check.sh full
```

[Full 44步逐项回执](assets/cpar-batch-d-20261001/full-check.md)、[Full 完整输出（仅规范化行尾空白）](assets/cpar-batch-d-20261001/full.txt) 保留执行证据。门槛执行前后 HEAD 均为 `353c0ec4b9aeaefc11b8b4dd6425b5fe696356b7`；后续只有本报告、图片与回执文件的文档提交，不能称该文档提交本身重新执行过 Full。

正式 Full 包含现有源码／依赖政策、单向生成契约检查、158 个管理操作的 SPA 双构建、严格 Clippy、Rust 全量测试、loopback serve 与实际 gateway 回归、密钥扫描和 RustSec audit。它不包含 Prism Vitest 或真实 Provider，因而另行记录上述469项前端单测与120项浏览器检查。

### EgoLite 场景冻结

| 场景 | 检查数 | 已验证范围 |
|---|---:|---|
| [inventory](assets/cpar-batch-d-20261001/inventory.json) | 65 | 16工作path × 1440/375px × light/dark＝64，含 overview 别名；再加812×375横屏/reduced-motion。检查可见内容不越过容器、页宽、主题与浏览器无未捕获错误；允许内部表格滚动。 |
| [shell](assets/cpar-batch-d-20261001/shell.json) | 5 | 错误登录保留原query、overview身份、窄屏导航焦点、Escape回菜单、失效会话返回原入口。 |
| [password](assets/cpar-batch-d-20261001/password.json) | 2 | 强制改密成功事实保留至登录失败后；主动改密入口可达。只使用合成密码。 |
| [usage](assets/cpar-batch-d-20261001/usage.json) | 7 | 费用游标失败保留数据并重试同一cursor；估算来源保留；retry busy；清除表单同步；请求与账本输入缓存口径。 |
| [runtime](assets/cpar-batch-d-20261001/runtime.json) | 5 | 初读失败可恢复、刷新保留矩阵、空首片仍能找到后页账号、未知池写入count1、失败核对继续锁定。 |
| [maintenance](assets/cpar-batch-d-20261001/maintenance.json) | 6 | 审计空首片续读和cursor恢复、预检失败↔成功保留/清除反馈、服务旧准入状态、设置搜索、历史差异返回焦点。 |
| [catalog](assets/cpar-batch-d-20261001/catalog.json) | 5 | 失败/成功只读重读均保留有效选择；刷新未知结果阻止重放；失败读回仍关闭；硬过期禁止接入。 |
| [pin-uncertain](assets/cpar-batch-d-20261001/pin-uncertain.json) | 3 | 保留先前回执，丢响应后blocked；失败读回仍blocked；成功读回不自动增加Pin请求。 |
| [recovery-uncertain](assets/cpar-batch-d-20261001/recovery-uncertain.json) | 6 | 保留排程事实且不冒充恢复；失败读回阻止重提；成功读回无自动mutation；变可用、目标消失、后读失败仍保留结果。 |
| [apply-uncertain](assets/cpar-batch-d-20261001/apply-uncertain.json) | 3 | 运行应用丢响应count1，失败读回继续关闭；成功服务观测可允许新的明确操作，但不制造先前应用回执。 |
| [details](assets/cpar-batch-d-20261001/details.json) | 8 | 请求详情与历史差异两家族，各1440/375px × light/dark；视口、焦点与Escape关闭。 |
| [permissions-egress](assets/cpar-batch-d-20261001/permissions-egress.json) | 2 | 403权限错误与空授权不同；出口依赖未读到的节点数／引用是未知。 |
| [i18n](assets/cpar-batch-d-20261001/i18n.json) | 3 | 经实际设置选择English，汇总来源、请求来源和输入口径翻译；窄屏英文布局保持容器内。 |

验收入口为 [受控 EgoLite 脚本](../../scripts/acceptance/cpar-batch-d.mjs)。实际启动命令为 `VITE_PRISM_FIXTURES=1 npm --prefix web/prism run dev -- --host 127.0.0.1 --port 5188 --strictPort`，在同一个 TaskSpace32 用 `ego-browser nodejs` 逐场景执行该脚本并设置上述固定 commit。验收只在本地；HTTP seam 的请求计数只记录 operation 和 cursor，不记录 body 或凭据。

主线程抽查了截图中的文字、布局与可见关闭控件： [桌面浅色用量](assets/cpar-batch-d-20261001/usage-1440-light.png)、[窄屏深色用量](assets/cpar-batch-d-20261001/usage-375-dark.png)、[窄屏请求详情](assets/cpar-batch-d-20261001/request-detail-375-dark.png)、[窄屏历史差异](assets/cpar-batch-d-20261001/historical-diff-375-dark.png)。其余同家族主题／尺寸图片保留在证据目录。导航、表格滚动区和关闭动作使用公开 UI／键盘控件；没有用坐标或DOM隐藏状态绕过门槛。


前端完整单测已在 `36530ff` 执行：`npm --prefix web/prism test`，exit 0，66 个文件／469 项 PASS。见 [完整单测日志（仅规范化行尾空白）](assets/cpar-batch-d-20261001/frontend-unit.txt)。测试脚本的一行时序修正不改这些生产／单测文件，源码 diff 已确认为空。

测试 seams 沿 #9 的既定边界：纯计算／生命周期逻辑的单测，加生成 ManagementApi HTTP 响应 seam 的 EgoLite 实际 UI。只在本地 fixture 使用合成状态和故障；没有绕过提交控件验证写入，也没有真实收费请求。不存在的观察源、NULL checkpoint、旧数据无 Usage 来源与混合来源保持未知，不能由精确价格反推实测 Usage。

## Standards 审查

独立只读审查 `ff23f2d...26f63c6`：覆盖 PARTIAL（生产及单测 diff 已审，验收脚本后半未完整审阅）。发现 **1 项硬性问题**：UsageEvidence 的新增来源／口径标签没有响应英文切换；依据 DESIGN.md 中英文响应要求。`36530ff` 改用反应式消息包，并让汇总 FamilyCell 使用同一标签包。格式单测和实际 Settings→English→Usage/请求详情回归通过。没有要求额外抽象或风格重构。主线程负责修改与最终验证。

## Spec 审查

独立只读审查同一固定 diff：覆盖 PARTIAL。发现 **2 项 P2**：运行应用丢响应可重提；恢复结果依附可恢复行，资格改变／行消失时会隐藏。两项先在 EgoLite 受控场景复现，再修正：RuntimeApplyNotice 先只读核对且不制造旧请求成功回执；RecoveryCard 保留目标和结果，即使目标变可用／消失或后续读取失败。`36530ff` 的相关回归通过。后端投影完整路径未由本探子再次通读；没有据此发明后端缺口，现有权威字段与本次消费一致。

两轴未合并排序：Standards 1 项、Spec 2 项；已返回的3项均已修正。没有取得独立探子的第二轮最终 verdict，不能把初轮 PARTIAL 写成独立最终 PASS。其余新增验收脚本由主线程检查并实际运行。

## 整体收口与未执行项

D 的交付不能关闭 [#57 / CPAR-48](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57) 或父规格 #9。A/B/C 原证据保持：[A](cpar-batch-a-20260930.md)、[B](cpar-batch-b-20260930.md)、[C](cpar-batch-c-20260930.md)。

| 范围 | 状态与依据 |
|---|---|
| C / 11 渠道 × 三协议及逐项扩展 | **BLOCKED**。沿 C 的已记录剩余项：Kiro 原生输出 hard-cap；Grok Web 原生多轮连续性；部分 Official/Console 原生响应 metadata；逐渠道扩展与新构建真实证据。没有删除例外或改变冻结规格。 |
| 88 stories、F01–F08、T01–T15 的整体关闭 | **BLOCKED**。D 仅交付 CPAR-42–47；已知 C 未完成，不能把全范围收口记为 PASS。 |
| 每一个现存表单／渠道变体的浏览器穷举 | **NOT_RUN**。清单是静态已连接入口；D 实际覆盖基本页面与变更相关状态、共同容器代表项。保留既有 A 的详情/授权/配置生命周期证据，未将它升级为本次新提交的逐变体验收。 |
| Playwright runner / 既有全部 e2e 文件 | **NOT_RUN**，按用户规则使用 EgoLite。相关旧 A 目录下一页文案入口已更新为当前共享反馈；旧 e2e 中 A 之前的 Pool recovery/Pin 必填字段断言不作为本次 CPAR D 的证据。 |
| D 新构建真实 Provider、远端 CI、Linux 发布产物、部署 | **NOT_RUN**。本任务没有运行这些外部操作；本地 Full 不能证明线上已加载 D。 |
| FPS／动效性能测量 | **NOT_RUN**。已核对 reduced-motion preference 和布局，不推定实际帧率。 |

没有 backend/schema 数据修改，回退 D 可使用原有 Git 提交恢复前端代码；部署候选的实际发布、目标运行装配与生产验证仍须独立授权和门槛。本次专用本地 fixture 预览和 EgoLite TaskSpace已关闭。Full另有自有loopback进程／临时数据库并由其脚本收尾；已有网关与线上进程不参与。
