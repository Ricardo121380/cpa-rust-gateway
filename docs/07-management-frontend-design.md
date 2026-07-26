# 07 · 管理前端设计文档 — Prism(Liquid Glass 管理与观测面板)

| 项目 | 值 |
|---|---|
| 状态 | `v0.2 Draft — 提案(对应功能矩阵 H18/H19/H20/H21,均为「待定」项,采纳需走 Change Request)` |
| 日期 | 2026-07-26(v0.2 修订:更正 CPAMP 参照对象并重构定位) |
| 定位 | 新一代管理 + 可观测性 Web 面板的设计契约;功能对标 **CPAMP = seakee/CPA-Manager-Plus**,视觉采用 Apple Liquid Glass 设计语言 |
| 配套文档 | [08 · 前端开发文档](08-management-frontend-development-plan.md)(工程实现、任务分解、验收) |
| 上游输入 | `docs/00-06`、`docs/openapi/management-v1.json`、`crates/*` 源码勘察、seakee/CPA-Manager-Plus 全量源码勘察(web + manager-server + docs)、Apple HIG Materials + WWDC25 219/356 |
| 代号 | **Prism(棱镜)** — 请求是光,网关是玻璃:光经棱镜折射到多个上游,与 Liquid Glass 的透镜化(Lensing)隐喻同构 |

> **v0.2 勘误**:v0.1 曾把 CPAMP 误认为 router-for-me/Cli-Proxy-API-Management-Center(CPA 官方面板)。经确认,CPAMP 指 **seakee/CPA-Manager-Plus** —— 一个「自托管管理面板 + AI Gateway 可观测性仪表盘」,覆盖请求、用量、成本、配额、失败诊断与账号健康。这一更正实质性改变了产品定位(§1)与缺口优先级(§10),本版为整体重写。

---

## 1. 定位:双平面,一个面板

### 1.1 真 CPAMP 是什么

seakee/CPA-Manager-Plus 由三部分组成(源码勘察结论,详见 §3):

1. **React 19 单文件 SPA**(Vite singlefile,hash 路由,ECharts,i18n 四语,三主题);
2. **Go manager-server sidecar**(:18317):从 CPA 的 usage queue 摄取请求事件 → sha256 去重落 SQLite(WAL)→ checkpoint 增量 rollup → 提供组合分析 API、价格簿、配额冷却、账号处置队列、Codex/xAI 巡检调度;并把浏览器从不接触 CPA Management Key 的反向代理做在服务端;
3. **双模式部署**:轻量面板(同一 SPA 内嵌进 CPA :8317,替换官方 UI,无额外服务)vs 完整模式(:18317 + 持久化与自动化)。SPA 通过探测无鉴权 `GET /usage-service/info` 的 service id 自行判别模式。

**关键结论:CPAMP 需要 sidecar,是因为 CPA(Go)本体没有持久化观测存储。而本项目的 Rust 网关已把等价物做进了本体** —— `gateway_event_log`(append-only、幂等去重)、`AsyncSqliteEventWriter`(有界队列、批量重试)、`TelemetryPipeline`(Prometheus 计数器 + 结构化 JSON + OTel span)全部类型冻结、测试覆盖,只是尚未在 `serve` 进程接线,也没有查询端点(勘察证据见 `crates/gateway-observability`、`crates/gateway-store/src/event_store.rs`)。

### 1.2 因此 Prism 的定位

**一个面板、两个平面、零 sidecar:**

| 平面 | 内容 | 后端现状 |
|---|---|---|
| **控制平面** | 版本化配置图(上游/端点/凭据/绑定/模型/路由/候选/访问组/Key/出口策略)、草稿→验证→发布→回滚、审计、备份 | 契约完备(41 操作,代码 1:1 对齐),缺全图读取(G1) |
| **观测平面** | 仪表盘、请求监控、用量分析、配额与健康、失败诊断 | 事件类型与持久层已在库,缺接线 + 查询端点(G2/G3) |

这与 v0.1 的最大差别:观测平面从「Later 附录」升级为**产品的一半**。CPAMP 生态反复证明了这一点 —— CPA 上游移除内置统计后,所有社区面板第一件事都是把用量观测加回来;CPAMP 的 README 开篇三问全是观测问题(请求为什么失败/成本花在哪里/账号配额是否健康)。

### 1.3 非目标(不变 + 新增)

- 不做多用户/RBAC(H22 Later)、不做美元级客户计费(BL-22);
- 不做远程托管前端(loopback/私网 + 同源,C1-C12 全部继承,见 §4);
- **不做 sidecar 进程**:观测能力通过网关本体接线实现,拒绝 CPAMP 的「三把钥匙、两个端口」复杂度(其文档为此花费了整章排障篇幅);
- 不复刻 YAML 源码编辑/diff(本项目无 config.yaml,配置是版本化图 + 原子发布);
- 不复刻浏览器端配额探测为主路径(CPAMP 的 /api-call 浏览器扇出探测受 CORS/UA/无人值守限制,其自身也已补服务端巡检 —— 我们直接以服务端投影为主)。

