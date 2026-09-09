# Prism 下一阶段前端开发方案

日期：2026-09-09。状态：**前期设计与接口方案，尚未实施**。正式前端目录：`web/prism/`。

讨论定稿：用户已确认 V4、Codex 统一实施前后端、补齐核心后端能力、以本地真实网关验收为终点。当前实施范围、顺序、分工和完成条件见 [Prism V4 正式实施计划](prism-v4-execution-plan.md)，配套 [Goal prompt](prism-v4-goal-prompt.md)。本文件保留此前的需求分析与接口依据；BE-FE-01/02 已纳入本轮，原有仅前端估算不代表新增全栈范围。

基于 `1ba4340b56e9f4214c98912f78dc477fdc817ea2`、旧计划 A–D 已交付内容和最新后端 handoff 制定。保留现有 React/Vite/TanStack Query/Zustand、四文件嵌入和管理安全边界；视觉方向锁定为用户明确要求的 **Apple Liquid Glass**，遵循 `web/prism/DESIGN.md`，不得自行替换为 Linear、实底侧栏或靛蓝主题。完整原型覆盖现有 12 个栏目与新增账号池、模型目录。

配套：[项目审查与证据](../reports/project-review-2026-09-09.md)、[Liquid Glass V4 完整交互设计](../design/prism-liquid-glass-v4.html)、[V4 排版与配色交接](prism-opendesign-v4.md)、[V3 完整栏目与交互清单](prism-opendesign-v3.md)。V4 按用户最新要求继续打磨整体结构与配色，保留 V3 的全部栏目与交互边界；当前视觉以 V4 为准。设计由本会话模型完成，通过 OpenDesign MCP 读写和预览。本文件不修改 `docs/06`、`docs/08` 的历史验收状态，也不把新后端 API 提案标成已存在。

## 1. 接手前需要知道的变化

建议阅读顺序：

1. 根目录 `CLAUDE.md` / `AGENTS.md` 与 `docs/cross-boundary-log.md` 最新记录。
2. [Oracle Singapore handoff](claude-code-oracle-singapore-vps.md)，尤其管理同源、CPAR/Autoreg 分工和发布方式。
3. `docs/08-management-frontend-development-plan.md` 的 §3.0、`web/prism/DESIGN.md` 的既有视觉与降级约束。
4. 本次 review 的 F1–F3、B1–B4，然后执行下面的 F0。

旧计划的 A–D 已完成，不能重新估成从零建设；其 87/99 接线率仍成立，但新增 schema 字段未接。此次确认的权威契约差异有 `CatalogStatus`、`ProviderAccountPoolItem`、`ProviderAccountEntitlement`。后端 P13-15 仍未完成所有渠道和正式 Gate。

最近交接必须继续遵守：

- 权益是 `domain + tier + source + confidence + observed_at_ms`；在**账号详情**展示。`null` 是未观测，不是 Free，也不等于 `tier=unknown`。Build/Web/Console 不共享权益结论，ChatGPT 与 Claude 的同名套餐也不混用。
- 账号认证、运行时、quota、权益和目录是不同事实；可以并排展示，不能合成为 overall health。
- 模型 ID 来自后端真实目录和授权投影，不写前端模型白名单或套餐到模型的映射。
- 已导入且受支持的 OAuth 日常刷新由 CPAR 负责；初次授权、撤销 grant 后的交互重授权与注册补池属于 operator/Autoreg。Prism 展示状态，不新增自动授权/自动恢复逻辑。
- 目录按 target 展示 `fresh/stale/expired/missing`。1 个 Fresh 不意味着所有 target 正常；Missing 不是空目录刷新成功。
- Oracle 生产运行状态在旧 handoff 中是有日期的证据。前端开发不把那些时点的模型集合、账号状态或 revision 写成常量。

## 2. 下一版的产品目标

操作员应能完成三个清楚的任务：

1. 找到一个账号，分别看到它的认证、调度、续期时间和权益证据，知道是否需要人工处理。
2. 区分“草稿配置中的模型/路由”和“当前客户端实际上可以调用的 exact model”，并知道目录数据从哪里来。
3. 从请求或失败记录进入 attempts、目标账号和路由诊断，保留筛选上下文，不靠复制多个 ID 在页面间手工寻找。

第一阶段的结果是可用的账号/目录视图和可靠的会话/版本状态。完整时间趋势、延迟分位和自动巡检不作为这一阶段的前端完成条件。

## 3. 信息架构：复用现有路由，拆解拥挤页面

