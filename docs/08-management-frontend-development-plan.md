# 08 · 管理前端开发文档 — Prism 工程实现计划

| 项目 | 值 |
|---|---|
| 状态 | `v0.1 Draft — 与 docs/07 v0.2 配套;采纳需随 H18 等项走 Change Request` |
| 日期 | 2026-07-26 |
| 定位 | [07 · 设计文档](07-management-frontend-design.md)的工程实现侧:技术选型、目录结构、构建与嵌入管线、状态与数据层、测试与验收、任务级分解 |
| 方法论 | 前端实现阶段遵循 impeccable skill(`/impeccable init` 建立 PRODUCT.md/DESIGN.md;每阶段以 `critique`/`audit`/`polish` 作质量门;检测 hook 开启);图表遵循 dataviz 六项验证 |

---

## 1. 技术选型(含依据与否决项)

| 决策 | 选择 | 依据 | 否决的备选 |
|---|---|---|---|
| 框架 | **React 19 + TypeScript(strict)** | CPAMP 同栈验证了本领域全部 UI 形态;生态成熟;打包自托管满足 CSP | Vue(无既有参照资产)、继续 vanilla(表格/图表/表单复杂度已越界) |
| 构建 | **Vite 7+,固定输出文件名,关闭内容哈希** | 产物文件集必须与 `management_ui_resources.rs` 静态路由清单一一对应(C3) | vite-plugin-singlefile(内联脚本违反 CSP 'self' 无内联,C4;CPAMP 用它是因为要嵌进别人的二进制,我们嵌自己的,无此约束) |
| 路由 | **react-router HashRouter** | 嵌入式静态服务无服务端 fallback;与 CPAMP 同因同选 | history 模式(需要 Rust 侧 catch-all 路由,违反静态清单原则) |
| 服务器态 | **TanStack Query v5** | 轮询/失效/重试/visibilitychange 暂停开箱即得;ETag→revision 推进做在统一响应回调 | SWR(功能面窄)、手写(CPAMP 手写了 useMonitoringAnalytics 的节流/中止/快照,全是 Query 内置能力) |
| 客户端态 | **Zustand v5(仅内存,无 persist)** | 会话(Key/CSRF)/版本上下文/UI 偏好;C6 禁一切浏览器存储 → **不引入 persist 中间件**(CPAMP 的混淆持久化明确不抄) | Redux(样板过重) |
| 图表 | **echarts/core 显式注册(Bar/Line/Heatmap + Grid/Tooltip/Legend/DataZoom/VisualMap + Canvas 渲染器)** | v0.1 曾定自绘 SVG;真 CPAMP 的图表面(dataZoom 趋势/热力图/多实体对比)自绘成本不可接受;tree-shaken ECharts 打包自托管,CSP 兼容 | 完整 echarts 包(体积)、Recharts(热力图/dataZoom 弱)、自绘(仅保留火花线/健康条带/占比条这三个微图表自绘) |
| 样式 | **CSS custom properties + CSS Modules(SCSS)** | Token 三层双主题(:root → @media → [data-theme]);无运行时 CSS-in-JS | Tailwind(可复现构建面变大;玻璃/材质语义用原生变量表达更直接) |
| 表单 | **react-hook-form + zod**(zod schema 由 OpenAPI 生成器同源产出) | PATCH 整体替换语义(C11)需要「完整对象编辑」模型;zod 镜像后端校验规则 | 手写校验(v0.1 SPA 的教训:校验缺失导致 400 往返) |
| i18n | 轻量 messages 模块,zh-CN(默认)/ en | CPAMP 四语证明需求真实;两语起步 | i18next(功能超量,可后换) |
| API 层 | **升级 `generate-management-client.mjs`**:除现有 fetch 包装外,追加产出 zod schemas + TanStack Query typed hooks | 保持「生成客户端是唯一通道」不变量(C5),裸 fetch 检查继续生效 | openapi-typescript + 手写 hooks(两套真源) |
| 测试 | **Vitest(纯模型模块)+ Playwright(嵌入产物冒烟)+ axe-core(a11y)** | CPAMP 的「逻辑下沉纯模型 + 千行级测试」模式直接采纳;仓库已有 .playwright-cli | — |
| 包管理 | npm + lockfile 提交 + `.nvmrc` 固定 Node | 可复现构建(C10)的前置 | — |

