# 08 · 管理前端开发计划 — Prism

| 项目 | 值 |
|---|---|
| 状态 | `v0.4 — 后端 P13 收口后的重排;批 A/B 是修复,不是新功能` |
| 日期 | 2026-08-18 制定,2026-08-21 追记执行进度(§3.0) |
| 执行到 | **批 A–D 全部收口(2026-08-21)。** 接线率 87/99,剩余 12 个全部是明确不做的,无待办项 |
| 取代 | v0.3(2026-08-11,并仓当日)。v0.3 的路线图假设后端还在推进,现在 P13-04…P13-10 全部 `DONE`,排序依据变了,差异见 §7 |
| 位置 | `web/prism`,由 `cargo build` 构建并嵌入 |
| 协作边界 | [AGENTS.md](../AGENTS.md) / [CLAUDE.md](../CLAUDE.md);越界留痕见 [cross-boundary-log](cross-boundary-log.md) |
| 设计侧 | [07 · 设计文档](07-management-frontend-design.md);实现决策与踩坑见 `web/prism/DESIGN.md` |

---

## 1. 现状核实(2026-08-18 实测,非引述)

### 1.1 后端(以 docs/06 P13 任务表为准)

| 任务 | 状态 | 对前端的意义 |
|---|---|---|
| P13-04 A/B | `DONE` | `operations/account-pools`(已接)、`operations/usage`(**未接**) |
| P13-05 A/B/C | `DONE` | 计费账本 + 价格目录 + 每请求成本(**全部未接**) |
| P13-06 A/B/C | `DONE` | Provider 池 live state、operator action、失败归因(**全部未接**) |
| P13-07 A/B/C/D | `DONE` | Route Explain 加了必填字段与可选 `provider_id`(**接线已过期**) |
| P13-08 | `DONE` | Channel Pin 诊断(**未接**) |
| P13-09 / P13-10 | `DONE` | 纯数据面,无管理 UI |
| P13-11 A–E4 | `DONE_WITH_BOUNDARY` | compatible 代理池/节点/绑定 CRUD、Provider egress 状态投影(**全部未接**) |
| P13-11 E5 | `DEFERRED_UNAUTHORIZED` | 真网络 canary。**前端不得据此宣称任何"已验证可用"** |
| P13-12 / 13 / 14 | `DEFERRED` | Autoreg、媒体协议、额外 Provider。**不排期** |

**结论:后端侧前端要用的东西全部就位,且已过正式 Delivery Gate。没有一项前端工作在等后端。**

### 1.2 前端接线率

```
契约算子      99
已接线        54  (54.5%)
未接线        45  (45.5%)   ← 其中 3 个是明确不做
```

**这是 08-18 的基线,不是当前值。当前值见 §3.0。**

**怎么数(踩过一次坑,写下来):** 对每个 `operationId`,在 `src/` 非 `generated`
非 `*.test.*` 的文件里找**带引号的字面量** `"opName"`。只匹配单行
`call<T>("op")` 会漏掉跨行调用与 `callText` 这条路径 —— 我因此把 71 报成过 68。
注释里提到算子名通常不带引号,所以引号字面量这个口径既不漏也不多。

### 1.3 生产死代码 —— 本轮最大的单一问题

`src/api/proposed.ts` 是独立仓时期为**提案中的 G3 分析端点**建的通道。它的 `analyticsAvailable()` 定义是:

```ts
function fixturesEnabled(): boolean {
  return import.meta.env.DEV && import.meta.env["VITE_PRISM_FIXTURES"] === "1";
}
export function analyticsAvailable(): boolean { return fixturesEnabled(); }
```

**生产构建里恒为 `false`。** 所有依赖它的界面在真网关上只渲染 "contract pending" 空态。

| 依赖 proposed 的文件 | 行数 |
|---|---|
| `features/usage/`(页面 + model + 单测) | 1716 |
| `features/monitoring/`(页面 + 导出 + 单测) | 503 |
| `api/proposed*.ts` | 393 |
| `components/data/` 仅被用量页消费的六个图表组件 | 867 |
| **合计** | **3479 / 16926 = 20.6%** |

外加 `OverviewPage` 的分析半区、`HealthStrip`、`TokenMixBar` 也走同一分支。

这正是 v0.3 §7 记下的风险条目"为不存在的数据建 UI"**已经发生**的那一次。后端最终实现的是 `operations/usage` + `operations/billing`,形状与提案完全不同。

### 1.4 一处正在失效的接线

`explainRoute` 在 P13-07B/D 之后:

