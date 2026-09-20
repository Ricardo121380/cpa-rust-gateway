# CPAR 本地开发现状与 Goal 对标审计

日期：2026-09-20（Asia/Shanghai）。用途：后续开发准备和里程碑决策。

后续：用户已完成七项讨论决策，见 [剩余开发与前端精修计划](../handoffs/prism-remaining-development-plan-20260920.md)。本报告保持审计时点的代码与验证事实；后续计划落盘不代表问题已修复。

## 1. 结论

**项目已具备可用的 Rust 网关和完整管理页面基础；当前工作主要是把分散、不同代际的交互收敛到统一的操作生命周期。当前 Goal 尚未达到整体完成或发布条件。**

- 当前分支为 `codex/prism-v4-delivery`，HEAD 为 `75715595b381e234564b949ee390cafb4553c701`。相对最后有记录的生产修订 `763b57053ce5e9478d71e77b8766da4fdeead625`，本地领先 10 个提交，其中 9 个涉及后续实现、1 个为交付文档；不能把本地成果视为已上线。
- Goal 工具状态为 `paused`。已有七个专项检查点获得 C2C 批次批准；计费批次尚在工作树，C2C checkpoint 为 `c2c_2f8c / iteration 9 / PLAN_RECEIVED`，没有本批次 APPROVED 记录。
- 基础功能覆盖与本轮交互完成度必须分开：8 个日常工作区、14 个管理入口和解锁页均有代码，但仍存在高级访问组、提供商编辑、出口、监控和配置工作区的未收口交互。
- 本次重新执行前端类型检查、静态契约检查、46 文件/369 项单测，以及两项针对性 Rust 回归，全部通过。没有重新运行全量 E2E、EgoLite 整体验收、完整仓库 Gate 或生产验收。

本报告采用当前代码优先、最新专项计划其次、历史报告按其当时范围解释的原则。方法为代码结构扫描、核心调用链核对、全部直接 Sheet 调用点 AST 统计及定向测试；不是全库逐行安全审计，也不将旧测试数量当作当前整体通过证据。读取了已有 C2C 本地 checkpoint，本次没有向 ChatGPT 发起新的实现或评审轮次。

## 2. 代码结构与实际能力

| 层次 | 当前实现 | 关键代码证据 |
| --- | --- | --- |
| Rust 结构 | 21 个 workspace package，协议、Provider、路由、存储、控制与 HTTP 分层 | [Cargo.toml](../../Cargo.toml)，members 列表 |
| 管理端结构 | React/Vite、TanStack Query、Zustand；14 管理入口与解锁页；将目录/价格/高级维护归入 8 个日常工作区 | [App.tsx](../../web/prism/src/App.tsx):21；[navigation.ts](../../web/prism/src/app/navigation.ts):90 |
| 管理契约 | 权威 OpenAPI 含 113 个 path、152 个 operation；前端副本及生成客户端通过当前静态检查 | [management-v1.json](../openapi/management-v1.json)；[check.mjs](../../web/prism/scripts/check.mjs) |
| 渠道接入 | 命名渠道由后端准备专属目标；Kimi Coding 与 API 分离；普通授权不枚举不相关提供商 | [AddAccountDialog.tsx](../../web/prism/src/features/accounts/AddAccountDialog.tsx):25、92；[account_channels.rs](../../crates/gateway-http-actix/src/management_resources/account_channels.rs):174 |
| 配置操作 | 日常操作可创建工作草稿、修订校验、验证发布；记录保存/应用/不确定结果，避免重复写入 | [configurationTask.ts](../../web/prism/src/features/config-versions/configurationTask.ts):9；[modelTask.ts](../../web/prism/src/features/models/modelTask.ts):79 |
| 会话与版本 | 会话代际、选择代际、单调修订、失效锁定；统一客户端拒绝迟到或失主响应 | [client.ts](../../web/prism/src/api/client.ts):123；[versionStore.ts](../../web/prism/src/features/config-versions/versionStore.ts):27；[sessionStore.ts](../../web/prism/src/session/sessionStore.ts):20 |
| 目录与权限 | 当前实现只限制既有候选的账号可用性，不因发现新模型创建公开模型或继承权限；携带目录过期证据 | [route_snapshot.rs](../../crates/gateway-router/src/route_snapshot.rs):919；[runtime.rs](../../apps/gateway/src/runtime.rs):3738 |
| 请求数据 | 记录请求终态、耗时和首内容延迟；存储按快照、时间、过滤、游标读请求及聚合 | [request_observation.rs](../../crates/gateway-http-actix/src/request_observation.rs):120；[request_history.rs](../../crates/gateway-store/src/control_plane/request_history.rs):124 |
| 计费与维护 | serve 启动并停止计费/TTL worker；账本有筛选下推和整行分页；TTL 维护有界 | [deployment.rs](../../apps/gateway/src/deployment.rs):300；[billing_ledger.rs](../../crates/gateway-store/src/billing_ledger.rs):305、730；[maintenance_worker.rs](../../apps/gateway/src/maintenance_worker.rs):14 |