## 2. 架构

### 2.1 目录结构(feature-first,借 CPAMP 验证过的形态 + 提前拆分纪律)

```text
web/admin-ui/
├── src/
│   ├── app/                    # 壳:AppShell、GlassRail/TopBar/Dock、路由表、Providers
│   ├── generated/              # 生成客户端 + zod schemas + query hooks(只读,CI 校验新鲜度)
│   ├── api/                    # 生成层之上的薄封装:revision 拦截、错误映射、能力探测
│   ├── session/                # 内存会话 store(Key/CSRF/版本上下文)——唯一可触碰秘密的模块
│   ├── design/                 # tokens.css、玻璃 mixin、材质语义、图表主题(双主题色板)
│   ├── components/             # 通用组件(§07 设计文档 §9 清单)
│   │   ├── glass/  ├── data/   # DataTable/RankTable/StatTile/HealthStrip/charts 包装
│   │   └── form/               # schema 驱动表单原语
│   ├── features/               # 每区一目录;**纪律:页面组件 ≤300 行,逻辑进 model.ts**
│   │   ├── overview/  usage-analytics/  monitoring/
│   │   ├── config-versions/  upstreams/  models-routes/
│   │   ├── access/  egress/  runtime/  audit-backup/  settings/
│   │   └── <feature>/{FeaturePage.tsx, model.ts, model.test.ts, components/}
│   ├── i18n/
│   └── utils/                  # 时间范围/URL 过滤器契约/格式化(tabular)
├── PRODUCT.md  DESIGN.md       # impeccable init 产物(FE-0 生成,含反参照:CPAMP 的白卡片通用 admin)
└── vite.config.ts  tsconfig.json  package.json
```

**硬性纪律**(CPAMP 的 3900 行页面是反面教材,其纯模型+重测试是正面教材):图表 option 构造器、表格列定义、过滤器归一化、徽章映射一律进 `model.ts` 纯函数,配套 vitest;JSX 只做装配。ESLint 规则限制 features 内文件行数(warn 300 / error 500)。

### 2.2 数据层

```text
生成客户端(唯一 fetch 通道)
  └─ api/client.ts:注入 X-Management-Key / X-Config-Version / If-Match / CSRF
       ├─ 响应回调:ETag "rev-N" → versionStore.advance(); 409 → 冲突事件总线
       ├─ 错误归一:{error:{code}} → typed AppError(§07 §6 映射)
       └─ TanStack Query hooks(生成):
            ├─ 配置面:staleTime 30s,变更后按资源族 invalidate
            ├─ 观测面:refetchInterval 10s(激活页)/60s(总览),hidden 暂停
            └─ 能力探测:GET /admin/capabilities(G7)一次探测 → featureFlags
                 └─ 503 fail-closed 兜底:无 G7 时按端点族退化探测(缓存 unavailable 原因)
```

- **revision 管线**:`versionStore = {configVersionId, revision, status}`;所有 mutation hook 自动携带 If-Match 并在 onSuccess 推进;409 → 全局冲突条组件 + 自动 refetch 当前资源族,绝不自动重放;
- **时间范围/过滤器 URL 契约**:`utils/timerange.ts` + `utils/filters.ts` 定义序列化(`?from=&to=&model=&credential=&client_key=&status=&stage=`),所有观测页经 `useUrlFilters()` 读写,跨页下钻 = 构造链接,零共享内存态;
- **数据保鲜**:Query 的 `placeholderData: keepPreviousData` 实现过滤切换旧快照;选项下拉用独立 query + 永不清空的合并缓存(CPAMP「稳定选项缓存」的 Query 化)。

### 2.3 会话与秘密(C6/C7)