- 响应新增**必填** `price_policy`(nullable,`null` 表示策略关闭)与每候选**必填** `price_evidence`(闭集六值);
- 新增可选查询参数 `provider_id`;
- **多 Provider 路由在不传 `provider_id` 时 fail closed,返回 `provider_scope_required`。**

`RuntimePage.tsx:608` 只传 `requested_model` 与 `protocol`,且全仓 grep 不到 `price_evidence` / `price_policy` / `provider_scope_required` 任何一处渲染。

**即:多 Provider 路由的 Route Explain 面板现在是坏的。** 这不是待开发项,是回归。

---

## 2. 三条硬事实(决定后面所有排期)

计划里的每个取舍都从这三条推出来,先立在前面,免得后面反复解释。

### 2.1 `operations/usage` 没有时间桶

一行 = 一个 `(provider, channel, account, public_model, protocol, client_key, access_group)` 组合在 `[from_ms, to_ms]` 区间内的**聚合**,只带一个 `observed_at_ms`。

**后果:**

- 趋势线不能由一次查询得到。要 N 个点就得发 N 个窗口的查询,前端自己拼时间轴;
- `ZoomBrush` / `Heatmap` 的 dataZoom 语义没有数据支撑。**ECharts 依然不引入** —— v0.3 说"等时间桶到位",现在明确:时间桶不会来,理由消失,结论不变;
- 六个 token 家族各带独立 `confidence`(`{total, confidence}`),**不是一个总数**。UI 必须逐家族显示置信度,不得合并求和后标一个总置信度。

### 2.2 契约里没有延迟,也没有请求成败列表

现有 `MonitoringPage` 的 KPI 行(P50/P95 延迟、成功率、实时事件流)在真网关上**无数据源**。全契约能拿到的只有三样:

| 源 | 有什么 | 没有什么 |
|---|---|---|
| `getObservabilityMetrics`(Prometheus,已接) | 累积计数器、队列/丢弃分级 | 分位数、单请求 |
| `listOperationalBilling` | 每请求账本行:`request_id`、`response_id`、六类 token、成本、五档置信度 | **延迟、成败、错误** |
| `listProviderAccountFailures` | 失败归因:`error_code`(17 值)、`error_scope`(10 值)、`retry_decision`(6 值)、`request_id` | **延迟、成功侧的任何东西** |

**所以监控页是重新设计,不是接线。** 能诚实做出来的是"账本流 + 失败归因流",不是"请求成败监控"。这个差别必须写在页面上,不能靠标题含糊过去。

### 2.3 前端永远不算价

`operations/usage` 的 `cost_confidence` 恒为 `"unpriced"` —— 成本只在 `operations/billing` 侧,由后端按绑定的 catalog 算好。`rate_dominance_v1` 的比较结果也是后端给的闭集六值。

**前端只渲染,不推导。** 不做"未知价格按均价估算"这类补全,`unpriced` 就显示 `unpriced`。

---

## 3. 批次计划

四批,批内可并行,批间有序。总量约 **30 个工作日**。

### 3.0 执行进度(2026-08-21 追记)

分支 `claude/route-candidates`,全部已推送。

| 批次 | 状态 | 提交 |
|---|---|---|
| A1 + A2 · 路由候选 / 校验 / Explain 价格证据 | ✅ | `c4969f7` |
| B1 · 用量分析 | ✅ | `2cb233c` |
| B2 · 请求监控 | ✅ | `cd27ff3` |
| B3 · 计费与价格目录 | ✅ | `04877b3` |
| B4 + B5 · Overview 收口 / 拆除 proposed 通道 + 新门禁 | ✅ | `b522225` |
| C1 · Provider 账号池 | ✅ | `bc79ffc` |
| C2 · Provider egress 三分区 | ✅ | `cc8f198` |
| C3 · Compatible 代理池 / 节点 / 绑定 | ✅ | `a95c5de` |
| D1 · Client Key 编辑 · D2 · Channel Pin · D4 · 绑定核对 · D5 · 质量门 | ✅ | `ef8cdc8` |
| **D3 · 单资源详情抽屉** | ❌ **不做** | 见 §4 与 DESIGN.md §25.1 |
| D6 · i18n **骨架层** | ✅ | `1273838` |
| D6 · i18n **页面正文** | ⬜ **不排期** | 实测体量见下,由使用方决定 |

**接线率 87/99(87.9%)。** 剩余 12 个未接线**全部是明确不做的**,没有一个是待办项:

| 归属 | 个数 |
|---|---|
| D3 单资源 GET(五个原有 + 三个 `getCompatible*`)—— **不做** | 8 |
| `getClientKey` —— **不做**(与上同因) | 1 |
| §4 明确不做(`exportCredential` / `previewRestore` / `restoreBackup`) | 3 |