因此，9 月 9 日审查提出的计费没有进入 serve、运营读取全局上限、目录过期及既有 TTL 无运行 owner，不能再原样列为当前未实施功能。它们已有后续代码和历史专项验收；本次只对请求存储和目录手动开放边界补做了定向回归。

## 3. Goal 与计划成功标准

当前 Goal 是“Prism 弹窗、抽屉、表单、确认和详情工作区深度优化”，成功标准同时包括：

1. 全栏目遵守统一视觉、稳定动作区、校验、焦点和移动端规则。
2. 操作绑定正确账号/资源/版本，保存后重读，冲突与不确定结果不重放写入。
3. 逐批 C2C 批准，真实本地 gateway 与 EgoLite 验收。
4. 三尺寸、主题和辅助偏好覆盖，以及适用的前端、Rust、契约、嵌入门禁。
5. 签名上线既有 Oracle 服务，验证回滚与生产边界，交付英文报告。

可执行专项规范是 [modal-workspace refinement](../design/prism-modal-workspace-refinement-20260916.md)。[总开发计划](../06-development-plan.md)仍为 `v1.316 / 2026-09-03`；[9 月 13 日功能对齐计划](../handoffs/prism-complete-alignment-execution.md)及 9 月 16 日渠道交付已改变若干旧行为。总计划不能直接用作当前前端的唯一任务队列。

## 4. 进度对照

“已完成”以下均限定为对应批次，不代表整个 Goal 已完成。

| 状态 | 工作包 | 已有成果及证据 | 剩余差距 |
| --- | --- | --- | --- |
| 已完成：批次 | 共享 Sheet / 渠道授权 | portal、inert、Tab、返回/丢弃保护、取消优先于离开；提交 `6a0cd12`、`560314d`、`4060ab4`；C2C `c2c_5477` 批准 | 所有旧调用方仍需逐项迁移或证明兼容 |
| 已完成：批次 | 旧 OAuth 重新授权 | 绑定授权尝试、保留挑战、回调校验、不确定结果恢复；`0d3e065`，`c2c_8a4d` 批准 | 不代表真实官方登录已重新验证 |
| 已完成：批次 | Provider 接口/账号工作区 | 子资源编辑、连接、分阶段回执、移动端矩阵；`2536d2a`，`c2c_b63f` 批准 | 顶层提供商编辑/删除仍无统一 footer |
| 已完成：批次 | 账号维护 | 普通/原生账号准确定位、批量逐项结果、替换凭据与运行态入口；`402b1a2`，`c2c_a74e` 批准 | 全应用最终浏览器复验待完成 |
| 已完成：批次 | 模型/目录 | 显式来源、目录分页、快照重验、无变化不创建草稿；`45540d1`，C2C 第 3 轮批准 | 目录其他诊断视图仍需最终核对 |
| 已完成：批次 | 日常密钥/权限/发布 | 受限签发、单次显示、权限隔离、发布回执；`664ccbb`，C2C 第 5 轮批准 | 高级访问组创建/编辑/删除/授权路由仍是旧流程 |
| 已完成：批次 | 高级模型/路由/候选 | 固定修订、精确目标、复杂 capability 保留、丢响应恢复；`7571559`，C2C 第 8 轮批准 | 整体回归及上线待完成 |
| 进行中 | 计费工作区 | 目录导入/恢复/策略/检查器已拆分；保存与发布语义分开；369 单测包含该工作树 | 10 个 tracked 修改及 7 个新计费文件未提交；C2C 第 9 轮待审；完整预览仍有条目可见性缺口 |
| 进行中：部分适配 | 出口策略 | 已使用共享 Sheet、部分 dirty/busy 保护 | Proxy Pool/Node/Binding 与删除流程缺少统一 footer、busy 和结果恢复；专项验收待开展 |
| 进行中：沿用基础实现 | 运行、监控、配置、审计、设置 | 已有真实数据、详情和高级入口；部分面板仍使用兼容适配 | 专项交互收口及测试升级待开展；不能归类为“功能不存在” |
| 未启动：最终收口 | 当前整合版本验收 | 有历史整体验收、当前各批局部验收 | 尚无当前完整版本三尺寸、全状态、性能和全量适用 E2E 通过证据 |
| 未启动：本 Goal 发布 | 签名发布与英文总报告 | 既有发布机制和前次回滚点可复用 | 当前优化尚未统一构建发布；最终报告尚未形成 |

