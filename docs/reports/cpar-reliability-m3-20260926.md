# CPAR M3：V2 日常管理、诊断与恢复

日期：2026-09-26。范围为[可靠性计划](../handoffs/cpar-reliability-alignment-plan-20260926.md)的 M3，承接 M2 `1a2fdb8`，本轮开始 HEAD `065c3f2`。实现提交与本报告、跨边界日志同批保存。

## 1. 结论与边界

M3 的应用恢复、分层读取失败、会话竞态、安全 attempts 诊断、写入结果核对和 V2 本地交互已通过验收。正式嵌入 SPA 使用真实 gateway、独立临时状态和 loopback TLS mock；没有使用生产凭据，没有新增真实推理，也没有部署。schema 仍为 M2 的 30。

这不是整轮 M0–M4 完成：大样本查询优化、隔离生产副本升级/回退、签名候选、最多 12 次受控真实推理、Oracle 发布和生产登录后复验属于 M4。官方登录、真实账号元数据不能由本轮合成账号或 fixture 结果代替。

## 2. 按验收编号交付

| 编号 | 实现与实际验证 | 证据 |
|---|---|---|
| M3-01 应用错误 | 解锁页和应用根路由加入恢复边界；显示安全问题编号，不渲染原始堆栈。对真实 gateway 的读取响应注入非法列表，实际触发组件渲染异常；恢复同一路由、保留筛选且不重放写入 | [RouteRecovery](../../web/prism/src/app/RouteRecovery.tsx)、[恢复回执](evidence/cpar-reliability-m3-20260926/recovery.json)、[截图](evidence/cpar-reliability-m3-20260926/render-recovery.png) |
| M3-02 读取恢复 | 首次/下一页/后台刷新分开；保留旧记录、筛选及最后成功时间。请求第二页失败保留 50 行，重试仅请求该 cursor，读到 57 行；后台失败保留全部 57 行。账本保留 54 行，失败归因保留 1 行；首次失败也有重读入口 | [共享状态](../../web/prism/src/components/PagedReadStatus.tsx)、[请求回执](evidence/cpar-reliability-m3-20260926/recovery.json)、[账本/失败回执](evidence/cpar-reliability-m3-20260926/monitoring-recovery.json) |
| M3-03 会话与并发 | 新增 401 分类与清理回归，沿用 session generation/revision ownership；修复真实撤销会话与路由并发导致的空白。immediate/settled/held 三时序均回到登录、清除管理画布。409/丢响应需核对，不自动重复变更 | [会话源码](../../web/prism/src/app/AppShell.tsx)、[即时](evidence/cpar-reliability-m3-20260926/session-immediate.json)、[稳定后](evidence/cpar-reliability-m3-20260926/session-settled.json)、[延迟交付](evidence/cpar-reliability-m3-20260926/session-held.json)、[fixture 写入](evidence/cpar-reliability-m3-20260926/fixture-native.json) |
| M3-04 安全诊断 | 持久 Attempt 按准确 ID/归属/结果投影编号、时间、耗时、闭集错误与重试决定；缺证不补序号或零。成功 attempt 标明“已建立上游响应”，不冒充外部请求终态。提供账号/Provider/目录/诊断深链；记录系统准入和积压状态进入总览，缺失指标为未知 | [CR](../change-requests/CR-20260926-m3-safe-attempt-diagnostics.md)、[AttemptTimeline](../../web/prism/src/features/monitoring/AttemptTimeline.tsx)、[投影回归](../../crates/gateway-http-actix/tests/m3_attempt_observation.rs)、[真实请求详情](evidence/cpar-reliability-m3-20260926/attempt-detail.png) |
| M3-05 操作闭环 | Provider 取消/继续编辑，长模型表单脏状态，受限 Key 保存应用后重读，价格输入校验均实际操作。真实 gateway 批量停用第一项已保存待应用、第二项注入 409、第三项未执行；总写入 2、提交成功 1、发布 0。原生账号结果不明/409/待应用及 OAuth 轮换丢响应使用 fixture 验证，未伪装真实授权 | [主要操作](evidence/cpar-reliability-m3-20260926/workflows.json)、[批量](evidence/cpar-reliability-m3-20260926/batch.json)、[原生 fixture](evidence/cpar-reliability-m3-20260926/fixture-native.json)、[轮换 fixture](evidence/cpar-reliability-m3-20260926/fixture-refresh.json) |
| M3-06 V2 视觉 | EgoLite 检查八工作区 48 状态、解锁 6 状态、授权选择/模型/密钥三类弹窗 18 状态；1440×900、1280×720、390×844，深浅色。另查 Tab/Shift+Tab 焦点限制、Escape 脏表单保护、长 exact ID、空态、错误态、辅助偏好 | [工作区](evidence/cpar-reliability-m3-20260926/workspaces.json)、[弹窗](evidence/cpar-reliability-m3-20260926/dialogs.json)、[解锁](evidence/cpar-reliability-m3-20260926/unlock.json) |