---

## 2. 用户与场景

**用户**:单一自托管运营者,loopback/私网访问,技术背景强,中文为主。

高频场景按频率排序(v0.2 依真 CPAMP 重排,观测场景前置):

1. **日常巡检**(每天多次):打开总览 → 今日请求/失败率/Token/延迟一眼扫过 → 健康条带有无红块 → 审计尾部有无异常;
2. **故障诊断**:某模型不通 → 请求监控过滤失败 → 尝试时间线看 8 阶段失败位置 → Route Explain 看候选排除原因 → 凭据 403 归因(账号被禁 vs 出口被拒);
3. **用量复盘**(每周):按模型/凭据/Client Key/时间拆解请求、Token、(未来)成本;
4. **凭据运维**:OAuth 重授权、配额窗口查看、配额恢复探测、轮换 Client Key;
5. **变更发布**:克隆活动版本 → 修改 → 验证 → 发布 → 必要时一步回滚;
6. **初始布线 / 备份恢复**(低频):完整配置链、加密备份预检与恢复。

依 impeccable 方法论,本产品全站为 **Operate 模式**(访客在完成任务):可扫读性、一致性、密度与真实使用场景优先于表达;品牌存在于精确的细节里,克制是底线(Restrained 色彩策略,系统字体栈,150-250ms 状态动效,无页面加载编排)。

---

## 3. 参照系:真 CPAMP 逐页对照

### 3.1 CPAMP 页面清单(源码勘察,`apps/web/src/router/MainRoutes.tsx` + 截图)

导航顺序(用户已被验证接受的顺序,分组即信息架构):

```text
组1(观测): 仪表盘 · 用量分析* · 请求监控*
组2(配置): 配置面板 · AI 提供商 · 插件管理*
组3(凭据): 认证文件 · OAuth 登录 · 配额管理 · 凭证健康巡检
组4(诊断): 日志查看*(需文件日志)
组6:       系统信息          (* = 能力探测后条件显示)
```

### 3.2 对照表(→ Prism 对应与差异)