| 导航组 | 页面/工作区 | 调整方式 |
|---|---|---|
| 运行 | 总览、请求与失败、用量分析、计费与价格 | 保留四个已有页面；请求两条流、用量观测、全局价格目录分别保留自己的范围。 |
| 资源 | 账号池、上游、模型目录、模型与路由、访问控制 | 新增独立账号池与目录状态入口；其余三页继续承接现有配置能力。 |
| 管理 | 运行诊断、出口策略、配置版本、审计与备份、设置 | 保留五个已有页面；RuntimePage 提取日常账号与目录任务后聚焦诊断。 |

**完整范围为 14 个工作区，另有解锁页。** 原型没有禁用的主导航项，每页均有内容和主要交互；全部栏目、子视图与旧源码对应关系见 V3 交接清单，排版、颜色与详情层级采用 V4 细化。正式实现不得以三页样板或侧栏占位作为整轮完成条件。

新增账号入口使用 `#/accounts`，目录入口使用 `#/catalog`。原型总览是 `#/overview`，正式前端继续保留既有 `#/` 总览路由，兼容别名可在路由层处理。其他现有路径、带筛选 query 和原 runtime 账号定位链接应保持可用；已有十二页不应因重排导航而丢失功能。

全局顶栏继续显示所选配置版本、draft/active/archived 与 revision。运营读取卡片直接标注“实时 · 跨版本”或“配置版本 X”，不能让顶栏选择器暗示所有数据都按版本过滤。每个分页投影保留自己的 snapshot 和观测时间。

## 4. 页面级设计与接口依据

### 4.1 账号工作台：已有接口可直接推进

复用 `listProviderAccountPools`、`listOperationalAccountPools`、`getCredentialMetadata`、现有 CredentialSheet 和失败归因。

- 主列表以 Provider / Channel / Account 为精确身份，主列保留账号、认证、运行时、并发/上限、期限。先提供可解释的筛选，不以“健康分”排序。
- 详情抽屉分“运行状态、账号权益、配置绑定、失败证据”。先显示已有完整列表字段，仅在真正缺少信息时追加 GET；不重启旧计划已经否决的逐行详情预读。
- 权益详情显示域、套餐、来源、置信度和观测时间。枚举使用当前契约；未知域/值显示原值及未识别提示，不映射为相近套餐。
- `expires_at_ms`、`refresh_due_at_ms`、`quota_sync_due_at_ms` 为 nullable；空值显示“无观测/不适用”，不以 0 代替。不从一个时间字段推断“刷新成功”。
- 冷却/请求恢复保留已有确认、精确 account 目标和四态结果。观察到 `reauth_required` 只提示重授权责任，不自动调用 OAuth 或触发 Autoreg。
- runtime pool 的 GET 不带版本，action 带版本；配置 inventory 带版本。界面关联时明确显示来源，不能用一套通用 scope 覆盖这三者。

**验收场景**：Build `supergrok`；Web 独立域；Console/null；ChatGPT/Claude 同名 `free`；auth active + runtime cooling；无权限/503；翻页 snapshot 冲突；未选版本仍能看 runtime pool，但不能发送需要版本的 action。

### 4.2 模型目录：先交付状态，完整可调用清单依赖后端

现在即可交付的“目录状态”使用 `getCatalogStatus`：

| 字段 | 呈现 |
|---|---|
| `endpoint_id + credential_id` | 精确 target，联动到对应账号/端点。 |
| `freshness` | 四态文字与图形；后端结论为准。 |
| `snapshot_version` | 目录版本；缺席时不编造。 |
| `refresh_due` | 后端是否要求刷新；与 Credential 的 refresh due 区分。 |
| `model_count` | 仅表示后端返回的目录计数，不直接命名为“可调用模型数”。 |
| `last_failure_at_ms/class` | 最近失败时间和安全分类；不输出原始 Provider body。 |
| `observed_at_ms` | 观测时间；Missing/failure-only 行不能宣称存在成功观测。 |

主视图使用状态、模型数、最近成功/失败和 target；完整阈值/移除规则放在展开说明。移除现有对“stale 就是超过 24h”的不准确解释：默认 Fresh 到 6h，6h 后 Stale，24h 是 refresh due，72h 才 Expired。相对时间可以由浏览器计算，不能替代服务端 verdict。

**完整模型清单的接口缺口**：管理契约没有与授权后 `/v1/models` 等价的枚举。`listPublicModels` 继续作为配置视图，不替代 effective catalog。需要下方 BE-FE-01；在其实现前只发布目录状态，不展示伪造的可调用模型选择器。