### 八工作区实际操作

| 工作区 | 本轮核对的操作 |
|---|---|
| 仪表盘 | 真实请求数与终态、记录健康；刷新失败保留观测值和原时间深链，成功后才推进相对时间范围 |
| 账号管理 | 渠道选择、Kimi 授权前入口不要求无关 Provider/endpoint；运行状态精确定位；详情、批量部分失败、取消与完成后重读 |
| AI 提供商 | 列表/详情和页内编辑；取消触发未保存保护，“继续编辑”保留输入；fixture 验证轮换结果不明时禁止重复轮换 |
| 模型管理 | 目录分页/exact ID 与手动开放边界；搜索空态，超长模型 ID 表单、Escape 与放弃；不因发现目录扩权 |
| API 密钥 | 创建仅允许当前 exact 模型的密钥，保存/应用/重读；一次性密钥关闭后清除，无浏览器持久化 |
| 请求与日志 | 首屏、第二页和后台错误注入；持久请求详情及 attempts；账本、失败归因重读保留数据 |
| 用量与价格 | 读取合成用量及费用置信度；价格表 11 个必填缺失时禁止预览，编辑后离开有保护 |
| 设置与高级维护 | 深浅主题、降低透明度/减少动效/增强对比度；真实退出与会话撤销；高级入口保留 |

所有截图均为合成数据。源码沿用 V2 token、玻璃导航和实底数据面板，本轮只增恢复界面与诊断细节，没有重新换风格；使用 frontend-design、ui-ux-pro-max、frontend-ui-craft、React best practices 审核布局与交互，没有调用外部设计模型。

## 3. 本轮实际发现并修复的问题

1. **真实会话撤销后空白**：此前的 `Navigate` 绝对目标不随位置改变；并发导航覆盖第一次跳转后，该组件 effect 已消费，页面可能停在 settings 且 appChildren 为 0。按 location key 重挂锁定跳转后，三时序通过。[首次失败回执](evidence/cpar-reliability-m3-20260926/session-immediate-failure.json)保留，不用成功结果抹去发现过程。HTTP 拒绝在隐藏管理接口模式为 404；独立单测也覆盖标准 401。
2. **后台刷新与分页丢展示**：原 `isError` 分支遮掉旧数据；独立 recovery 状态保留快照。总览/请求相对时间随成功快照保存，失败不悄悄变更指标深链范围。
3. **账本/失败状态接线遗漏**：最终静态复核查到两处删除旧错误分支却漏放共享状态组件，修复后补跑真实 gateway 首屏/后台故障，不仅检查类型。
4. **诊断深链与缺证**：目录原本忽略 endpoint/credential 参数；账号精确目标可能不在首批 100 条。现在目录按目标过滤、账号有界继续分页，保留历史不存在的空态；无观测时不按数组次序猜 attempt 编号。
5. **写入不明可重复提交**：原生账号与令牌轮换新增提交闩锁，只有明确校验拒绝可直接修正后再提；409/网络失联提示核对。重读和运行配置应用与原写入分开。

## 4. 本次验证结果