| CPAMP 页面 | 核心能力(源码级) | Prism 对应 | 差异/依据 |
|---|---|---|---|
| **仪表盘** | 连接状态卡、版本卡、健康状态卡;今日 6 KPI 瓦片带火花线(请求/RPM/TPM/成本/成功率/延迟);流量趋势(bar+line);10 分钟粒度请求健康条带;Token 构成(缓存/输入/输出/推理);模型成本排行;渠道健康;最近失败;60s 轮询 `GET /dashboard/summary` | **总览** | 直接对标;成本瓦片在 G9 批准前显示 Token 量;健康条带数据来自 gateway_event_log(G2/G3) |
| **用量分析** | 6 子页(总览/趋势/模型/客户端Key/凭证/热力图);时间范围+粒度(自动/时/天);高级过滤(认证文件/最小延迟/缓存状态);6 种图表:多轴趋势线、成本排行横条、健康趋势、Token 堆叠、实体对比多线、weekday×hour 热力图(visualMap+点击下钻贡献者);异常时间点表;全部由**单个组合 POST /monitoring/analytics** 驱动 | **用量分析** | 对标其组合端点设计(G3);**图表纪律不同**:CPAMP 趋势图是三 Y 轴(违反单轴原则),Prism 用小倍数/指数化替代 |
| **请求监控** | 3 视图(账号汇总/调用方Key汇总/实时);9 KPI 瓦片;时间范围+6 个过滤下拉+搜索(API Key 走 sha256 匹配);10s 自动刷新(页签隐藏暂停);逐请求行:模型/强度/状态/成功率/TPS/首字/耗时/用量/花费;JSONL 导出+可断点续传分块导入;脱敏开关;仅失败过滤 | **请求监控** | 对标;行内字段映射到我们的 RequestEvent+AttemptEvent+UsageEvent(value-free:无请求体,失败只有错误码×域+阶段);TTFB/耗时来自 AttemptEvent 时间戳对 |
| **配置面板** | 可视化 YAML 编辑+CodeMirror 源码+保存 diff;Manager 连接配置页 | **配置版本工作区** | 本质差异:版本化图+原子发布替代文件编辑;diff 对应 validate 错误码 + 版本对比(G10) |
| **AI 提供商** | 7 类 provider 表格+抽屉编辑+健康检查(经 /api-call 探测 /models);行内优先级编辑;read-modify-write PUT(无并发控制) | **上游域** | 我们分层为 Upstream→Endpoint(单协议)→Credential→Binding;**并发控制不同**:ETag/If-Match 乐观并发替代 read-modify-write |
| **认证文件** | 卡片网格+批量操作(下载/启停/优先级/删除);问题过滤(reauth/限额/禁用);配额冷却与处置队列徽章内联;Sub2API 浏览器端拆分导入 | **上游域 · 凭据** | 凭据 AEAD write-only、无下载(安全模型更严);冷却/处置状态对应我们的运行时可用性六态 |
| **OAuth 登录** | 5 provider 设备流卡(authurl→轮询→回调粘贴);Vertex JSON 导入 | **OAuth 设备流向导** | 契约已有 start/status/cancel;Grok Build 六种导入形状 + 设备授权轮询 |
| **配额管理** | 5 provider 区块,浏览器经 /api-call 探测上游配额;观测头快照(30d)增强置信度展示 | **运行时 · 配额** | 服务端投影为主(grok_build_* 七表已持久化,缺 HTTP 暴露 = G6);header 快照思想 ≈ 我们的 source+confidence 双标签配额窗 |
| **凭证健康巡检** | 本机(浏览器)+服务端(调度/租约/历史)双模巡检;建议动作(重登/启用/禁用/删除/保留)+一键执行;**所有权规则**:自动恢复只恢复自己禁用的 | **运行时 · 健康** | 对应我们的健康六态+受控恢复探测;所有权规则直接进后端设计参考(G6/G8);首版只读投影,自动化 Later |
| **Auth Issues 处置队列** | 凭据故障→候选队列(pending/ignored/resolved),证据脱敏展示,语义分类而非状态码分类 | **运行时 · 处置队列(Later)** | 语义分类哲学与我们的 17 码×10 域错误分类同构 |
| **模型价格** | LiteLLM/OpenRouter 手动同步+模糊匹配候选确认+本地覆盖;cache read/write/长上下文计费语义 | **成本(G9,需批准)** | 新后端范围;BL-22 禁客户计费,运营者成本估算是独立决策 |
| **插件管理** | 安装/商店/iframe 插件页 | 无对应 | 本项目首版无动态插件(设计决策,00 文档) |
| **日志查看** | 文件日志尾随+过滤+错误请求日志下载 | **审计 + 请求尝试** | 哲学差异:我们无自由文本请求日志,全部 value-free 事件与闭集枚举 |
| **系统信息** | 快链、模型分组视图、清除本地登录 | **设置/关于** | 对标 |

### 3.3 从 CPAMP 复制的模式(设计资产)

以下经其源码验证有效,直接纳入 Prism 设计:

1. **能力探测 + 优雅降级**:单一缓存 hook 探测能力端点,导出逐特性布尔值 + 机器可读不可用原因(`service_not_configured | service_unavailable | monitoring_disabled`);导航隐藏、路由重定向、页面渲染原因专属空态。Prism 对应:fail-closed 503 分类 + 新增 `GET /admin/capabilities`(G7);
2. **组合分析端点**:一次 `POST analytics {from,to,timezone,filters,include{...}}` 返回 summary+timeline+ranks+filter options+anomalies,支撑 6 个子页 + 跨页下钻(G3 的契约模板);
3. **深链下钻**:用量分析 → 请求监控的每个「查看详情」把时间窗+全部过滤器编码进 URL query,目标页挂载时解析。Prism 从 FE-0 起就把过滤器纳入 URL schema;
4. **UI 微模式**:stat tile+火花线;10 分钟健康条带;weekday×hour 热力图点击下钻;稳定选项缓存(过滤下拉在刷新中不闪烁);过滤切换期间保留旧数据快照;「真空 / 过滤后空 / 投影不可用」三种空态严格分离;折叠侧栏的短标签 i18n 变体;单一全局刷新按钮委托当前页注册的 handler;
5. **每页 UI 状态持久化**(tab/过滤/排序/页大小)—— 但受 C6 零存储约束,Prism v1 以会话内存实现,持久化是开放决策(见 08 文档 §2.4);
6. **可断点续传的用量导入 + 流式 JSONL 导出**(G3 附属,数据可迁移性);
7. **后端参考**(写入 G2/G3 的 CR):事件内容哈希去重 + 死信表;checkpoint 增量 rollup(wake channel + ticker + 续跑 timer,每批 1000);「format version 不匹配 → 重建派生表」;自动化「所有权」规则(谁禁用谁恢复);env > DB > 默认的配置来源 + 逐字段 env 锁定。

### 3.4 刻意不同(且有依据)

