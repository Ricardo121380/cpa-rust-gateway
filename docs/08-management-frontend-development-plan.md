# 08 · 管理前端开发计划 — Prism

| 项目 | 值 |
|---|---|
| 状态 | `v0.4 — 后端 P13 收口后的重排;批 A/B 是修复,不是新功能` |
| 日期 | 2026-08-18 |
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

### 批 B · 用真数据源换掉 3479 行死代码(~11.5 天)

**为什么排第二:** 单块价值最大 —— 20.6% 的前端代码在生产里不产生任何像素。而且它每多活一天,后来者就多一分把它误读为"已完成的用量分析"的风险。

| # | 内容 | 算子 | 工作量 |
|---|---|---|---|
| B1 | 用量分析页改造 | `listOperationalUsage` | 4 天 |
| B2 | 请求监控页重设计 | `listOperationalBilling` `listProviderAccountFailures` `listRequestAttempts` | 3 天 |
| B3 | 计费与价格目录页(全新) | `listBillingCatalogs` `importBillingCatalog` `rollbackBillingCatalog` `get/set/clearRoutingPricePolicy` | 3 天 |
| B4 | Overview 分析半区收口 | 复用 B1/B3 摘要 | 1 天 |
| B5 | 删除 `api/proposed*` 与 fixtures 的 analytics 部分 | — | 0.5 天 |

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

| # | 内容 | 算子 | 工作量 |
|---|---|---|---|
| C1 | Provider 账号池 live + operator action + 失败归因 | `listProviderAccountPools` `applyProviderAccountPoolAction` `listProviderAccountFailures` | 2.5 天 |
| C2 | Provider egress 状态三分区 | `listProviderEgressStatus` | 2 天 |
| C3 | Compatible 代理池 / 节点 / 绑定 CRUD | 15 个算子 | 3.5 天 |

**C1 要点:**
- `auth_status`(4 值)与 `runtime_status`(7 值)是**两个独立维度**,不合成一个"健康"值。沿用 `pools.ts` 里已有的判断:`cooling` 是等待,`unauthorized` 是停止,两者色调必须不同;
- `applyProviderAccountPoolAction` 的两个动作(`cool_down` / `request_recovery`)都要显式确认对话框,确认文案写明作用对象是**精确到 account 的**;
- 响应 `202` 的四态(`cooling|probe_scheduled|recovery_required|rejected`)各有文案,`rejected` 不等于失败;
- `409` 陈旧目标 → 重新拉取快照后重试,不静默吞掉。

**C2 要点(P13-11E4 的边界,逐条抄在页面上):**
- 三个域 `egress` / `session` / `clearance` 是 `oneOf` 的三种行,**分区展示,不合并成一张表**;
- **不合成 overall health**;
- **不加任何 action 按钮** —— 这是只读投影;
- **空的 Web / clearance 行只意味着"该来源不存在",不等于健康、可用、新鲜、已测试或可用于生产。** 这句必须在空态里出现;
- 传 exact `X-Config-Version`,保持 opaque cursor 原样,`409` 快照冲突后从头重读。

**C3 要点:**
- `proxy_endpoint` 是**只写**字段:请求里有,响应里永远没有。表单必须直说"保存后不再回显,修改需重新输入" —— 与 `CredentialInput.secret` 同一类诚实处理;
- 读模型只给 `proxy_configured: boolean`,**不得在浏览器里拼 SOCKS5 地址或构造任何传输请求**;
- 三层实体(pool → node → binding)的绑定入口要吸取子资源 CRUD 那次教训:**新建的 pool 在有 node 之前不出现在下级视图**,入口必须放在面板级而非行级。

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

**D1:** 现在只有签发与吊销两个极端。`status` 闭集三值(`active|disabled|revoked`),补上 `disabled` 与改过期。注意 `PATCH` 是全量替换,表单要先 `getClientKey` 预填 —— 与子资源 CRUD 里 `getEndpoint` 同一模式。

**D2:** 只收集契约里的有界字段(`provider_id` `channel_id` `route_id` `credential_id` `requested_model` `protocol` `mode`),**不提供任意 prompt / body 输入框**。receipt 渲染 `outcome` 三值 + `upstream_sent` + 八值 `stage`。`upstream_sent: false` 与 `outcome: failed` 是不同信息,必须分开显示。

**D5 两个洞的现状:**
- `playwright.config.ts:16` 只有一个 `chromium` project,没有窄屏;
- `check.mjs:79` 的 `--ink-3` 规则只在**声明了 `color: var(--ink-3)` 的同一个块内**查字号,继承来的字号查不到。

**D6:** `zh.ts` 96 行 / `en.ts` 98 行,框架就位且英文包完整性由类型强制,但页面正文全是硬编码中文。切英文后大部分界面仍是中文。**这是已知缺口,不是已完成项** —— 是否真要英文界面由使用方定,不由计划表推动。

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