- `session/` 是唯一持有 `mgmt_`/`csrf_` 值的模块;值存 Zustand 内存 store,模块封装 getter,禁止导出原始值给日志/错误上报;
- reveal-once:签发响应的 `key` 字段进入专用短命 store,sheet 关闭即 `zeroize`(置空 + 触发 GC 无法保证,但引用清除 + 不入任何缓存);Query 的 mutation cache 对该响应配置 `gcTime: 0`;
- 检查脚本(§5.2)继续机械禁止 localStorage/sessionStorage/indexedDB/document.cookie 出现在源码。

### 2.4 开放决策:UI 偏好持久化

CPAMP 每页记忆 tab/过滤/排序/页大小(localStorage),UX 收益显著;C6 现行机械全禁。**v1 决定:零存储,UI 偏好仅存内存(刷新丢失)**;同时在本文档立项后续 CR-FE-PREFS:新增 `src/uiPrefs/` 单一白名单模块(zod schema 限定键与值类型,禁止字符串自由值),检查脚本从「全禁 API」改为「仅 uiPrefs 模块可调用 + schema 审计」。待 FE-2 后按实际痛感决定是否提交。

## 3. 构建与嵌入管线

### 3.1 产物契约

```text
dist/
├── index.html                  # 无内联脚本/样式,CSP meta 与服务端 header 双保险
└── assets/
    ├── main.js  main.css       # 固定名,无内容哈希
    ├── vendor.js               # react+router+query+zustand(固定名)
    ├── charts.js               # echarts/core 按需注册(懒加载 chunk,固定名)
    └── generated/management-client.js
```

- Rust 侧 `management_ui_resources.rs` 的 `include_bytes!` 清单与上表一一对应,新增文件必须同步改清单 + `build.rs` rerun 追踪(C3);
- `charts.js` 懒加载:配置面不付图表体积,观测页首次进入时加载(CSP 'self' 下动态 import 合法)。

### 3.2 可复现构建(C10,FE-0 第一周穿刺项)

已知风险与对策,按顺序执行:

1. 锁定 Node(`.nvmrc`)+ npm lockfile + `npm ci`;
2. Vite 配置:`build.rollupOptions.output` 固定 `entryFileNames/chunkFileNames/assetFileNames`;关闭 sourcemap;`define` 注入的版本号来自 git tag 而非时间戳;
3. esbuild/rollup 的非确定性验证:双构建 `sha256sum dist/**` 对比;若 minifier 并行导致差异,降级 `minify: 'terser'` + `maxWorkers: 1`(接受构建变慢);
4. **仍失败的退路**:保留 v0.1 的 tsc-only 管线,React 以 `react/jsx-runtime` + tsc 编译(无 bundler),`importmap` 静态引用自托管 vendor 文件 —— 设计系统与组件代码不变,牺牲 DX;
5. `scripts/check-management-spa.mjs` 扩展:新产物清单校验、双构建字节一致、CSP 无内联、裸 fetch 禁令、存储 API 禁令、reveal-once 边界函数存在性 —— 全部保留并适配新目录。

### 3.3 CI 门禁(接入 `scripts/check.sh`)

`fast` 档新增:`npm run lint && npm run type-check && npm run test && node scripts/check-management-spa.mjs`;`full` 档追加:双构建一致性、Playwright 冒烟(对嵌入产物经 `gateway serve` 管理监听器)、axe-core 关键页扫描、dataviz 色板验证脚本(design/chart-palette.json 变更时)。

## 4. 关键实现规格

### 4.1 玻璃组件(design/ + components/glass/)

- `tokens.css`:07 文档 §8.2 全量变量,三层覆盖(`:root` → `@media (prefers-color-scheme)` → `[data-theme]`);
- `.glass` 基类 + `data-material="draft|active|archived"` 材质语义变体;`@supports not (backdrop-filter)` 不透明降级;`prefers-reduced-transparency/contrast/motion` 三降级在 tokens 层实现,组件零感知;
- 玻璃面计数守卫:开发模式下 `GlassProvider` 统计挂载的玻璃面,>3 时 console.error(性能预算的机械化);
- Route Prism 的 SVG 折射:独立懒加载模块,`@supports (backdrop-filter: url(#f))` + Chromium UA 双查,失败静默回落 `.glass` 基线。

