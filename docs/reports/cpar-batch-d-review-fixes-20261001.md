# CPAR D：评审修复与补充验收（2026-10-01）

## 范围与固定版本

本次修复用户要求“修复一下当前codereview指出的问题”，沿用明确授权“授权 Codex 完成本次 D 的前后端”。父规格 [#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)，D 为 #51–56 / CPAR-42–47。固定比较起点仍为 `ff23f2d8b838b8e5189222d4433acf53c92308e4`；修复前 HEAD 为 `2fa1a4240c6a8f5728353690c77206287a52d9cf`。

- 代码与补测提交：`d1ee7702937a87777cc53e3d0de392d1e2528b84`。
- 最终验收脚本提交：`7478d276ea1fb017491c1e3f9c34f666bbf3fc10`；相对 `d1ee770` 仅修改验收脚本的响应观测与草稿读回流程，`web/prism` Git tree 相同。
- 最终生产代码提交：`d837056bbc4eb29bdf52594b4db4b8ee18f4f70b`；关闭独立 Standards 终审新增的栏目搜索文案 P3。相对 `7478d27` 只替换两处文本为 zh/en 消息包并追加跨边界记录，交互逻辑无变化。
- 没有修改后端、权威契约、生成客户端、schema、权限范围或渠道能力；已有用户 `AGENTS.md` 与无关未跟踪产物保留。
- 前端修改的精确文件与授权在 [同提交跨边界记录](../cross-boundary-log.md)；`d1ee770`、`d837056` 均含 `Cross-Boundary:` trailer。
- [首次 D 报告](cpar-batch-d-20261001.md)及其 `353c0ec` 证据保持历史语义；本报告的结果不覆盖 A/B/C 或真实 Provider、远端 CI、部署与生产证据。

## 修复与评审项闭环

| 评审项／补测发现 | 修复与实际行为 | 回归证据 |
|---|---|---|
| Spec P2：模型来源拓扑首次读取失败被显示为空连接 | 只有 topology 成功时才显示“尚未连接提供商”；初读失败保持未知，重试可恢复，刷新失败保留有时间的旧来源。此问题在固定起点前已存在，本次按 D 的状态准确要求修复，不能称 D 新引入回归。 | [model-connections](assets/cpar-batch-d-review-fixes-20261001/model-connections.json)，4 项 |
| Spec P2：失败归因 tab 未有 D 浏览器操作证据 | 对 `tab=failures` 补充查询／筛选、尝试详情、精确绑定诊断、返回、恢复与分页；页面清单同时加入该 query 工作区。 | [failures](assets/cpar-batch-d-review-fixes-20261001/failures.json)，16 项；[inventory](assets/cpar-batch-d-review-fixes-20261001/inventory.json)，69 项 |
| Spec P2：模型／路由编辑与返回未有 D 浏览器证据 | 从公开“编辑当前配置”创建并接续合成草稿，经真实 UI 维护来源、公开模型和路由；覆盖无匹配、来源筛选、导航返回、保留编辑／放弃、拒绝、保存、回执与读回。 | [models-routes](assets/cpar-batch-d-review-fixes-20261001/models-routes.json)，21 项 |
| Standards P3：出口／运行表格有重复滚动声明 | 相同选择器和声明合并到 `app.css`，保持 wrapper 承担横向滚动；两处局部重复规则删除。 | 桌面／375px、light/dark 的页面清单复核 |
| Standards 终审 P3：栏目搜索空态／清除按钮硬编码中文 | 保留原中文，将两处文本放入 zh/en `navigation` 消息包，并使用页面已有的 `useMessages()`；搜索匹配、清除与输入焦点处理保持原样。 | `d837056` 增量独立 Standards／Spec 静态复核均 PASS，最终类型、469 单测与 Full 通过；该文案补丁未重跑 EgoLite。 |
| 补测：带筛选的失败列表为空，被写成整个配置版本无失败 | 按已应用的 filters 区分“无匹配”与无筛选的成功空结果；新增文案使用 zh/en 消息包。未提交的表单输入不改变结果的解释。 | 失败 no-match 红灯及修复后回归；[i18n](assets/cpar-batch-d-review-fixes-20261001/i18n.json) 经实际设置切换英文 |
| 补测：同一 Sheet 从来源清单进入编辑时焦点离开对话框 | 标题所代表的步骤改变且焦点已离开 Sheet 时，把焦点放回 dialog；保留在对话框内的有效焦点，不改原有关闭、放弃与并发保护。 | 来源编辑的焦点红灯；桌面／窄屏、双主题的来源与路由编辑、Escape 和触发按钮焦点返回 |
| 报告把 Markdown 文件数写成链接数 | 首次 D 报告的“798个Markdown链接”更正为“798个Markdown文件”，与原检查脚本输出单位一致。 | 文档静态检查与最终 diff |

三个可观察状态缺陷先有工作树红灯，再有修复后的绿灯。[红灯证据目录](assets/cpar-batch-d-review-fixes-20261001/red/model-connections.json)另保留 [失败列表红灯](assets/cpar-batch-d-review-fixes-20261001/red/failures.json)与[焦点红灯](assets/cpar-batch-d-review-fixes-20261001/red/models-routes.json)。其 commit 字段明确是代码基线加回归脚本／工作树状态，不冒充固定提交的正式 Full 回执。

新增 `lost-after-write` 在本地 fixture 的原始管理写入已返回成功之后丢弃响应，随后通过公开草稿入口和编辑表单读回 `max_attempts=3`，`updateRoute` 次数没有增加。它证明此合成路径的“已执行、响应丢失、读回不重放”；原有 `lost` 模式仍是调用原始请求前抛错，只证明 UI 的未知结果保护，不升级为真实服务器或 Provider 的写入完成证据。

读回较新的 ETag 会使已有接续确认的 owner 失效，这是现有并发保护。最终脚本观察读取完成和旧确认退出，再做一次明确的只读选择，保留原 revision／selection 保护。初次完整执行的 [失败回执](assets/cpar-batch-d-review-fixes-20261001/attempts/d1ee770-models-routes.json)保留：前 19 项通过，最后失败源于验收脚本假设旧确认始终保留，随后只修正脚本。另补上公开控件的可用条件和显式主题结果等待；未通过私有 React/Zustand 状态替代用户操作。

## 最终验证

以下记录按各自实际执行的不可变提交保存，不重新标注旧结果的 SHA。15 个场景和首轮完整前端单测在 `d1ee770` 执行；仅改变草稿读回预期后，模型／路由场景在 `7478d27` 重跑。两提交的 `web/prism` Git tree 相同。最后的文案补丁改变了前端 tree：最终单测和正式 Full 在 `d837056` 重新执行，浏览器结果继续保留在原两提交，不能称为新 HEAD 的英文栏目搜索 UI 验收。

| 层级 | 状态 | 实际证据与边界 |
|---|---|---|
| 前端完整单测 | **PASS** | 最终 `d837056`，`npm --prefix web/prism test`，66 文件／469 项，exit 0。[metadata](assets/cpar-batch-d-review-fixes-20261001/final-copy/frontend-unit.json)、[日志](assets/cpar-batch-d-review-fixes-20261001/final-copy/frontend-unit.txt)。[首轮](assets/cpar-batch-d-review-fixes-20261001/frontend-unit.json) 保留在 `d1ee770`。 |
| 类型检查 | **PASS** | 最后文案修复后 `npm --prefix web/prism run type-check` exit 0；最终 `d837056` 正式 Full 另覆盖 Management SPA 类型／双构建门槛。 |
| EgoLite | **PASS** | 16 场景／167 检查；15 场景146项在 `d1ee770`，模型／路由21项在 `7478d27`。逐场景 SHA 和相同前端 Git tree 保存在 [summary](assets/cpar-batch-d-review-fixes-20261001/browser-summary.json)；[完整输出](assets/cpar-batch-d-review-fixes-20261001/browser.txt)。 |
| 仓库正式 Full | **PASS** | 最终 `d837056`，44/44，exit 0；Rust 1432 passed／0 failed／12 ignored，Darwin27/arm64，2026-10-01 05:13:20–05:16:49 UTC，约209秒。[metadata](assets/cpar-batch-d-review-fixes-20261001/final-copy/full-summary.json)、[44步回执](assets/cpar-batch-d-review-fixes-20261001/final-copy/full-check.md)、[完整输出](assets/cpar-batch-d-review-fixes-20261001/final-copy/full.txt)。[前轮 Full](assets/cpar-batch-d-review-fixes-20261001/full-summary.json) 保留在 `7478d27`。 |
| 最终文档与附件静态检查 | **PASS** | docs 7/7，exit 0；[metadata](assets/cpar-batch-d-review-fixes-20261001/final-copy/docs-summary.json)、[7步回执](assets/cpar-batch-d-review-fixes-20261001/final-copy/docs-check.md)、[输出](assets/cpar-batch-d-review-fixes-20261001/final-copy/docs.txt)。在代码 HEAD `d837056`、本次报告／证据工作树执行；回执链接补齐后另检查文档链接、暂存密钥和空白，不声称报告提交重新执行 Full。 |
| 本次临时环境收尾 | **PASS** | TaskSpace33 finish 原始回执、自有 Vite PID98021 的关闭及5188无监听见 [cleanup](assets/cpar-batch-d-review-fixes-20261001/cleanup.json)。 |
| 真实 Provider／远端 CI／Linux 发布产物／部署／生产验收 | **NOT_RUN** | 本次范围是 D 本地修复；没有外部写入、收费调用或共享服务状态改变。 |

最终正式命令为 `CARGO_NET_OFFLINE=true CHECK_REPORT_PATH=docs/reports/assets/cpar-batch-d-review-fixes-20261001/final-copy/full-check.md bash scripts/check.sh full`。执行前后 HEAD 均为 `d837056bbc4eb29bdf52594b4db4b8ee18f4f70b`。Full 包含源码政策、契约与生成客户端、SPA 类型／双构建、严格 Clippy、全工作区 Rust tests、实际 gateway 自有 loopback 回归、密钥／依赖／RustSec 检查；不包含 Vitest 或真实 Provider。日志仅规范化行尾空白、换行和终端颜色，不改变结果或删除失败输出。

[附件一致性回执](assets/cpar-batch-d-review-fixes-20261001/evidence-integrity.json)核对31份 JSON、34张 PNG、167项逐场景计数、各自 SHA、规格正文 hashes、脚本 hash 及 AGENTS.md 保留状态，并列出当时附件的 SHA-256。红灯和首次失败记录保留；哈希仅用于本地一致性检查，不是外部签名或新的浏览器执行。

浏览器检查使用同一 TaskSpace33、EgoLite 实际页面、本地 Vite 和生成 ManagementApi 的管理 HTTP seam。仅合成 fixture，UI 写入、筛选、取消与返回走公开控件；请求观测仅保留 operation、cursor、响应 status，不含 body、凭据或真实账号材料。库存可见项为16个工作 path 加 failures query 工作区，在1440/375px × light/dark 组合检查68次，另有812×375横屏/reduced-motion；并未新增路由。

新补测分别覆盖 initial loading/error、成功空结果、筛选无匹配、权限拒绝、刷新保留成功快照、busy、局部已加载、opaque cursor 重试、编辑取消、确认写入、未知结果、明确读回、焦点、键盘、桌面／窄屏与两种主题。这里的模型／路由编辑是代表流程，不是每个现存表单／渠道变体的穷举；原有历史差异截图仍只证明布局／焦点／Escape，不能当成选择比较基线后的 diff 计算验收。

主线程检查了 [窄屏失败归因](assets/cpar-batch-d-review-fixes-20261001/failures-375-light.png)、[窄屏尝试详情](assets/cpar-batch-d-review-fixes-20261001/failure-attempts-375-dark.png)、[窄屏来源编辑](assets/cpar-batch-d-review-fixes-20261001/model-source-edit-375-dark.png)和[桌面路由编辑](assets/cpar-batch-d-review-fixes-20261001/route-edit-1440-light.png)的实际图像，核对可见控件和视口边界；其余同家族尺寸／主题图片保留。

## Standards

**PASS；未解决 documented-standard 发现 0，Fowler heuristic 发现 0。**最终结论来自两个明确区间的独立只读复核：[两轴记录](assets/cpar-batch-d-review-fixes-20261001/review-summary.json)。

完整 `ff23f2d...7478d27` 复核覆盖46个 Prism changed-file hunks、CSS 增量删除、完整408行 D 验收脚本及 A 验收脚本的一处断言变化。该轮重复 CSS 和历史 Usage 文案修复成立，但仍发现 `SettingsPage.tsx:101` 栏目搜索空态／清除按钮硬编码中文的 P3，依据 `DESIGN.md:547–550,2004`。`d837056` 将两处文本接入现有反应式消息包。

另一位干净探子完整核对 `7478d27...d837056` 四文件增量及必要邻接代码，确认 Pack 同形、原中文和清除／焦点逻辑保持、同提交跨边界日志与 trailer 齐全，返回增量 PASS。旧 PARTIAL 评审和中断后未返回终态的探子结果不补写为 PASS；本次结论依赖上述已返回 complete 的覆盖链。探子未执行测试或浏览器，最终验证由主线程执行。

## Spec

**PASS（限定本轮 D diff）；未解决 missing／partial 0、scope creep 0、实现错误 0。**已读取 GitHub #9／#51–56 完整正文；读取时间和正文 hashes 见 [规格源记录](assets/cpar-batch-d-review-fixes-20261001/spec-sources.json)。

完整 `ff23f2d...7478d27` 复核覆盖 D 生产／单测 changed hunks 与完整408行验收脚本，集中复核 `2fa1a42...7478d27`。模型拓扑初读错误空态已纠正；#52 的 failures16项和 #54 的 models-routes21项操作证据补齐。新增失败筛选空态、Sheet 步骤焦点及共享滚动声明符合 #9；Usage 反应式文案、运行应用未知结果先读回和恢复回执独立保留的既有修复再次核实成立。`lost-after-write` 的 fixture 已执行写入后丢响应、读回不增写，脚本未放宽现有 owner／revision 保护。

另一位干净探子完整核对最后四文件 `7478d27...d837056` 增量，依 #9 的搜索／返回／焦点和 #56 的保留设置能力要求，确认仅替换文案节点，返回增量 PASS。[两轴记录](assets/cpar-batch-d-review-fixes-20261001/review-summary.json)保留各区间、探子及证据限制。代表流程不证明全部渠道／表单变体，历史 diff 截图不证明比较计算；本轮 PASS 不关闭 C、#57、父 #9 或真实／生产验收。

## #57 前的剩余项

本次 D 的修复与局部门槛不能关闭 [#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57) 或父规格 #9。保留 [C 的实际台账](cpar-batch-c-20260930.md)，不新增能力例外、不关闭 capabilities 以取得绿色结果：

1. **BLOCKED：C 的软件／协议缺口。**Kiro 原生输出 hard cap、Grok Web 原生多轮连续性、Official/Console 尚未保真的原生 metadata。
2. **BLOCKED／NOT_RUN：所承诺的真实组合和声明扩展。**逐11渠道三协议、对应模型／账号／参数及 reasoning、parallel、stored、continuation、compact、WS 等实际声明逐项形成证据；C 已记录的失败与环境阻断保持原分类，新构建真实调用未执行。
3. **#57 的全范围证据核对待做。**将88故事、F01–F08、T01–T15 与 A/B/C/D 的审查、兼容／数据边界、本地、EgoLite、真实与正式门槛逐项关联；缺少必要条件的组合不能被已通过样例替代。已有各批证据继续保留其具体提交与环境。
4. **NOT_RUN：发布候选后的实际部署及生产验收。**按 #57 准备影响、回退和候选；只有授权具体目标／范围后才执行部署。当前通过不能称线上已加载 D。

未修改后端或 schema，回退依据为 D 代码提交链；无需数据迁移。报告和附件后续提交只记录证据，不声称其文档 HEAD 重新执行了 Full。