**这张表就是完成度证明:未接线数 = 明确不做数。** 计划里没有剩下的活。

**唯一开放项**是页面正文英文化(D6 的另一半),它不排期 —— 见批 D 的 D6 条目。

门禁现状:**202 单测 · 76 E2E · `check:full` 绿 · 真网关验证通过**。

**实施中发现、写进代码但计划原文没有的事实**(下一轮接手先读这几条,否则会重踩):

1. **`listOperationalUsage` 不是版本作用域的。** 运营面的版本作用域是**逐算子**的,
   不是整面统一:同在 `/admin/operations/*` 下,account-pools / billing catalogs /
   account failures / egress status **带**版本,而 usage / billing / provider pools /
   request attempts **不带**。判定方式与当前实况写在 `src/api/client.ts`
   的 `declaredHeaderNames` 上方(含一条可直接跑的命令),不再抄一份会漂移的清单。

   **本条原文有两处错,2026-08-24 核实后更正:**
   - 原文说"真网关上直接死在 `unknown config version`"。那个字符串出自
     `src/dev/fixtures.ts:567`,**是 fixture 的报错,不是真网关的** —— 当年挂的是 fixture。
   - 原文的机制也不对。`client.ts` 的守卫是
     `options.versionScoped === true && declared.has("x-config-version")`,所以给一个
     不声明版本头的算子传 `versionScoped: true` **根本不会报错,而是静默空操作** ——
     头不发、没提示。**真正的危险方向恰好相反**:你以为这个读取按版本过滤了,其实没有。
     已在 `send()` 里改成显式抛错(带单测),这个方向从此不会再静默。
2. **`listProviderAccountPools` 不需要版本,`applyProviderAccountPoolAction` 需要。**
   所以运行时页不能在"未选版本"时整页早退,那句话对池表是假的。
3. **action 没有 `If-Match` 是对的** —— 它动运行时不动配置,没有 revision 可守。
4. **"策略未设置"必须认 `404` + `management_resource_not_found` 两者。**
   只认 404 会把 `management_access_denied`(会话已死)误报成"还没配价格策略"。
5. **`sumFamily` 里 `null` 不是 0。** 缺口要抬成 `partialCoverage`,否则少算的钱
   看起来像省下的钱。
6. **billing 的 `summary` 在游标截断之前算好。** 因此 `limit: 1` 就能拿到整窗摘要,
   Overview 的 `BillingGlance` 依赖这条性质。
7. **billing 的 `status` 参数是计价置信度,不是请求成败。**
8. **带 `min`/`max` 的输入,浏览器原生约束校验先于任何自写校验。** 冷却时长的
   越界值根本走不到 `validCooldown`,E2E 断言的是 `validity.rangeUnderflow`。

**一条本轮新记的前端待办(有证据,未做):`versionScoped` / `mutating` 两个开关是冗余的,可以删掉。**
实测契约里 **84/99** 个算子声明 `X-Config-Version`、**45/99** 声明 `If-Match`,
而**两者在声明时全部是 `required: true`,没有一个是可选的**。既然"声明了就必须发",
客户端完全可以从生成客户端推导,不需要调用点再传一个可能写错的布尔。
删掉它们等于删掉整类错误(而不是给它加门禁)。代价是要动六十多个调用点,
属于独立的一次机械重构,不适合塞进收尾。

**一条已记录未修的后端不一致(`docs/cross-boundary-log.md`,标 action required · 低优先级):**
`listProviderAccountPools` 未接线时返回 **500**,而本网关其余所有注入式投影都是 **503**
—— 503 正是面板判定"此部署未启用该投影"的依据。前端不冒充判断,把两种可能都写在
错误块里;后端改成 503 后前端无需改动即自动正确分类。

---

### 批 A · 修复已交付契约的接线(~3 天)· ✅ 已完成 2026-08-18

**为什么排第一:** 后端已经为这两块付过正式 Gate 的成本,前端接线过期让它等于没交付。

| # | 内容 | 算子 | 工作量 | 状态 |
|---|---|---|---|---|
| **A1** | **路由候选与校验** | `createRouteCandidate` `validateRoute` `getRoute` `updateRoute` `deleteRoute` | 2 天 | ✅ |
| A2 | Route Explain 补齐 | `explainRoute`(+`provider_id`) | 1 天 | ✅ 见下方交付说明 |

**实施中修正的三处计划错误**(详见 `web/prism/DESIGN.md` §17):