- 前端类型检查、构建与 **413 项单测 / 56 个文件**通过。没有把旧测试数量作为本次证据。
- `scripts/check.sh fast` 全部步骤通过：**1,352 项 Rust 测试，0 失败，12 项原有 ignored**；strict Clippy、契约、文档链接、边界、秘密扫描、嵌入四文件与确定性构建均通过。[逐步门禁](evidence/cpar-reliability-m3-20260926/fast-check.md)
- 文档/证据暂存后再执行 `scripts/check.sh docs`，链接、契约引用、计划、秘密扫描与 whitespace 通过；新验收脚本语法及源码/包边界检查通过。[文档收口门禁](evidence/cpar-reliability-m3-20260926/docs-final.md)
- 新增 Rust observation 2 用例及 runtime 持久投影断言；不新增写 API、数据库迁移或依赖。权威 OpenAPI 修改后运行 `sync-contract`，前端 vendored contract 由脚本更新。
- 从实际嵌入网关读取四个资产，逐个 SHA256 与最终 dist 一致；保留 CSP/缓存头。[资产回读](evidence/cpar-reliability-m3-20260926/embedded-assets.json)
- 真实 gateway + loopback：JSON/SSE 成功、上游 HTTP 失败、截断流、客户端取消共 5 请求；成功用量进入账本。目录 3 模型/2 页，开放仍为 1。再补 52 条合成成功请求，形成 57 请求 / 54 账本供分页交互验收。[链路](evidence/cpar-reliability-m3-20260926/request-chain-acceptance.json)、[种数](evidence/cpar-reliability-m3-20260926/pagination-seed.json)
- fast 门禁另执行四组三轮 Agent loopback 回归，覆盖 JSON/SSE × 显式/存储历史，以及工具结果、reasoning、失败/截断/取消。全部是本地 mock，真实调用额度保持 **0/12**。
- 浏览器仅使用 **EgoLite / TaskSpace 12**，未用 Safari/Playwright 替代。原生渠道故障与轮换的 UI fixture 仅证明前端恢复语义；Kimi 官方开始授权未点击，真实官方登录留在 M4。
- 请求详情中的账号、提供商、模型目录、运行诊断四个链接逐一实际点击；核对单账号目标、提供商工作区、目录精确过滤和诊断对象提示。[四个深链回执](evidence/cpar-reliability-m3-20260926/diagnostic-links.json)
- 首两次 fast 因新增测试中的 unwrap/错误类型转换未满足编译和 lint 要求而停止，修正后完整重跑。[第一次](evidence/cpar-reliability-m3-20260926/fast-gate.md)、[第二次](evidence/cpar-reliability-m3-20260926/fast-gate-final.md)。不把未结束的门禁写成成功。

### 交互计时的限制

使用 HEAD `065c3f2` 的独立前端源码副本与当前候选、同一 EgoLite p2/1440×900/浅色/空账号 fixture，对“导航到账号”和“打开渠道选择”预热1次后各测5次。阈值保持“中位数同时退化超过10%和50ms，且可复现”。[首轮](evidence/cpar-reliability-m3-20260926/ui-timing-first.json)曾超阈值；[第二轮](evidence/cpar-reliability-m3-20260926/ui-timing.json)没有复现，候选导航185.8ms、弹窗297.5ms，基线分别1160ms/2205.5ms。

不能据此宣称加速：两版样本都有约1秒的帧/动画等待波动。专项观测中 DOM 在4.9ms出现，但240ms的CSS动画结束等待到1255.8ms，下一帧约1003.6ms才交付；两个版本的弹窗CSS没有变化。[观测记录](evidence/cpar-reliability-m3-20260926/timing-investigation.json)。结论限于本次未复现稳定退化，不是生产时延、Core Web Vitals或M4容量门禁。

## 5. 复跑和证据范围

脚本与执行入口见 [acceptance README](../../scripts/acceptance/README.md)。使用独立合成状态，私有密码只存该临时目录；截图/回执不保存口令或密钥。先完成真实控制器登录/改密和种数，再依次运行恢复、布局、弹窗、批量、会话脚本。不可并行操作两个标签页来测同一页的轮询/动画时间。

[证据目录](evidence/cpar-reliability-m3-20260926/)保存全部布局/行为 JSON、门禁、四文件 SHA256 和 25 张代表截图。完整原始截图与执行日志保留在本机 `output/m3-local-20260926/`；布局 JSON 中其它截图文件名对应本机原始目录，没有声称所有原图均入库。完整矩阵在最后的会话跳转修复前采集；该修复后专门重跑三会话时序，账本/失败提示也单独复验，未重复无变化的全部截图。

M4 继续：10万/100万数据与精确聚合优化 → 隔离生产副本迁移/回退 → 同候选正式门禁与签名 → 按登记最多12次真实推理 → Oracle 发布及生产登录后的复验。没有将真实渠道依赖标为已完成。

验收后退出合成管理员会话，关闭本轮 EgoLite 空间与自建 gateway/mock/Vite 服务，保留私有临时状态和原始证据；没有停止其他任务或生产服务。[清理回执](evidence/cpar-reliability-m3-20260926/cleanup.json)