### 4.3 模型与路由配置：已完成工作台继续使用

- 保留 Public Model CRUD、创建 Route/Candidate、校验、Explain。
- 用清楚的步骤显示“模型 → Route → Candidate → Endpoint/Credential → Access Group”；每一步链接到现有编辑入口，不把拓扑校验等同于真实可用性。
- 当前没有候选完整枚举/修改/删除，也没有完整 Route/Alias 列表。新增 BE-FE-02 后再做可编辑表格；此前保留手动 Route ID 和明确的不完整提示。
- 候选的 exact upstream model、channel 和 endpoint 身份不得靠自动加前缀改名消歧。
- Channel Pin 保持单独的真实请求确认；页面导航和刷新不会触发它。验收分别记录目录可见、文本成功、流式工具回传和助手文本历史重放，不把一个测试结果扩展到全部能力。

### 4.4 总览、请求与用量：先做真实信息的编排

- 总览保留累计 attempts、attempt 成功率、管道 backlog/丢失及账本置信度；指标下始终显示“自进程启动累计”或精确时间窗。Attempt 成功率不改名为请求成功率。
- 首屏把异常入口放在指标后，链到带筛选条件的账号、目录或失败列表。任何计数来自分页时，区分已加载数量与全量。
- 账本与失败使用两个视图；请求详情把 `listRequestAttempts` 接成简明时间顺序/步骤列表，只显示契约拥有的状态，不虚构时间轴上的延迟。
- Usage 继续显示六类 token 各自的置信度和不完整覆盖。后台 B2 完成前，不通过 N 次时间窗查询拼趋势图。
- B1 完成前，空账本说明“暂无账本记录，不能据此判断没有消费”；不能由前端猜测物化 worker 的健康。明确物化状态需 BE-FE-03 的后端字段。
- 真正的时间桶、请求级成败/延迟、RPM/TPM 和 P95 进入 BE-FE-03 之后的独立扩展批次。

## 5. 视觉方向：Apple Liquid Glass，完整栏目统一

**用户已明确指定 Apple Liquid Glass，这不是待选择项。** 材质基线是 `web/prism/DESIGN.md`、现有 `tokens.css`、`glass.css`、`PrismLens.tsx` 和 `GlassSurface.tsx`。V3 复用材质配方及透镜几何；V4 按本轮要求调整环境色、灰阶、外阴影、布局和详情尺寸，保留透镜算法、版本材质与降级。具体 token 差异见 V4 交接；它们尚未写入正式前端。前一稿的 Linear 方向不进入后续实施。