| CPAMP 做法 | Prism 做法 | 依据 |
|---|---|---|
| 三把钥匙(客户 Key/CPA 管理 Key/CPAMP Admin Key)× 两端口,文档整章排障 | 单管理 Key + 单管理监听器 | 无 sidecar;`mgmt_`/`rgw_`/`csrf_` 前缀命名空间已区分 |
| Management Key 混淆后存 localStorage | 密钥仅内存,刷新即清 | C6 机械强制;CPAMP 自己的混淆是可逆的 |
| provider 写入 read-modify-write PUT,并发裸奔 | X-Config-Version + If-Match 乐观并发 | 后端已实现,409 无部分写入 |
| 失败原文 fail_body 存本地 DB(仅 DB 可见) | 全链路 value-free(错误码×域+阶段,无原文) | 后端冻结设计;诊断能力差异如实告知用户 |
| 用量趋势三 Y 轴 | 单轴纪律:小倍数/指数化 | dataviz 规范;双轴是图表第一反模式 |
| 3900 行页面组件 | 纯模型模块 + 薄 JSX,从第一天拆分 | CPAMP 自己靠 1500+ 行测试补偿,我们提前 |
| RESP/HTTP 队列摄取,消费者宕机超保留期即永久丢数据 | 网关本体直写事件日志(无跨进程队列) | 架构优势:AsyncSqliteEventWriter 就在进程内 |
| hash 路由(单文件嵌入需要) | hash 路由(同理:嵌入式静态服务无 fallback) | 相同约束,相同选择 |

---

## 4. 硬约束(doc-locked,不变)

C1-C12 全部继承 v0.1(BL-19 独立 SPA 不进热路径;G10 UI 开关不影响数据面基准;编译期 include_bytes! 嵌入+静态路由清单;CSP 'self' 无内联无 CDN;全部流量走生成客户端;秘密零浏览器存储;reveal-once/write-only/备份工件不读取;X-Config-Version+If-Match;统一 404 不可探测;双构建字节一致;PATCH 整体替换;API 先于 UI)。任何新页面能力先改 OpenAPI 契约。

---

## 5. 信息架构

### 5.1 导航(9 区,观测优先 —— 采用 CPAMP 验证过的分组次序)

| # | 区 | 主要内容 | 后端依赖 |
|---|---|---|---|
| 1 | **总览** | 连接/版本/健康卡 + 今日 KPI 瓦片 + 流量趋势 + 健康条带 + Token 构成 + 失败尾部 + 审计尾部 | G2+G3(空态可先行) |
| 2 | **用量分析** | 总览/趋势/模型/Client Key/凭据/热力图 六子页 | G2+G3 |
| 3 | **请求监控** | 凭据汇总/Key 汇总/实时 三视图 + 尝试时间线抽屉 | G2+G3(尝试时间线今天可用,8 条窥孔) |
| 4 | **配置版本** | 生命周期中枢:列表/谱系/克隆/验证/发布/回滚 | 现有契约 |
| 5 | **上游** | Upstream→Endpoint→Credential→Binding;端点测试;目录发现;OAuth 向导 | G1;OAuth/测试/发现契约已有 |
| 6 | **模型与路由** | 公开模型/别名/1:1 路由/候选;结构验证;Route Prism(Explain) | G1 |
| 7 | **访问控制** | 访问组/授权矩阵/Client Key 签发-吊销 | G1 |
| 8 | **出口策略** | Egress Policy CRUD + 引用视图 | G1 |
| 9 | **运行时** | 可用性矩阵/目录新鲜度/配额(家族窗口)/恢复探测/(Later)巡检与处置队列 | 契约已有(投影空)+G6 |
| 10 | **审计与备份** | 双审计流;备份预检/恢复 | 现有契约 |
| ⚙ | 设置 | 主题/语言/连接信息/关于 | 本地 |

### 5.2 全局壳(不变的三面玻璃 + 观测化顶栏)

```text
┌──────────────────────────────────────────────────────────────────────┐
│ ⌘ 顶部上下文栏(玻璃):版本 ▾ rev-42 ◐draft │ ●数据面 │ 今日 1.8k/98.7% │
│                                            [验证] [发布…]  🔍  ⚙︎     │
├──────────┬───────────────────────────────────────────────────────────┤
│ 玻璃导航  │  内容画布(实底,永不玻璃)                                │
│ ○总览    │  KPI 瓦片行 / 图表卡 / 数据表格(虚拟滚动)                │
│ ○用量分析 │                                                          │
│ ○请求监控 │                                                          │
│ ─────    │                                                           │
│ ○配置版本 │                                                          │
│ ○上游    │                                                           │
│ ○模型路由 │                                                          │
│ ○访问控制 │                                                          │
│ ○出口策略 │                                                           │
│ ─────    │                                                           │
│ ○运行时  │                                                            │
│ ○审计备份 │                                                           │
├──────────┴───────────────────────────────────────────────────────────┤
│    ▽ 草稿浮动操作条(玻璃,仅 draft):「3 处未发布 · rev-42」[放弃][发布] │
└──────────────────────────────────────────────────────────────────────┘
```