### 4.2 图表(components/data/charts/)

- `echartsCore.ts` 显式注册(CPAMP 模式);单一 `<ChartView>` 包装 init/setOption(notMerge)/ResizeObserver/dispose/`role="img"` + aria-label;
- `design/chartTheme.ts`:双主题色板对象(07 §8.9 已验证四色 + 状态池 + 热力图单色相阶梯),按 resolvedTheme 选取,禁止 echarts 内置主题;
- 微图表(SparkLine/HealthStrip/TokenMixBar)自绘 SVG,不进 echarts,保证 StatTile 轻量;
- 单轴纪律的机械化:`ChartView` 拒绝含 ≥2 个 `yAxis` 的 option(dev 断言)。

### 4.3 表单(components/form/)

- `SchemaForm`:zod schema → 字段渲染(文本/数字/枚举下拉/开关/chips/KV 编辑器);编辑模式加载完整对象,提交完整 `*Input`(C11);
- 跨引用选择器 `RefSelect`:数据源 = 当前版本图(G1),按类型过滤(egress_policy_id/endpoint_id/credential_id/access_group_id);
- 破坏性操作 `ConfirmDestructive`:必须传入级联清单文案(删上游 → 列端点/凭据/绑定数),两步确认。

### 4.4 观测页数据规格(G3 契约的前端侧)

`POST /admin/analytics` 请求/响应形状(与后端 CR 同步评审,前端为契约共同作者):

```jsonc
// 请求
{ "from_ms": 0, "to_ms": 0, "timezone": "Asia/Shanghai",
  "bucket": "auto|hour|day",
  "filters": { "model": [], "credential_id": [], "client_key_prefix": [],
               "endpoint_id": [], "status": "all|success|failed", "stage": [] },
  "include": { "summary": true, "timeline": ["requests","tokens"],
               "ranks": {"by": "model|credential|client_key", "limit": 10},
               "heatmap": {"metric": "requests"}, "options": true,
               "events_page": {"cursor": null, "limit": 100} } }
// 响应:summary{6 KPI} + timeline[buckets] + ranks[] + heatmap[] + options{} + events{page,cursor}
```

前端约定:per-tab 只请求所需 include;`events_page` 游标分页;全部数字 tabular 格式化;成本字段整体缺席时 UI 不渲染成本列(G9 语义)。

## 5. 测试与验收

### 5.1 分层

| 层 | 工具 | 覆盖 |
|---|---|---|
| 纯模型 | Vitest | 每个 feature 的 model.ts:过滤器归一化、图表 option 构造、徽章映射、时间桶、revision 状态机(409 路径)、URL 契约 round-trip |
| 组件 | Vitest + Testing Library | SchemaForm 校验镜像、RevealOnceSheet 生命周期(关闭后不可再取)、ConfirmDestructive 两步、空态三分(空/过滤空/不可用) |
| 集成冒烟 | Playwright(对 `gateway serve` 嵌入产物) | 解锁→只读浏览→草稿 CRUD→验证→发布→回滚;观测页时间范围/下钻链接;双主题截图对比 |
| a11y | axe-core + 手动 | 三降级开关;键盘全通路;reveal-once 焦点陷阱;玻璃上文字最坏对比 |
| 性能 | Playwright + CDP throttling | 6× CPU 滚动帧率;观测页 10 万事件模拟数据 P95 交互延迟;玻璃面计数断言 |
| 设计 | impeccable hooks + `/impeccable audit·critique·polish` | 每阶段出口;检测器 60 规则自动跑于 UI 文件编辑后 |

### 5.2 安全不变量(机械校验清单,继承并扩展 check 脚本)