CPAMP 继续作为任务组织参考：账号筛选、对象详情与异常到处理对象的导航。参考 [固定源码版本](https://github.com/seakee/CPA-Manager-Plus/tree/1ae656c82990c480f3f104326a08c6e0001eeb4c) 与 [公开演示](https://seakee.github.io/CPA-Manager-Plus/#/demo/accounts)，不复制其后端能力或数据口径。材质与平台层次参考 [Apple Liquid Glass](https://developer.apple.com/documentation/technologyoverviews/liquid-glass)。

| 部分 | 设计要求 |
|---|---|
| 环境与玻璃 | V4 用低饱和蓝灰/青灰环境与微弱棱镜光带，降低 grain 和外阴影。顶栏、侧栏、草稿坞最多三面 chrome 玻璃；保留垂直 tint、受光边环、内阴影与 SVG 边缘折射。 |
| 内容 | 银灰环境、浅灰工作画布、白色面板形成三级层次；深色采用石墨灰。数字、表格、表单和图形放实底；内容在顶栏下滚动，侧栏保持独立导航，文字与控件有足够净空。 |
| 色彩与字体 | Apple 蓝用于操作与焦点；系统 SF/PingFang 字体、等宽数字。正文 14px，表格 13px、主要元数据至少 12px；正常状态用圆点和中性文字，警告/错误保留明确标签与淡色背景。对比度按 V4 新实底重新验证。 |
| 版本材质 | 草稿更厚更柔，活动清澈，归档低饱和；两条渲染路径消费同一组 token。发布成功才换态。 |
| 页面层次 | 顶栏当前位置 → 标题与主操作 → 作用域/观测 → 组合指标 → 数据面板。账号与目录的筛选、列表、结果数量同组；对象详情用右侧 520px 检查面板，编辑/确认居中，手机详情左右各留 12px。参数、错误码和 ID 保持 exact 值。 |
| 完整导航 | 14 页使用同一壳体；桌面矮屏仍能看到全部栏目，手机菜单包含全部入口。移动账号行转卡片，其余长表在自身容器内滚动。 |
| 降级 | 同时支持浅色/深色、减少透明度、增强对比度、减少动态效果；继续保留 SVG 不支持时的 blur fallback 与特异性约束。 |
| 交互 | 搜索、筛选、子视图、详情、草稿编辑与确认都有设计；键盘可达，Escape 关闭，焦点回收。真实写入操作必须保留目标和影响。 |
| 状态与语言 | 加载、真空、过滤后空、投影不可用、管道未接线、读取失败、session 失效各自呈现。中文优先，生产文案继续维护 zh/en 键，不以原型替代既有多语言机制。 |

**OpenDesign 的实际使用方式**：通过运行中 OpenDesign 的 MCP 读取项目、frontend-design、impeccable-design-polish 与 Apple 参考，设计与源文件由本会话模型完成，通过 `create_artifact` / `write_file` 保存和迭代，在原生应用和 raw preview 中检查。未调用 `start_run`，没有 OpenDesign 内部 Agent 生成记录，没有使用模型 CLI、Cloud 或 BYOK，也没有改模型配置。

原型位于现有项目 `Prism Gateway Console — Redesign` 的 `prism-liquid-glass-v4.html`。原有 `index.html`、V2、V3 均保留。它覆盖完整页面设计；真实 API 接入、生产 CSP 与四文件构建仍属于以下实施批次。

## 6. 开发批次、所有权与验收

估算为一名前端开发者的有效工作日，含相关回归与评审；不等于日历承诺，不含后端等待、真实 Provider 配额或生产发布。首个可交付里程碑约 **10–14 天**，已有接口范围收口约 **13–18 天**。

| 批次 | 工作与主要文件 | 依赖 | 估算 | 退出条件 |
|---|---|---|---|---|
| F0 基线修复 | 同步契约；相关 runtime DTO/fixture；修复 session invalid 和跨版本 ETag；统一权威契约检查入口。主要是 `src/api`、`src/session`、`versionStore`、`utils/revision`、`scripts` | 现有后端契约即可 | 2–3 天 | F1–F3 审查问题关闭；权威契约 gate 通过；缺字段和乱序响应回归通过。 |
| F1 完整视觉落地 | 以 V4 的 14 栏目 + 解锁页作为页面清单；合并本轮 token、数据面板与检查面板，复用 GlassSurface、PrismLens 与状态，逐页保留已有能力 | F0 的 scope/状态口径明确 | 3–4 天 | 每个栏目与主要子视图均可用；桌面/窄屏、明暗/系统降级通过；每个指标都有来源或明确待后端。 |
| F2 账号与目录状态 | 抽出 `features/accounts`；账号详情接权益；目录状态接五个新增 operational 字段；复用 Runtime/Credential 组件 | F0、F1 | 3–4 天 | §4.1/4.2 场景覆盖，三个缺失来源不合成全局失败，单个 Fresh 不合成全局健康。 |
| F3 导航与诊断闭环 | 保留筛选上下文；账号/目录/请求/Explain 深链；RuntimePage 拆职责；总览说明收口 | F2 | 2–3 天 | 可从一条失败记录定位到已有 target 与 attempts；所有真实操作仍显式触发。 |
| F4 已有接口质量收口 | 响应型、可访问性、关键 fixture E2E、本地真实网关契约/嵌入验证、更新交接 | F0–F3；B1/B2 若尚未完成则如实保留计费/规模限制 | 3–4 天 | 四文件、CSP、无存储、双构建、真实管理同源验证均有证据；未实现后端能力不算通过。 |

职责：仓库通用分工仍记录为 Claude Code 前端、Codex 后端；用户已针对本轮明确授权 Codex 统一执行前后端，具体以正式实施计划为准。任何实际越界改动仍按根目录规则记录 `cross-boundary-log` 和 commit trailer。本方案本身不启动子 agent，不要求改变既有模型/调用服务。

首批 F0 的顺序：

1. 核对当前分支、已有改动及最新跨边界日志。
2. 执行 `npm --prefix web/prism run sync-contract`，审阅三个 schema 的变化。
3. 更新两个页面 DTO、权益/目录 fixture 和断言，确认不是仅同步 JSON。
4. 修复审查报告中的 F2/F3；把 request 所属版本/session 作为响应处理依据，409 不自动重放。
5. 在前端日常检查中增加权威契约对照。生成器当前不提供 schema DTO，因此需针对变化字段测试；不为本批实现完整通用代码生成框架。
6. 处理已交接的 `models/model.ts` EOF 文案空白问题，并验证 diff。旧 500/503 问题后端已修，不重复修复。
7. 留下可审查的 commit/交接，再开始页面整理。

`versionScoped`/`mutating` 开关可从操作参数推导，是既有提议的独立机械简化；可以在响应归属修复后单独做，不与视觉批次混成一次大改。

## 7. 需要后端配合的三份接口提案

这些是待提交/评审的 CR 范围，不是已可调用端点；准确路径、operationId 和 schema 由后端定稿。前端不增造 generated operation，不用 fixtures 伪装其完成。

| 提案 | 最小后端交付 | 为什么现有接口不足 | 前端解锁内容 |
|---|---|---|---|
| BE-FE-01 有效模型与来源 | 管理鉴权下的 effective model/provenance 投影；返回 serving snapshot/config revision，exact model、Provider/channel/endpoint/credential 来源、目录状态/时间、歧义/不可选原因；支持按已存在 Client Key ID 或 Access Group 做授权预览，复用数据面逻辑，无需浏览器提供 Client Key secret | `listPublicModels` 是配置；catalog status 只有 target 状态/数量；跨 listener 直接读 `/v1/models` 违反现有调用边界 | 真正的模型选择器、配置与可用目录对照、模型来源详情。 |
| BE-FE-02 配置图与候选维护 | 有界、revision 一致的 Route/Candidate/Alias 读取，能列出未绑定/孤立草稿资源；候选更新/删除或同等明确的集合替换操作，携带 If-Match/审计/校验 | 运营 inventory 只给连通记录；现有候选只能创建，不能完整读取或纠错 | 完整的模型/路由工作台和绑定导航。 |
| BE-FE-03 观测与处理状态 | 先接 B1 并提供账本 checkpoint/lag/error 的安全状态；解决 B2；然后定义 request-level outcome/latency、固定时间桶、范围/时区/coverage、统计水位和有界摘要 | 账本不等于请求成功记录；失败列表不是全集；Prometheus 累计值没有历史时间桶 | 实际请求趋势、延迟分布、窗口成功率与更完整的成本分析。 |

后端建议顺序：B1/B2 优先，B3/B4 保证持续运行边界；BE-FE-01/02 随 P13-15 与配置工作确定契约；BE-FE-03 的图表部分最后。BE-FE-01/02 接口定稿后前端额外约 4–7 天；BE-FE-03 的分析界面约 4–6 天，均不含后端实现。

## 8. 验证与完成定义

每批只运行受影响检查；F4 统一做完整前端验证，不在每次文案修改后反复跑全仓。

- 类型、DTO/fixture 与权威契约一致；新枚举有明确映射，未知值和 `null` 有独立呈现。
- 前端单测覆盖版本 A/B 乱序、同版本 revision 倒退、鉴权拒绝后的锁定/缓存失效、missing/expired、entitlement domain/tier 组合、opaque cursor 冲突。
- 将 V4 的 14 个栏目逐项映射到实际路由与原组件，V3 保留为交互清单依据；主导航不能留下占位或禁用项。E2E 选关键用户路径，并覆盖 390px、键盘、暗色及系统降级。fixture 使用可控时钟，不把已过期日期硬标 Fresh。
- 构建产物仍恰好是 `index.html`、`assets/main.js`、`assets/vendor.js`、`assets/index.css`。不得为了路由懒加载产生无人服务的第五个文件；构建体积以此次 663,932 字节未压缩为基线比较，不宣称这是性能指标。
- 运行 `node scripts/check-management-spa.mjs` 作为权威契约 + 前端双构建门禁；不能只报告局部 `npm run check`。
- 关键读取/写入路径在本地真实 gateway 的管理 listener 验证，使用合成配置、临时状态目录与 loopback mock Provider。fixture、真实本地网关与外部 Provider 验收分别记账。
- 真实部署仍通过嵌入 Prism 的完整 gateway artifact；Oracle 安装/重启/流量或真实 Provider 调用使用具体的另行授权，不因本计划或页面完成而自动进行。
- 交付报告列出已完成、依赖后端、未验证和外部延期项。新的接线进度按“字段/状态/用户任务/真实来源”记，不再仅以 operation 数量作为完成证明。

本轮继续保留的暂缓范围：全量正文英文、在线恢复备份、秘密导出、Autoreg 控制面、未支持媒体协议、真实 Provider 自动批量巡检，以及缺乏服务端口径的趋势/百分比图表。