1. **候选是只增的。** 契约只有 `createRouteCandidate`,没有 list / update / delete。
   原验收标准里的"改候选 → 校验失败 → 改回"不可能,已删除。候选的唯一读取路径是
   `explainRoute`,因此 A1 与 A2 合并实施 —— 否则候选视图建完要立刻按 A2 重写一遍。
2. **`price_evidence` 是七个值不是六个**:`dominant` `equal` `dominated`
   `incomparable` `unpriced` `not_evaluated` `disabled`。
3. **A2 只在 fixture 下验证过。** `explain_route` 第一步是
   `snapshot_for(config_version_id)`,而编译快照只在版本**发布后**存在;草稿上它 503,
   与"本部署未接线"在协议层无法区分。离线部署发布不了,所以真网关上跑不到价格证据渲染。
   已把这一点做成面板文案(草稿上不再甩锅给部署)与 fixture 行为(非 active 版本返回 503)。

**顺序在 2026-08-18 核实后调换过(原为 Explain 在前),依据两条源码证据:**

**第一,面板会把草稿改成验不过的状态,而且自己没有出口。** `ModelsPage` 已接 `createRoute`,但零候选的路由在后端被两条独立路径拒绝:

```rust
// crates/gateway-control/src/management_mutation_service.rs:2074
if active_candidates.is_empty() {
    error_codes.push("route_missing_active_candidate");
}
```

(`route_compiler.rs:1254` 是第二条,发布时的编译路径。)于是在 Prism 里建一条路由 → 草稿 validate 报 `route_missing_active_candidate` → 发布被挡 → **面板里没有任何入口能补上候选**,只能回滚或改用 curl。

**第二,现有提示是错的。** `ModelsPage.tsx:151` 成功后写"候选(Candidate)编辑与 Route Prism 视图等待 G1 契约解锁",但 `createRouteCandidate` 一直在契约里。这条文案把一个前端欠账说成了后端未交付。

相比之下 A2 的失效只影响多 Provider 路由的诊断面板,而那个面板本就要求手输 `route_id`,爆炸半径小得多。

**A1 的一条硬约束(设计前先知道):契约里没有 `listRoutes`。** 全契约唯一能枚举 route_id 的读操作是 `listAccessGroupRoutes`(按访问组逐个查)与运营库存的 `route_ids` 字段,`listPublicModels` / `getPublicModel` 都不含路由。所以:

- 路由选择器用运营库存的 `route_ids`(与 AccessPage 同源),**未绑定的路由不会出现**,这句写在选择器旁边;
- 自由文本输入保留为兜底,不能只给下拉;
- 若认为该由后端补 `listRoutes`,走 `docs/change-requests/`,**不在前端绕过**。

**顺带修掉的既有缺陷:`.sheet-panel` 从来没有 `max-height`。** 面板高过视口就两头被裁,
提交按钮永远够不着。修在共享层(面板封顶 + 表单滚动 + 动作行 sticky),
`CredentialSheet` 也在这条线附近,只是没人撞上。

**A1 验收(已达成):**
- 候选表单覆盖契约全部必填:`id` `endpoint_id` `upstream_model` `credential_scope` `transform_mode`(四值)`enabled` `priority` `weight` `capability_override`;
- `validateRoute` 的 `error_codes` 逐条展示,未知码原样显示;
- 已改掉 `ModelsPage.tsx:151` 那条错误提示,建路由成功后直接引导去加候选;
- 真网关跑通:建路由 → validate 报 `route_missing_active_candidate` → 加候选 → validate 通过。

**A2 验收(fixture 下达成,真网关受限于上述快照约束):**
- 路由含多个 Provider 时可显式指定 Provider;省略时渲染 `provider_scope_required` 专属文案;
- 渲染 `price_policy` 血缘,`null` 显示为"价格策略未启用",不显示为 0 或空;
- 每候选渲染 `price_evidence` 七值之一,配徽章词汇表(色 + 形 + 文字);
- `PROTOCOLS` 补齐三个协议 —— 此前缺 `openai_chat_completions`,Chat Completions 路径在面板里无法解释。

---

### 批 B · 用真数据源换掉 3479 行死代码(~11.5 天)· ✅ 已完成 2026-08-20

**为什么排第二:** 单块价值最大 —— 20.6% 的前端代码在生产里不产生任何像素。而且它每多活一天,后来者就多一分把它误读为"已完成的用量分析"的风险。