批次状态证据：[设计台账](../design/prism-modal-workspace-refinement-20260916.md):3、142、276、311、345、378；[跨边界记录](../cross-boundary-log.md):2732 起。

## 5. 可复算的量化结果

不把不同工作量的任务等权相加成“项目完成百分比”。以下数字分别衡量入口、代码迁移、测试和交付，不能互相替代。

| 指标 | 当前值 | 含义与限制 |
| --- | --- | --- |
| 日常工作区/管理入口 | 8/8 工作区；14 管理入口＋解锁页 | 入口覆盖 100%；不表示所有内部操作通过 |
| 管理 API 同步 | 152 operations，静态检查通过 | 当前契约、生成物和前端约束一致；不代表 152 个 operation 的真实渠道 E2E 全部通过 |
| 共享浮层动作区 | 65 处直接 Sheet，47 处显式 footer（72.3%），18 处未显式 footer | AST 统计；纯只读面板可不需要 footer，不能将 18 处全部算成缺陷 |
| 台账覆盖 | 34 行 caller；15 行仍含 pending；5 行明确 Compatibility adapter | 部分 pending 已滞后于后续批次批准；审计/设置等也未逐项进入该表，不能据此算可靠完成率 |
| 当前前端逻辑回归 | 46/46 文件、369/369 用例通过 | 本次运行；Vitest 使用 Node 环境，不证明视觉或焦点行为 |
| 可发现浏览器回归 | Chromium 254 用例/41 文件 | 本次只执行 `--list`；没有声称这 254 项全部通过 |
| 当前定向 Rust 回归 | 2/2 通过 | 请求快照/未知历史/100001 样本窄窗；目录手动开放与 exact credential 准入 |
| 计划新鲜度 | 总计划最后更新 9 月 3 日，落后当前 17 天 | 文档滞后，不是工期延期 17 天；仓库未提供可据此计算延期的冻结日期计划 |
| 发布差距 | 最新记录生产 `763b570` → 本地 `7571559`，10 提交＋计费工作树 | 本次未 SSH 核验生产 SHA，生产版本只引用最新交付记录 |

统计方法：解析 workspace members 和 OpenAPI operationId；使用 TypeScript AST 枚举 `web/prism/src/**/*.tsx` 的直接 `Sheet` 调用及 `footer` 属性；Git 历史/状态；Playwright 用例枚举。源码共 81 个 TSX 文件，不能把文件数当功能数。

## 6. 具体缺口、风险和成因

### R1：高优先级——部分写操作仍可在提交中离开，不能保留稳定结果