裸 fetch 禁令(生成客户端唯一通道)/存储 API 禁令(uiPrefs CR 批准前全禁)/CSP 无内联/reveal-once 边界函数存在/凭据 secret 不回显断言/备份工件不读取/双构建字节一致/产物清单与 Rust 嵌入清单一致。

## 6. 任务分解(与 07 §11 阶段对应)

依赖标记:`[CR:Gx]` = 需该契约缺口先行。已按可并行泳道组织。

### FE-0 基座(约 2 周,含穿刺)

| # | 任务 | 泳道 | 验收 |
|---|---|---|---|
| 0.1 | 提交契约 CR 包:G1 全图 + G7 capabilities(+G2/G3 观测同批立项) | 契约 | CR 文档评审通过 |
| 0.2 | 可复现构建穿刺(§3.2 步骤 1-4) | 构建 | 双构建 sha256 一致,或触发退路决策 |
| 0.3 | 新目录脚手架 + 生成器升级(zod+hooks)+ check 脚本适配 | 构建 | CI fast 档绿 |
| 0.4 | `/impeccable init`:写 PRODUCT.md/DESIGN.md(反参照:CPAMP 白卡片 admin;世界:07 §8 契约) | 设计 | 文件评审 |
| 0.5 | design/tokens + glass 基类 + 三降级 + 材质语义 + 图表主题 | 设计 | Storybook 式样张页;a11y 开关演示 |
| 0.6 | AppShell(三面玻璃)+ 路由骨架 + 会话/解锁页 + 错误映射 | 应用 | 解锁→空壳可导航;404 语义正确 |
| 0.7 | 版本上下文(versionStore + VersionPicker + 只读模式)[CR:G1] | 应用 | active 图只读浏览全通 |
| 0.8 | Rust 侧:嵌入清单更新 + capabilities 端点实现 [CR:G7] | 后端 | 探测 hook 返回真实特性位 |

### FE-1 配置面(约 3 周)

| # | 任务 | 验收 |
|---|---|---|
| 1.1 | DataTable/SchemaForm/RefSelect/ConfirmDestructive/StatusBadge/IDChip 基座组件 | 组件测试绿 |
| 1.2 | 出口策略 + 上游 + 端点 + 凭据 CRUD(含 provider 家族导入向导、写后清空) | 表单校验镜像测试 |
| 1.3 | 绑定编辑器(insert-only 语义如实呈现,G4 前删父重建流程) | — |
| 1.4 | 模型/别名/路由/候选(tier 分组 + SWRR 槽位预览 + 1024 告警 + 能力差异红标) | model.ts 测试覆盖槽位展开 |
| 1.5 | 访问组/授权矩阵/Client Key(签发 reveal-once + 组合轮换向导 + 吊销) | reveal-once 生命周期测试 |
| 1.6 | 配置版本工作区:验证错误码映射 → 发布向导 → 退火动效 → 回滚 | Playwright 全流程 |
| 1.7 | OAuth 设备流向导(Grok Build 轮询状态机)+ 端点测试 + 目录发现 diff | 状态机 model 测试 |
| 1.8 | 双审计流 + 备份/恢复页 | — |
| 1.9 | 质量门:`/impeccable critique` + `audit`;修复后 `polish` | 出口:G10 语义验收复现 |

### FE-2 观测 MVP(约 2 周)[CR:G2+G3]

| # | 任务 | 验收 |
|---|---|---|
| 2.1 | 后端联调:G2 接线 + G3 summary/timeline 最小面 | SQLite 直查对账一致 |
| 2.2 | StatTile/SparkLine/HealthStrip/TokenMixBar/ChartView 微图表族 | dataviz 检查清单过 |
| 2.3 | 总览页(07 §7.1 全布局,含未接线空态) | 双状态截图验收 |
| 2.4 | 时间范围 + 过滤器 URL 契约 + FilterBar | round-trip 测试 |
| 2.5 | 请求监控三视图 + 尝试时间线抽屉(8 阶段徽章) | 下钻链接全通 |
| 2.6 | 顶栏观测脉搏 | — |