顶栏新增**观测脉搏**:数据面 `/healthz` 状态点 + 今日请求数/成功率微型摘要(G3 就绪前隐藏)。玻璃仍然只有三面(导航/顶栏/浮动条)+模态层。

---

## 6. 全局交互契约(v0.1 继承 + 观测新增)

继承:版本上下文与只读/克隆语义、revision 管线(ETag 自动推进 + 409 冲突条)、错误映射(404 会话失效/400 表单/409 冲突/503 投影不可用一等空态)、秘密生命周期(reveal-once sheet/write-only/组合轮换向导)、schema 驱动表单 + 完整 `*Input` 提交 + 跨引用下拉。

观测平面新增:

- **时间范围模型**:全局 `TimeRange`(今天/24h/昨天/7d/30d/自定义)+ 粒度(自动/时/天),序列化进 URL query;跨页下钻保持范围;
- **轮询策略**:观测页激活 10s(页签隐藏暂停,visibilitychange),总览 60s,配置页按需;G8(SSE)落地后改事件失效;
- **过滤器 URL 契约**:`from,to,model,credential,client_key,endpoint,status,stage` 全部可编码/解析,任何「查看详情」都是带参链接;
- **数据保鲜**:过滤切换期间显示旧快照 + 加载指示(绝不闪空);下拉选项缓存不因刷新而清空;
- **脱敏默认开**:Client Key 只显示 prefix;credential 只显示 id/kind/revision;导出与展示同一脱敏规则。

---

## 7. 页面规格

### 7.1 总览

```text
┌ 活动版本卡 ──────┐ ┌ 数据面 ─────┐ ┌ 事件管道 ──────────┐
│ v-2026-07 ●      │ │ /healthz ●  │ │ pending 0 · fail 0 │
│ rev-87 · 3d 前   │ └─────────────┘ └────────────────────┘
┌ 今日 KPI(6 瓦片 + 火花线)────────────────────────────────┐
│ 请求 1,877(失败 24) 成功率 98.7%  P95 21.4s              │
│ Token 217.6M  RPM(30m) 2.3  TPM(30m) 119.4K              │
└──────────────────────────────────────────────────────────┘
┌ 流量趋势(今日,请求条+Token 线,小倍数)┐ ┌ Token 构成 ────┐
│                                        │ │ 缓存/输入/输出/ │
└────────────────────────────────────────┘ │ 推理 占比条     │
┌ 请求健康条带(10 分钟桶 × 状态色格)─────┴─────────────────┐
│ 00 ▫▫▫▫▫▫ 06 ▫▫▪▪▪▪ 12 ▪▪▪🟥▪▪ 18 ▪▪▫▫▫▫ 23             │
└──────────────────────────────────────────────────────────┘
┌ 最近失败(错误码×域+阶段)─────┐ ┌ 审计尾部(5 条)─────────┐
└───────────────────────────────┘ └─────────────────────────┘
```

- KPI 瓦片 = dataviz stat tile 规范(大数 tabular-nums、次行说明、火花线弱化);
- 成本瓦片在 G9 批准前不出现(不显示 $0.00 假数据);
- G2/G3 未就绪时:观测卡渲染「事件管道未接线」专属空态(附文档链接),配置类卡片(活动版本/布线规模)正常工作。

### 7.2 用量分析(六子页)

- **总览**:KPI 行(请求/成功率+P95/失败+异常点/Token 四类)+ 用量趋势(请求数、Token、成本各自小倍数或指数化对比 —— **不做三 Y 轴**)+ provider 汇总表;
- **趋势**:指标切换(requests/totalTokens/input/output/cached/reasoning/成本*)单轴折线 + >12 桶时内置 dataZoom + 选中桶虚线标记;
- **模型 / Client Key / 凭据**:RankTable(排名/名称/请求/Token/失败率/最后活跃)+ 展开对比面板 + 实体对比多线图(top-N,固定色序);
- **热力图**:weekday×hour,指标切换(请求/Token/失败率),visualMap 顺序色带(单色相),点击格子 → 贡献者面板(模型/Key/凭据)→ 深链请求监控;
- **异常时间点**(Later,G3 二期):后端聚合的异常桶列表;
- 数据:单一 `POST /admin/analytics`(G3),per-tab 按需 include。

### 7.3 请求监控(三视图)