| # | 内容 | 算子 | 工作量 |
|---|---|---|---|
| B1 | 用量分析页改造 | `listOperationalUsage` | 4 天 |
| B2 | 请求监控页重设计 | `listOperationalBilling` `listProviderAccountFailures` `listRequestAttempts` | 3 天 |
| B3 | 计费与价格目录页(全新) | `listBillingCatalogs` `importBillingCatalog` `rollbackBillingCatalog` `get/set/clearRoutingPricePolicy` | 3 天 |
| B4 | Overview 分析半区收口 | 复用 B1/B3 摘要 | 1 天 |
| B5 | 删除 `api/proposed*` 与 fixtures 的 analytics 部分 | — | 0.5 天 |

**以下五节要点均已实施**(`2cb233c` `cd27ff3` `04877b3` `b522225`);实施中发现、
计划原文没有的契约事实见 §3.0 第 1、4、5、6、7 条,踩坑记录见 `web/prism/DESIGN.md` §18–§21。

**B1 要点:**
- 主视图是**分组聚合表**(按 §2.1,这是数据的真实形状),不是时间序列;
- 趋势作为**次级视图**,由前端发 N 个窗口查询拼成,并在图上标注"由 N 次区间查询拼接,非服务端时间桶";
- 六类 token 各自带 `confidence` 徽章;
- 保留 `LineChart` / `MultiLineChart` / `RankTable`(拼接后的趋势与排行仍用得上),**删除 `Heatmap` / `ZoomBrush`** —— 无桶数据支撑不了。

**B2 要点:**
- 页面改名与副标题必须说清它是什么:**账本流 + 失败归因**,不是成败监控;
- 两个 tab 各自独立分页(两套 cursor,不合并);
- `listOperationalBilling` 行可下钻到 `listRequestAttempts`(`request_id` 首次在此可用);
- `summary` 的五档记录数(`exact/partial/unknown/unpriced`)直接呈现 —— 这是**计费可信度**指标,比任何自造的"成功率"都实;
- 现有 `export.ts`(JSONL 导出)保留,行形状换成账本行。

**B3 要点:**
- 价格目录导入是**整份提交**(`entries[]` 全量),UI 必须明说这不是增量;
- 六个费率字段单位是 `microunits_per_million`,展示时换算但**输入保持原单位**,避免往返丢精度;
- `setRoutingPricePolicy` 是 `PUT` + `If-Match`;`comparison` 当前闭集只有 `rate_dominance_v1`,做成单选而非自由文本;
- `clearRoutingPricePolicy` 需要明确二次确认 —— 它会让所有候选的 `price_evidence` 变成 `disabled`。

**B5 之后新增门禁**(写进 `web/prism/scripts/check.mjs`):

> `src/features/**` 与 `src/components/**` 不得 import `api/proposed`。

理由:这是 §1.3 那类错误的机械化预防。契约没有的形状,只能走 `docs/change-requests/`,不能再在 `src` 里长出一条影子通道。

---

### 批 C · 运行时与出口(~8 天)

| # | 内容 | 算子 | 工作量 | 状态 |
|---|---|---|---|---|
| C1 | Provider 账号池 live + operator action + 失败归因 | `listProviderAccountPools` `applyProviderAccountPoolAction` `listProviderAccountFailures` | 2.5 天 | ✅ `bc79ffc` |
| C2 | Provider egress 状态三分区 | `listProviderEgressStatus` | 2 天 | ✅ 见 DESIGN.md §23 |
| C3 | Compatible 代理池 / 节点 / 绑定 CRUD | 15 个算子 | 3.5 天 | ✅ 见 DESIGN.md §24 |

**C1 要点**(✅ 已实施 `bc79ffc`;实施中新增的约束见 §3.0 与 `web/prism/DESIGN.md` §22):
- `auth_status`(4 值)与 `runtime_status`(7 值)是**两个独立维度**,不合成一个"健康"值。沿用 `pools.ts` 里已有的判断:`cooling` 是等待,`unauthorized` 是停止,两者色调必须不同;
- `applyProviderAccountPoolAction` 的两个动作(`cool_down` / `request_recovery`)都要显式确认对话框,确认文案写明作用对象是**精确到 account 的**;
- 响应 `202` 的四态(`cooling|probe_scheduled|recovery_required|rejected`)各有文案,`rejected` 不等于失败;
- `409` 陈旧目标 → 重新拉取快照后重试,不静默吞掉。

**C2 要点(P13-11E4 的边界,逐条抄在页面上)**(✅ 已实施;实施中修正的两条见下方与 `web/prism/DESIGN.md` §23):
- 三个域 `egress` / `session` / `clearance` 是 `oneOf` 的三种行,**分区展示,不合并成一张表**;
- **不合成 overall health**;
- **不加任何 action 按钮** —— 这是只读投影;
- **空的 Web / clearance 行只意味着"该来源不存在",不等于健康、可用、新鲜、已测试或可用于生产。** 这句必须在空态里出现;
- 传 exact `X-Config-Version`,保持 opaque cursor 原样,`409` 快照冲突后从头重读。