- `Sheet` 的 `busy` 默认值为 false；只有显式传入后，共享关闭/导航保护才知道不可离开。
- [CompatibleProxyPanel.tsx](../../web/prism/src/features/egress/CompatibleProxyPanel.tsx):132、217、384、847 的写入/删除 Sheet 未传 busy；pending 主要只禁用提交按钮。删除弹窗的 onEscape 直接清空目标，保存后的处理则关闭编辑器、刷新列表，没有与新工作区一致的分阶段回执。
- [AccessPage.tsx](../../web/prism/src/features/access/AccessPage.tsx):130、467、525 和 [VersionsPage.tsx](../../web/prism/src/features/config-versions/VersionsPage.tsx):253 存在同类未迁移入口。
- 这是静态调用链确认的交互风险，本次没有浏览器注入延迟复现。后续须以 held-write 下的 Escape/关闭/Back，以及失败和响应丢失验证收口。
- 成因：共享组件先兼容旧调用，业务调用方未全部迁移；批次标题的覆盖范围大于实际逐动作覆盖范围。

### R2：高优先级——访问组表单允许配置运行时不支持的限额

- [AccessPage.tsx](../../web/prism/src/features/access/AccessPage.tsx):499 提示 `max_concurrency=4 rpm=600`，saveGroup 原样写入 limits。
- [runtime.rs](../../apps/gateway/src/runtime.rs):3339 明确拒绝 active 且 limits 非空的访问组，返回 RuntimeCompositionError::Unavailable。
- [access-groups.spec.ts](../../web/prism/e2e/access-groups.spec.ts):27 只证明 fixture 下保存 `max_concurrency=8 rpm=120`，未覆盖真实运行装配/发布。
- 影响：保存草稿可成功，但界面暗示限额可用，实际发布可能被拒绝。修复应限制前端可编辑能力并解释支持边界；不要由这次 UI Goal 顺便扩展共享配额执行器。

### R3：中优先级——计费“完整核对”仅展示部分输入

- [CatalogImportDialog.tsx](../../web/prism/src/features/billing/CatalogImportDialog.tsx):74 限制前 100 条差异，:75 的“核对本次完整价格”限制前 50 条且无分页/加载更多。
- 提交仍包含完整输入，因此不是丢数据；但管理员无法在确认阶段查看第 51 条之后的价格，未满足完整预览意图。
- 应使用有界分页或逐步展开，并增加 51–512 条输入的首尾可核对验收。现有 513 条拒绝测试不能证明允许范围内的完整核对。

### R4：中优先级——回归资产与现产品语义不一致

- [monitoring.spec.ts](../../web/prism/e2e/monitoring.spec.ts):16 仍断言“不提供延迟和成功率”；当前 [RequestHistory.tsx](../../web/prism/src/features/monitoring/RequestHistory.tsx):92 已呈现真实成功率和 P50/P95，MonitoringPage 默认进入 requests。
- [playwright.config.ts](../../web/prism/playwright.config.ts):35 使用 fixture；narrow 项目只覆盖一个 spec 且为 390×780，不能替代 Goal 要求的真实嵌入应用 390×844 全栏目验收。
- [vite.config.ts](../../web/prism/vite.config.ts):66 的单测环境是 Node；369 单测通过不能用于声明弹窗外观、键盘和响应式全通过。
- 成因：功能新增后，旧版页面测试仍保留；定向绿灯与完整验收没有统一状态索引。

### R5：中优先级——计划、台账和当前实现存在语义/状态漂移

- 总计划 :11 仍描述自动发现模型后加入授权模型；当前 [route_snapshot.rs](../../crates/gateway-router/src/route_snapshot.rs):919 只限制已有图，符合后续“手动开放”决策。
- 设计台账 Access 行仍写第 7 轮 review pending，而后续第 8 轮记录已涵盖对应恢复修复；Provider 批次已批准，但顶层表单 footer 仍 pending。两者应精确拆项，不能简单统改 DONE。
- 台账没有为 Overview、Usage、Audit、Settings 建立完整的工作区状态验收行，容易在收尾时遗漏。
- 成因：总计划、专项执行文件、跨边界日志和 C2C checkpoint 分散维护。应建立一个当前索引，保留历史事实并明确 superseded 关系。