- KPI 行(总调用/成功率/失败/Token 四类,窗口内);
- **凭据汇总**:表格或卡片模式;列 = 凭据(id+upstream)/状态徽章(运行时六态)/调用/成功/失败/成功率/Token/最后请求;展开行 = 该凭据配额窗口(G6)+ 最近尝试;
- **Client Key 汇总**:prefix(mono)/访问组/调用/失败/Token/最后请求;
- **实时**:逐请求事件行(时间/模型/协议/流式/状态/重试决策/Token/延迟*),仅失败开关,加载更多(保留上限说明);行点击 → **尝试时间线抽屉**:attempt 序列 + 8 阶段失败位置徽章 + 候选/凭据/端点 ID 链;
- 顶部说明现状:P12 阶段窥孔仅 8 条(如实标注),G2 接线后成为完整历史;
- JSONL 导出 / 分块导入(G3 附属端点)。

### 7.4 配置版本 / 上游 / 模型与路由 / 访问控制 / 出口策略

与 v0.1 §7.3-§7.7 规格一致(三栏渐进披露、provider 家族凭据导入向导、Grok Build 设备授权轮询 UI、绑定编辑器、端点四态测试、目录发现 diff 卡、候选 tier 分组 + SWRR 槽位预览 + 1024 上限告警、AllowedUnlisted 显著警示、能力差异红标、Route Prism 抽屉、授权矩阵、reveal-once、级联删除确认、redirect_mode 联动)。此处不重复,实现细节见 08 文档。

v0.2 增补:

- **上游 · 凭据卡**内联显示运行时状态徽章(可用性六态)与配额摘要(G6),对标 CPAMP 认证文件卡的冷却/处置徽章;
- **模型详情**增加「该模型今日用量」迷你条(G3),打通配置↔观测。

### 7.5 运行时

- **可用性矩阵**(endpoint×credential 六态热格)与**目录新鲜度**:同 v0.1,空投影如实显示;
- **配额**:按 provider 家族分区(对标 CPAMP 配额页):Grok Official 头部双窗 / Grok Build 四类窗(source+confidence 双标签)+ 账单计划 / Grok Web REST vs gRPC 观测窗 / Kiro 订阅+超额;数据依赖 G6;
- **恢复探测**:三态诚实呈现;`credential_forbidden` 行内入口;
- **(Later)健康巡检与处置队列**:采纳 CPAMP 的语义分类(证据驱动,非状态码驱动)与所有权规则,等待后端自动化范围获批。

### 7.6 审计与备份 / 解锁页 / 设置

同 v0.1(双审计流按版本过滤;备份 preflight → 空库恢复,无下载端点;解锁页单玻璃卡 + 不可探测文案;设置含主题三态/语言/清除会话)。

---

## 8. Prism 设计系统(Liquid Glass,v0.1 验证成果全部保留)

设计方向契约(impeccable 格式):

> **THESIS**:一个把「版本化配置」和「实时观测」放进同一块玻璃的网关面板;拒绝的类目默认 = 白卡片 + 彩色图标瓦片 + 侧栏蓝高亮的通用 admin(即 CPAMP 自己的样子)。
> **OWN-WORLD**:Apple Liquid Glass 官方规则下的三面玻璃(导航/顶栏/浮动条)+ 全实底内容画布;SF 系统栈,Mono 承载 ID 语汇;冷调环境渐变画布;tint 唯一强调;材质携带配置状态(draft 磨砂/active 通透/archived 哑光)。
> **STORY**:运营者解锁 → 顶栏看见版本与今日脉搏 → 总览扫健康 → 下钻监控/解释路由 → 草稿修改 → 浮动条发布(退火动效)。
> **FIRST VIEWPORT**(总览):KPI 六瓦片 + 健康条带在首屏,主操作是顶栏「发布」(仅 draft 时着色)。
> **FORM**:Operate 模式管理面板;署名元素 = Route Prism 光路(唯一 SVG 折射增强位)+ 发布退火;其余全部安静。

以下与 v0.1 完全一致,不重复全文,仅列条目与关键值:

- **原则**(§8.1):玻璃仅功能层/不叠玻璃/每屏 ≤3 面/只用 Regular 变体/tint 即强调/滚动边缘效果;
- **Token**(§8.2):canvas/surface/ink 三层双主题;玻璃变量(light `rgba(255,255,255,.55)` blur 20 sat 180% / dark `rgba(22,22,29,.52)`);SF 三角色(Display/Text/Mono,`tabular-nums`);同心圆角(内半径=外半径−内边距);4pt 网格;200ms `cubic-bezier(.32,.72,0,1)`;
- **玻璃配方与渐进增强**(§8.3):基线 backdrop-filter 全浏览器 + `@supports` 不透明降级;SVG feDisplacementMap 折射仅 Chromium、仅 Route Prism 与解锁卡;
- **材质语义**(§8.4):draft blur28/sat120+噪点、active blur14/sat190、archived blur20/sat80;发布=600ms 退火(reduced-motion 退化交叉淡入);
- **署名元素**(§8.5):Route Prism 光路图(静态=拓扑,诊断=Explain 解释器);
- **徽章词汇表**(§8.6):全部闭集枚举,色+图标+文字;v0.2 新增观测徽章:重试决策 6 值、10 分钟健康条带四色(无请求/成功/警告/失败)、异常严重度;
- **无障碍**(§8.7):reduced-transparency→磨砂近实底 / contrast-more→实底+边框 / reduced-motion→降级;WCAG AA 含最坏滚动情况;焦点环 2px tint;
- **性能预算**(§8.8):backdrop-filter 面每屏 ≤3;表格图表长列表永不玻璃;6× CPU throttle 验收;
- **图表规范**(§8.9,dataviz 六项验证通过):分类色 Light `#0066D6/#B85F00/#0079AB/#C41E77`、Dark `#0A84FF/#CC7A00/#2196C9/#DB4E92`;固定顺序、色随实体;状态色独立成池;单轴纪律;深色独立选定。v0.2 新增:热力图顺序色带 = 单色相(tint 蓝)明度阶梯,双主题分别取阶;健康条带用状态色池(good/warn/crit)+ 形状冗余。

---

## 9. 组件清单(v0.1 全集 + 观测新增)

新增:StatTile(大数+副行+火花线)、SparkLine、HealthStrip(10 分钟桶条带)、TrendChart(单轴+dataZoom)、RankTable(展开对比)、HeatmapCard(点击下钻)、TokenMixBar、AttemptTimelineDrawer、TimeRangePicker(+粒度)、FilterBar(URL 同步)、ExportImportBar(JSONL+分块续传)、PulseIndicator(顶栏脉搏)。
继承:GlassRail/GlassTopBar/GlassDock/GlassSheet/GlassPopover/GlassToast、Card、DataTable、StatusBadge、IDChip、RevisionChip、VersionPicker、CapabilityMatrix、TierWeightEditor、RoutePrism、AvailabilityMatrix、QuotaGauge、RevealOnceSheet、JsonKVEditor、EmptyState/UnavailableState、DiffCard、OAuthDeviceFlow。

---

## 10. 后端契约缺口(v0.2 重排,观测平面升级)

| # | 缺口 | 说明 | 优先级 |
|---|---|---|---|
| **G1** | **配置全图读取** | `GET /admin/config-versions/{id}/graph`(redacted)或补齐 endpoints/credentials/aliases/candidates/routes 的 list;store `load_configuration` 已具备,缺 HTTP 投影 | **P0(控制平面死锁)** |
| **G2** | **事件管道接线** | 在 `serve` 组装 BoundedEventQueue + AsyncSqliteEventWriter(+可选 TelemetryPipeline);组件全部在库且有测试,纯装配工作;含 AttemptEvent(P12 sink 目前丢弃时间戳) | **P0(观测平面死锁)** |
| **G3** | **组合分析端点** | `POST /admin/analytics`(from/to/timezone/filters/include→summary+timeline+ranks+options)+ `GET /admin/dashboard/summary`;参考 CPAMP 契约与其 rollup 纪律(checkpoint 增量、format-version 重建、原始事件+小时聚合);附属:JSONL 导出/分块导入 | **P0(观测平面)** |
| G4 | 子资源 update/delete | 别名/候选/绑定/授权 insert-only → 高频编辑要删父重建 | P1 |
| G5 | ClientKey 元数据 | name/created_at/last_used(CPAMP 用 api_key_aliases 表补显示名 —— 我们应做进主记录) | P1 |
| G6 | 家族运行时投影 | grok_build_* 七表(账单/配额窗/亲和)+ Kiro 目录的管理 HTTP 暴露;配额页与凭据卡依赖 | P1 |
| G7 | capabilities 端点 | `GET /admin/capabilities`(特性布尔+不可用原因),替代逐端点试探;对标 CPAMP /usage-service/info 模式 | P1(廉价高杠杆) |
| G8 | SSE 推送 | H20;落地前轮询 | P2 |
| G9 | 价格簿与成本 | 模型价格表(LiteLLM/OpenRouter 同步+本地覆盖+cache/长上下文语义)+成本计算;**新范围,需明确批准**(BL-22 边界:运营者估算 ≠ 客户计费) | P2(待批准) |
| G10 | 版本 diff API | 可先前端双图对比(G1 后) | P3 |

明确不做:备份下载端点、任意 API 代理(H14 Drop)、YAML 覆盖(H04 Drop)、跨进程用量队列(架构优势,不引入)。

---

## 11. 实施路线图(v0.2:观测提前,双轨并行)