**实施中修正的两条计划错误:**

1. **"分区展示"不是版式问题,是正确性问题。** 三个域共用一个分页流,一次混读再按 `domain`
   切分,会让一台有 100+ 条 egress 行的部署第一页里一条 session 行都没有 —— 而空态写的是
   "该来源不存在"。改成三次独立读取(各带 `domain=`),空才真的是空。代价是三个快照,
   因此每区标注自己的 `snapshot_id`。
2. **`409` 是两件事不是一件。** `..._cursor_conflict`(运行时快照轮换)从头重读有用;
   `..._config_conflict`(所选版本不是快照来源)从头重读没用,要换版本。给同一句提示会让
   第二种情况下的操作员反复点一个永远不会成功的按钮。

**顺带修掉的共享层缺陷:`client.ts` 对任何 `409` 都弹「配置已被其他会话修改」。**
十个 409 code 里有五个是运行时侧的(游标轮换、动作目标漂移),没有人改过配置。其中三个
**今天就能触发** —— 用量 / 监控 / 计费页都在翻分页。修在 `errors.ts::isRuntimeConflict`
一处,五条路径一起好。

**C3 要点**(✅ 已实施;实施中修正的一条见下方与 `web/prism/DESIGN.md` §24):
- `proxy_endpoint` 是**只写**字段:请求里有,响应里永远没有。表单必须直说"保存后不再回显,修改需重新输入" —— 与 `CredentialInput.secret` 同一类诚实处理;
- 读模型只给 `proxy_configured: boolean`,**不得在浏览器里拼 SOCKS5 地址或构造任何传输请求**;
- 三层实体(pool → node → binding)的绑定入口要吸取子资源 CRUD 那次教训:**新建的 pool 在有 node 之前不出现在下级视图**,入口必须放在面板级而非行级。

**实施中修正的一条计划错误:`proxy_endpoint` 与 `CredentialInput.secret` 是同类但反向。**
两者都只写、都不回显,到此为止相同;但凭据密钥在 PATCH 时**必填**(所以 Account 表单说
"哪怕只想改状态也必须重新输入"),而这里契约明说"省略或 null 保留现有封存值,给字符串才轮换"。
照抄那句话等于让运维重打一个正在正常工作的代理地址。

**另记两条已核实的契约事实:** `proxy_configured` 被后端**硬编码为 true**,是常量而非观测,
界面不能让它读起来像"面板验证过这个代理可用";`target_id` 按 `target_kind` 来自**两个不同的
命名空间**,且后端按整对匹配 —— `direct` + 任意 id 与 `proxy_pool` + 无 id 同样是 400。

**三个 `getCompatible*` 单资源 GET 故意未接:** 三个读模型都已完整(池与绑定的读模型等于
输入模型),逐行再拉一次拿不到新东西。它们属于 D3 的详情抽屉。

---

### 批 D · 收尾(~8 天)

| # | 内容 | 算子 / 范围 | 工作量 |
|---|---|---|---|
| D1 | Client Key 编辑 | `updateClientKey` `getClientKey` | 0.5 天 |
| D2 | Channel Pin 诊断面板 | `executeChannelPin` | 1.5 天 |
| D3 | 单资源详情抽屉 | `getUpstream` `getPublicModel` `getAccessGroup` `getEgressPolicy` `getConfigVersion` | 1 天 |
| D4 | 端点凭据绑定显式列表 | `listEndpointCredentialBindings` | 0.5 天 |
| D5 | 质量门补洞 | 390 窄屏 project;`--ink-3` 跨块继承 | 1 天 |
| D6 | i18n 页面正文 | 全站 | 3.5 天 |

**D1**(✅ 已实施):现在只有签发与吊销两个极端。`status` 闭集三值,补上 `disabled` 与改过期。

**原文错在最后一句** —— "表单要先 `getClientKey` 预填"。`listClientKeys` 返回的 schema 与
`getClientKey` **完全相同**,列表行本身就是完整记录,预读只多一个往返。见 D3。

**实施中发现并已呈现的一条:吊销不是终态。** `update_client_key` 无条件写入 status
(没有迁移检查),而 `revoke_client_key` 明确"retaining its redacted record" —— 哈希还在。
所以把一把 revoked 的 Key 改回 active,**当初发出去的密钥会重新可用**。已在真网关上验证确实成功,
表单在选中那一刻就弹警告。