### R6：验证缺口——性能目标尚无本轮闭合证据

- Goal 要求减少重复请求、订阅、昂贵派生和大列表开销；当前有局部 useMemo 和统一读取队列，但没有找到绑定当前整合版本的性能基线与复验报告。
- 当前 [App.tsx](../../web/prism/src/App.tsx) 静态导入页面；这本身不是缺陷。必须先测启动、交互、长列表和请求量，再决定是否优化；四文件构建约束下不能直接增加无人服务的异步 chunk。
- 三个大型维护面分别约为 runtime 14320 行、management_resources 11058 行、fixtures 3173 行，包含测试；这是变更审查成本因素，不是要求本轮全仓拆分的理由。

### 阻塞判断

- **当前没有证据表明剩余本地实现依赖新的账号、生产权限或外部接口。** Goal 暂停与计费复核未完成是调度/交付状态，不能自动解释为技术阻塞。
- 官方同意、二次验证或设备授权可能需要本人完成，属于真实渠道验证边界；不能因此停止不依赖真实登录的 UI、mock 和契约工作。
- 当前工作树有 10 个已跟踪修改、7 个本批新文件及其他无关未跟踪产物。提交必须列明路径，避免将历史脚本、输出或用户设计文件一起加入。

## 7. 下一步可执行清单

| 顺序 | 动作 | 完成标准 |
| --- | --- | --- |
| 1 | 将本报告缺口回填当前专项台账，逐动作拆分 Provider/高级访问组；补 Overview/Usage/Audit/Settings 行；总计划标明后续计划优先及旧目录行为已被替代 | 每项有目标文件、验收方式和明确状态；历史记录保留 |
| 2 | 收口计费：修复完整预览可达性，运行受影响测试，提交 C2C 第 9 轮 EXECUTED，修复阻断项后按路径提交 | 完整目录可逐项核对；保存/发布/不确定结果准确；APPROVED 与提交对应 |
| 3 | 补 Provider 顶层表单及高级访问组遗留交互；限制未支持的 limits；再统一出口 Pool/Node/Binding | 稳定 footer、busy 保护、失效清理、受控重读、响应丢失不重放；真实本地发布边界可验证 |
| 4 | 完成监控、配置差异/版本、审计和设置专项；更新旧监控测试 | 已加载/空/失败/重试/长内容/详情离开一致；真实请求统计与测试口径一致 |
| 5 | 运行当前整合版本的浏览器回归与 EgoLite 三尺寸/主题/辅助/键盘验收，补性能基线 | 逐栏目逐状态有记录；分开标注 fixture、真实 gateway/mock、真实渠道及人工待验 |
| 6 | 完成适用仓库与嵌入门禁，核对生产实际 SHA 后签名发布并保留回滚点；生成英文交付报告 | 本 Goal 全部必需项通过；线上哈希、四资源、CSP、隔离和健康证据齐全后才标 Goal complete |

优先修 R1/R2，再做余下视觉精修；不新增渠道、配额执行器或无关架构重构。仓库没有当前各批次承诺日期和工作量基线，本报告不编造延期天数或预计完成日期。

## 8. 本次验证记录

- `npm --prefix web/prism run type-check`：PASS。
- `npm --prefix web/prism test`：46 files / 369 tests PASS。
- `npm --prefix web/prism run check`：`check-prism-spa: OK`。
- `npm --prefix web/prism run e2e -- --list --project=chromium`：枚举 254 tests / 41 files，仅枚举，没有执行浏览器用例。
- `cargo test -p gateway-store --test request_history`：1 PASS，包含 100001 样本的窄窗及历史未知口径。
- `cargo test -p gateway-router materialized_catalog_preserves_manual_opening_and_exact_credential_eligibility`：1 PASS，178 filtered out。
- `git diff --check`：PASS。

本机输出：`/tmp/cpar-status-audit-{typecheck,unit,check,e2e-list,request-history,manual-catalog}-20260920.log`。这些是当前诊断的定向证据，不能替代完整上线 Gate。本次只新增本报告；未修改应用代码、Goal 状态、C2C checkpoint 或生产环境。