| 阶段 | 内容 | 依赖 | 出口条件 |
|---|---|---|---|
| **FE-0 基座** | 契约 CR(G1+G7 必须,G2+G3 同批提出);构建管线+嵌入清单+check 扩展;Token/玻璃组件库/双主题/a11y 三降级;解锁页/壳/版本上下文/只读全图 | — | 双构建字节一致;a11y 三开关生效;active 版本全图只读可浏览 |
| **FE-1 配置面** | 8 区 CRUD、发布/回滚向导、revision 冲突、reveal-once、级联确认、OAuth 向导、端点测试、目录发现 | G1 | G10 语义验收:两站 minimax-m3 聚合纯 UI 配置并发布;秘密不变量机械校验 |
| **FE-2 观测 MVP** | 总览(KPI/趋势/健康条带/Token 构成)+ 请求监控三视图 + 尝试时间线;时间范围与过滤 URL 契约 | G2+G3 | 真实流量下仪表盘数据与 SQLite 直查一致;空态/未接线态走查 |
| **FE-3 诊断与运行时** | 可用性矩阵/目录新鲜度/恢复探测/Route Prism(Explain)/配额页(家族分区) | G6(配额) | Chromium 折射增强 + Safari/Firefox 回落双路验收 |
| **FE-4 用量分析** | 六子页 + 热力图 + 排行 + 实体对比 + 导出导入 | G3 完整 | 图表色板验证脚本通过;10 万事件下 P95 查询体验达标(对标 CPAMP 公布的 1.12s 基准) |
| **FE-5 深化** | i18n(zh-CN/en)、审计/备份打磨、退火动效、性能验收;(若批准)成本 G9、SSE G8、巡检自动化 | G8/G9 | 6× throttle 帧率达标;WCAG AA 抽查;impeccable audit/polish 通过 |

任务级分解、验收细则与工程决策见 [08 · 前端开发文档](08-management-frontend-development-plan.md)。

---

## 12. 风险与开放问题

1. **H 区待定**:H18/H19/H20/H21 需 CR 获批(建议 H18/H19/H21 Keep-New,H20 Later);G9 成本是独立的新范围决策;
2. **可复现构建 vs Vite**:字节一致硬门槛,FE-0 首周穿刺,失败回退 tsc-only 轻量层(08 文档 §3.2 有备选方案);
3. **UI 状态持久化 vs 零存储**:CPAMP 的每页状态记忆是显著 UX 优势,但 C6 机械禁止全部浏览器存储;v1 以内存实现,提出「审计过的非秘密 uiPrefs 白名单」作为后续 CR(08 文档 §2.4);
4. **观测数据量**:CPAMP 在 10 万事件规模上花了十轮优化(rollup/按需 tab/紧凑 profile);我们的 G3 设计从第一天带 rollup 纪律,08 文档含性能预算;
5. **玻璃性能**:每屏 ≤3 面不可协商;观测页图表密度高,全部实底;
6. **假设标注**(shape 流程要求):a) 成本分析被假设为期望功能(CPAMP 核心),但等待 G9 批准;b) 观测平面被提为产品一半(依据 CPAMP 生态证据);c) 巡检自动化(自动禁用/恢复)不在 v1,仅只读投影。以上任一与预期不符,请在 FE-0 前指出。

---

## 13. 参考资料

**CPAMP(本设计的功能参照)**
- [seakee/CPA-Manager-Plus(源码勘察于 2026-07-26:apps/web 65+ 组件、apps/manager-server 108 个 Go 文件、apps/docs 全量手册)](https://github.com/seakee/CPA-Manager-Plus)
- 关键源码证据:`apps/web/src/router/MainRoutes.tsx`(路由清单)、`hooks/usePanelFeatureAvailability.ts`(能力探测)、`features/usage-analytics/UsageAnalyticsPage.tsx`(六子页与图表)、`apps/manager-server/internal/repository/sqlite/migrate.go`(事件/rollup/自动化 schema)、`internal/service/proxy/service.go`(服务端密钥注入)、`docs/usage-analytics-implementation-plan.zh-CN.md`

**Apple Liquid Glass(官方)**:HIG Materials(2025-06-09/2025-09-09)、WWDC25 Session 219/356、Adopting Liquid Glass、Apple Newsroom 2025-06-09 —— 链接同 v0.1。

**Web 实现**:kube.io(SVG 折射,2025-09-04)、LogRocket、axonixtools 2026 指南、rdev/liquid-glass-react(~5.7k★)。

**本仓库**:`docs/openapi/management-v1.json`、`docs/01/05/06`、`crates/gateway-store`(event_store 与控制平面)、`crates/gateway-observability`(队列/遥测)、`crates/gateway-router`(快照/Explain/健康/配额)、`crates/provider-grok`·`provider-kiro`。