**D2**(✅ 已实施)**:** 只收集契约里的有界字段(`provider_id` `channel_id` `route_id` `credential_id` `requested_model` `protocol` `mode`),**不提供任意 prompt / body 输入框**。receipt 渲染 `outcome` 三值 + `upstream_sent` + 八值 `stage`。`upstream_sent: false` 与 `outcome: failed` 是不同信息,必须分开显示。

**补两条原文没写的:** 它带 **`If-Match`** —— 不是只读诊断,真的会调用上游、消耗配额;
契约把 `attempt_count` 封顶在 **1**。两句都写在按钮上方。

**D3 —— 不做。** 核对契约后发现**每一个 list 返回的 schema 与它的 get 完全相同**
(`listUpstreams`→`Upstream` 与 `getUpstream`→`Upstream`,五对全部如此;`listClientKeys`
与三个 `getCompatible*` 同理)。列表行本身就是完整记录,再拉一次只多一个往返、不多一个字段。
`getEndpoint` 是唯一真正的反例 —— 契约里没有 `listEndpoints`,运营库存又不含 `base_url` ——
而它早就接了。接这八个会把接线率从 87 推到 95,**但那是为计数器接线**。

**D4**(✅ 已实施):价值不在"再列一次绑定"。面板上已有的绑定表来自运营库存,而那是
**join 驱动**的 —— channel / account / provider 三者都能解析才会出现一行。所以一条指向
已删除凭据的绑定在那张表里**完全看不见**,却仍然会让校验和发布失败。配置侧的回答一对,
差的那条就是卡住发布的那条。

**D5 实施后修正:洞比原文写的更糟。** 原文说"没有窄屏 project"。真正的问题是
`.canvas` 是 `overflow-x: hidden`,所以"文档横向滚动"这个断言**永远不会失败** ——
超宽内容不是滚出去而是**被裁掉**,右侧的操作按钮列直接够不着。改成检测"越界且无可滚动祖先"
之后一次抓出三页(计费 +533px、出口 +211px、运行时 +625px),已修。

**D5 原文:**
- `playwright.config.ts:16` 只有一个 `chromium` project,没有窄屏;
- `check.mjs:79` 的 `--ink-3` 规则只在**声明了 `color: var(--ink-3)` 的同一个块内**查字号,继承来的字号查不到。

**D6 —— 骨架层已做,正文不排期。**

**先量再定:** 计划估 3.5 天,实测 **9,776 个用户可见中文字符 / 约 1,461 个片段 / 32 个文件**。
且长句在 JSX 里被 `<strong>` 与 `<span className="mono">` 切碎 —— 不是"抽出来翻译",
是连结构一起搬。更关键的是,这批文案的精度就是批 A–D 的产出:
「空不等于健康、可用、新鲜、已测试或可用于生产」这种句子**译松一分就变成一个更弱的断言**,
比没有英文更糟。

**已做(骨架层):** 导航、设置、解锁、版本选择,加上**全部十个闭集状态词汇**
(徽章、图例、tooltip 的标签与释义,共 52 条)。

**枚举词汇的英文放在枚举旁边,不放进 pack** —— pack 是扁平键空间,而 `disabled` 在认证轴、
价格证据、egress 域是**三个不同的东西**(`active`/`expired`/`fresh`/`available` 同样碰撞),
一张扁平表会把整个模块存在的意义合并掉。详见 `web/prism/DESIGN.md` §26.2。

**语言开关此前是过度承诺**(「界面文案立即切换」—— 对骨架为真,对每一页正文为假),
现已改成如实说明英文覆盖到哪、以及为什么正文没译。

**未做(页面正文)不排期:** 是否真要一个完整英文界面由使用方定,不由计划表推动。
真要做,建议按页推进、逐句把关,而不是一次性机翻。

---

## 4. 明确不做

| 项 | 算子 | 理由 |
|---|---|---|
| 恢复流程 | `previewRestore` `restoreBackup` | 只能恢复到空库,活面板永远不满足前置条件。以文档指引替代 |
| 凭据导出 | `exportCredential` | 明文凭据出浏览器,顶在"秘密零浏览器存储"硬约束上。**需单独批准,不在本计划内** |
| ECharts | — | §2.1:时间桶不会来,dataZoom 与热力图失去数据支撑 |
| react-hook-form + zod | — | 契约规则分散在 `maxLength`/`enum`/`minimum`,现有手写解析器同样能表达且零运行时体积。**校验重复到痛再引入,不由计划表推动** |
| CSS Modules | — | 玻璃材质跨组件共享大量变量,模块作用域反成阻力 |
| 能力自描述(G7) | — | 不在 P13 清单内,**应视为可能永不到来**。各页继续靠 `503` 反推 |
| P13-12/13/14 相关界面 | — | 后端 `DEFERRED`。不给未定形状排期 |