### FE-3 诊断与运行时(约 1.5 周)

可用性矩阵/目录新鲜度/恢复探测(3.1);配额页家族分区(3.2)[CR:G6];Route Prism 基础版 + Explain 抽屉(3.3);SVG 折射增强 + 双路回落验收(3.4);凭据卡内联运行时徽章(3.5)。

### FE-4 用量分析(约 2 周)[CR:G3 完整]

六子页装配(4.1);RankTable + 实体对比(4.2);热力图 + 点击下钻(4.3);JSONL 导出/分块导入(4.4);10 万事件性能验收——对标 CPAMP 公布基准 Overview ~1.12s/5.23MB,目标 ≤1s(4.5)。

### FE-5 深化(持续)

i18n 双语(5.1);性能预算终验 + WCAG AA 抽查(5.2);`/impeccable polish` 全站(5.3);待批准项:G9 成本(价格簿 UI + 成本列点亮)、G8 SSE 化、巡检只读投影 →(再批准后)自动化(5.4-5.6)。

## 7. 与后端 CR 的接口(前端是契约共同作者)

| CR | 前端交付物(随 CR 提交) | 后端参考(源自 CPAMP 勘察,见 07 §3.3) |
|---|---|---|
| G1 图 | graph 响应的 TS 类型期望 + 页面消费清单 | — |
| G2 接线 | 无(纯后端);前端提供对账脚本 | AsyncSqliteEventWriter 已在库;AttemptEvent 时间戳必须入库(latency 唯一来源) |
| G3 分析 | §4.4 请求/响应形状草案 + per-tab include 矩阵 | 事件哈希幂等已有;rollup:checkpoint 增量 + format-version 重建;raw 前后沿 + 完整小时聚合;严格无过滤才走 rollup |
| G6 家族 | 配额页字段映射表(四家族 × source/confidence) | grok_build_* 七表就绪 |
| G7 capabilities | 特性位枚举 + unavailable 原因枚举 | 对标 /usage-service/info 无鉴权模式?否——我们保持管理鉴权后探测(网络策略已挡未授权者) |
| G9 成本 | 价格簿 UI 原型 + 计费语义清单(cache read/write/creation、长上下文、service tier) | CPAMP pricing/cost.go 与模糊匹配候选确认交互是好参照;同步仅手动触发 |

## 8. 风险登记

| 风险 | 等级 | 缓解 |
|---|---|---|
| 可复现构建失败 | 高 | §3.2 四级降级 + 退路管线;FE-0 第一周出结论 |
| G2/G3 后端排期挤压(与 P12 收尾竞争) | 高 | FE 阶段设计为 G1 先行:FE-0/FE-1 完全不依赖观测 CR;观测页空态先行合入 |
| 观测查询性能 | 中 | G3 自带 rollup 纪律;前端 per-tab include + 游标分页 + keepPreviousData;FE-4 设 10 万事件门槛 |
| 玻璃在低端设备 | 中 | 每屏 ≤3 面机械守卫;图表全实底;6× throttle 门槛 |
| UI 偏好零存储的体验损耗 | 低 | CR-FE-PREFS 预案(§2.4),FE-2 后评估 |
| echarts 体积 | 低 | core 按需注册 + charts.js 懒加载;体积预算:gzip 后 vendor ≤180KB、charts ≤120KB、main ≤150KB(CI 断言) |

## 9. 完成定义(DoD)

每阶段:CI fast+full 绿;安全不变量清单(§5.2)全过;impeccable 质量门(critique→fix→audit→polish 有界两轮);双主题 + 三降级截图归档;文档同步(本文件任务表勾选 + 07 文档如有设计漂移则先改设计再改码)。

项目级:G10 语义验收(纯 UI 完成两站聚合配置并发布,UI 开关不影响数据面基准);观测对账(面板数字 = SQLite 直查);备份恢复演练;WCAG AA 抽查;`docs/traceability.md` 挂接 H18/H19(/H20/H21 若批准)条目。