---

## 5. 完成定义(每一项都适用)

1. 类型检查干净;纯模型分支有单测;用户可见路径有 E2E;
2. `npm --prefix web/prism run check:full` 全绿(含双构建字节一致);
3. **对真网关验证过** —— fixture 不算数,且必须从管理监听器的 `/admin-ui/` 打开;
4. 涉及后端文件的改动已记入 `docs/cross-boundary-log.md` 并带 `Cross-Boundary:` trailer;
5. 设计决策与踩到的坑记入 `web/prism/DESIGN.md`。

**关于第 3 条:** 网关把唯一允许的浏览器 origin 推导为管理监听器自身地址(`apps/gateway/src/deployment.rs::management_origin`)。任何其他来源的写操作一律 `404 management_access_denied`,而 GET 照常成功 —— 极易误判为前端 bug。

```bash
cargo build -p gateway --bin gateway
./target/debug/gateway serve --data-listen 127.0.0.1:18180 \
  --management-listen 127.0.0.1:18181 \
  --state-dir <fresh-dir> --credential-dir <dir>
# 打开 http://127.0.0.1:18181/admin-ui/
```

新构建的二进制**拒绝打开上一次构建创建的数据库**,每轮验证用全新 `--state-dir`。

---

## 6. 风险

| 风险 | 现状 | 缓解 |
|---|---|---|
| 为不存在的数据建 UI | **已发生一次,代价 3479 行**(§1.3) | 批 B 清除;B5 后新增 `check.mjs` 门禁禁止影子通道 |
| 契约变了但接线没跟 | **正在发生**(§1.4 Route Explain) | 批 A 修复;根因是 `sync-contract` 只保证生成物同步,**不保证调用点跟进** —— 生成物变了而调用点没变,类型检查照样过 |
| 前端拖垮后端构建 | 真实存在:`cargo build` 会构建前端 | 前端门禁比后端快得多,提交前跑一次成本极低 |
| 两个工具互相覆盖 | 靠 cross-boundary-log,**完全依赖遵守,没有技术强制** | 开工前先读日志尾部;标 **action required** 的是别人改到你这边的东西 |
| 把 `DEFERRED` / `DEFERRED_UNAUTHORIZED` 读成"待接线" | P13-11E5 真网络 canary 未授权 | §1.1 表已逐条标注;**空的 egress/clearance 行不等于健康**(§C2) |

**关于第二条的补充:** 这次 Route Explain 的失效说明 `sync-contract` + `check.mjs` 的漂移门禁有个盲区 —— 它保证 `contracts/` 与 `src/generated/` 跟契约一致,但**响应体新增必填字段时,调用点不渲染它,一切照过**。目前只能靠读 cross-boundary-log 的 action-required 条目补,没有机械化手段。建议在批 A 完成后评估:是否让 `check.mjs` 对比"契约响应必填字段"与"src 中出现过的字段名",给出**警告级**(非阻断)提示。

---

## 7. 与 v0.3 的差异

| 变化 | 原因 |
|---|---|
| 新增 §1.3 生产死代码核实 | v0.3 把用量页记为"改造",实际是**从未在生产运行过**。量级(20.6%)决定了它必须排在新功能前面 |
| 新增 §1.4 正在失效的接线 | P13-07B/D 之后 Route Explain 对多 Provider 路由 fail closed。这是回归,不是待办 |
| "挂在后端任务下"整节删除 | P13-04…P13-10 全部 `DONE`,**没有一项前端工作在等后端**。这张表已无信息量 |
| 新增 §2 三条硬事实 | v0.3 说"等时间桶到位再引入 ECharts";核实后确认时间桶不存在也不会来。前提没了,结论要重新给理由 |
| 监控页从"接线"改为"重设计" | §2.2:契约无延迟、无成败列表。原设计的 KPI 行没有源 |
| ECharts 从"待触发"改为"明确不做" | 同上,触发条件已确认不会满足 |
| 新增 B5 门禁提案 | 机械化预防 §1.3 那类错误,而不是靠记性 |
| 新增 §6 关于漂移门禁盲区的分析 | 这次失效暴露了现有门禁的边界,值得记下来而不是修完就忘 |

---

## 8. 与 docs/06 的关系

`docs/06-development-plan.md` 是后端锁定主计划(P13 任务表、Delivery Gate、证据链)。本文档**不复制**其内容,只在 §1.1 引用任务状态。

**P13 任务状态以 docs/06 为准,前端不修改其任务表。**
