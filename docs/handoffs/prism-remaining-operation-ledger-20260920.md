# 剩余计划逐操作台账

日期：2026-09-20。范围与优先级以 [确认计划](prism-remaining-development-plan-20260920.md) 为准。下表将“已有能力”“本轮改动”“最终验收”分开；已有实现不代表本轮全量通过。当前功能检查点见 [报告](../reports/prism-remaining-checkpoint-20260920.md)。

| 工作区 / 操作 | 实现与当前状态 | 本轮完成门槛 |
|---|---|---|
| 全局 / 简单表单与只读详情 | 既有 Sheet 统一框架；本轮增加继承导航所有权的确认面板 | 保留独立授权清理、焦点、会话与单遮罩回归 |
| 全局 / 复杂页内操作 | 新增 OperationBoundary / InlineWorkspace；计费采用并已提交 503b740 | 关闭、同页 query、Back、忙碌、会话和错误恢复 |
| 全局 / 待应用批次 | 精确草稿身份、显式恢复入口与策略→密钥→发布链路已实现 | 合并差异与显式接续已提交 26521c2；真实网关应用/回滚/409及接续导航已验，全栏目整合仍待完成 |
| 仪表盘 / 时间、请求与费用指标 | 已有 RequestOverview 与费用范围联动 | 本轮加载/未知/深链、三尺寸和性能整体验收 |
| 账号 / 首次授权与重新授权 | 既有渠道专用入口、回执与取消逻辑；已批准批次保留 | 全渠道支持范围核对，失效/迟到响应及回退回归 |
| 账号 / 导入与凭据维护 | 既有身份展示、导入、更新、启停与任务回执 | 对应原生/配置作用域、部分成功与关闭保护整合回归 |
| 账号 / 批量、详情、健康 | 已有统一投影、筛选、只读详情 | 视觉统一、分页/长内容/未知状态实际检查 |
| 提供商 / 创建 | ProviderDialog 已采用统一表单 | 纳入整合验收，不重写已有接入逻辑 |
| 提供商 / 顶层编辑与删除 | 顶层编辑/删除已迁移固定底栏、捕获基线、禁止重放及待应用回执 | 22 条相关回归及真实网关编辑不提前生效已验；删除真实网关与全站视觉整合仍待验 |
| 提供商 / 接口、连接、账号子资源 | 既有已批准 providerResourceTask 与子资源工作区 | 保留协议/主机、引用检查、跨版本与部分结果回归 |
| 模型 / 上游目录、刷新与接入 | 既有 exact ID、多来源、手动开放 | 目录语义及未开放拒绝不变，整合视觉与错误态 |
| 模型 / 路由、候选、别名 | 既有已批准高级任务与回执 | 复杂编辑迁页内；原查询/草稿与任意能力键保护 |
| API 密钥 / 创建与权限维护 | 既有显式权限、私有组、一次性秘密；待应用链路已验证 | 统一草稿按钮语义、秘密消除和跨工作区回归 |
| API 密钥 / 高级组 CRUD | 已迁移独立维护表单、固定底栏、基线校验、待应用回执与非重放恢复 | 创建/编辑/删除及历史限制回归已验；真实网关新建组已验，最终视觉及真实删除待验 |
| API 密钥 / 路由授权、按组签发 | 路由授权已补固定底栏、待应用回执与恢复；按组签发沿用既有所有权 | 授权写入回执与实际网关重读已验；全栏目整合验收待完成 |
| 请求 / 终态、趋势、延迟、筛选 | 已有真实记录；本轮修复旧 E2E 口径与入口，9 条通过 | 整合三尺寸、未知历史与错误态 |
| 请求 / 账本、尝试与失败深链 | 已有分离口径、脱敏导出；诊断对象改用统一名称 | 详情框统一底栏、完整请求链路与导出回归 |
| 用量 / 时间与维度分析 | 既有真实聚合与六类 token 置信度 | 本轮整合筛选、费用未知、分页/长内容检查 |
| 价格 / 导入与恢复 | 全局保存与配置发布分离；真实本地导入已验 | 历史局部修正已获批准；最终真实恢复/生效时间与历史保留回归 |
| 价格 / 编辑与完整预览 | 页内、每页50、全量 backing model、错误定位 | 1/50/51/512、513拒绝、1024差异、冻结内容回读已验；最终视觉适配待完成 |
| 价格 / 容量与并发 | 后端原子容量256修复；Store/服务/HTTP/前端回归已验 | 契约同步与最终发布门禁；不清理历史目录 |
| 价格 / 路由价格策略 | 已有活动上下文准入草稿与单批次检查 | 跨草稿拒绝、显式合并差异与应用交付 |
| 设置 / 出口策略 | 出口策略已迁入页内，捕获修订、保留原始字段并返回待应用回执 | 页内编辑与真实网关策略保存已验；最终整合及视觉验收待完成 |
| 设置 / 代理池、节点、绑定 | 代理池/节点/绑定已迁入页内，固定操作区、禁止重放与精确草稿恢复 | 14 条相关回归、真实网关新建池/节点/绑定及敏感输入清理已验；实际代理连接不在本轮验收范围 |
| 设置 / 配置版本与差异 | 既有发布/回滚回执；创建补 busy | 复杂差异页内、摘要与明确应用已实现并通过局部真实网关验收；最终整合待完成 |
| 设置 / 审计与备份预检 | 既有只读/预检能力，不新增在线恢复 | 导航、错误、详情动作与真实约束检查 |
| 设置 / 外观、辅助、会话 | 既有内存偏好与管理员会话 | 深浅色、减透明/动效、键盘、失效清理验收 |
| 解锁页 / 登录与改密 | 既有管理员登录；不新增鉴权方案 | 真网关登录、失效返回与移动端验收 |
| 全局 / 性能 | 512条同数据五次开发环境对比已记录 | 最终视觉整合后同方法验证，不宣称生产 SLA |
| 发布 / Oracle 与交付 | 本轮未部署 | 全部必需检查、C2C、签名和回滚点完成后发布 |

设计依赖已解除：指定的 OpenDesign Pi/K3 max 实际解析为 high，用户已明确允许使用现有 K3 high 设计稿继续精修。生产适配仍需审查与实际应用验收；原型不能覆盖真实授权、协议、价格和配置语义。

本轮按用户最新要求不使用 Codex with ChatGPT，直接实施与本地验证；不以 C2C 等待作为阻塞。

2026-09-21 共享视觉批次：表单/页内编辑的材质、字重、间距和移动尺寸已适配 K3 high，16 条相关回归通过；真实 EgoLite 截图及未收口缺口见 [适配记录](../design/prism-workspace-adaptation-20260921.md)。提供商创建自动应用、复杂候选编辑形态、草稿读取时泛化冲突提示及最终全栏目验收仍需处理。

### 2026-09-21 — 提供商创建页内流程

- 新建提供商迁入共享页内工作区；地址、模型数量与手动开放确认在写入前校验，错误可修正。
- 创建出口策略、提供商、接口及可选账号/模型只保存工作草稿，不自动发布；成功保留结果，失败保留精确恢复目标且不重放写请求。
- 提交期间禁止关闭与并发本地操作；未保存离开由同一边界处理。切换草稿后重置提供商和连接查询，解决真实网关中旧列表缓存问题。
- 本地真实网关已确认创建记录存在于草稿、活动配置不存在该记录；三个目标尺寸无横向溢出。未调用真实 Provider，未部署。
- 剩余：复杂候选编辑页内迁移、草稿读取冲突提示根因、全栏目最终视觉/性能与端到端验收、签名发布。

- 本批最终验证：8 个 Chromium 定向回归、check:full 四文件确定性构建、gateway 编译、3 个 Rust 嵌入测试通过；EgoLite 在最终嵌入版本走通创建→结果→接续草稿→列表重读。日志 `/tmp/prism-provider-create-verified.log`、`/tmp/prism-provider-create-build-final.log`。

### 2026-09-21 — 候选页内编辑与草稿出口状态

- 候选新增/编辑迁入页内工作区，删除保留确认框；原始模型与能力键、来源校验、修订保护和不确定回执继续保留。
- 模型页其他操作、候选间切换及高级区收起使用同一离开边界；提交中不排队重放用户导航。
- 草稿冲突根因已核实：`provider_egress_status_service.rs::page` 要求所选配置/修订与 serving snapshot 一致。草稿页错误请求该投影导致 409。现仅活动配置读取，草稿展示无运行快照，不吞掉真正的冲突。
- EgoLite / 真实嵌入网关：草稿出口页无全局冲突；修改 local-candidate 权重 2→3，保存回执与草稿重读完成；1440×900、1280×720、390×844 均为单一页内编辑、无弹窗和横向溢出。此为合成临时数据，无真实推理或发布。
- 剩余：全部工作区整体验收、长内容/空错态/键盘与主题偏好、最终性能对比、完整 mock 接入计费链路及签名上线。本批不宣称全计划完成。
- 本批最终门禁：28 个 Chromium 路由/候选/出口回归通过（含未保存、busy、丢响应、任意能力键及 stale revision）；TypeScript、check:full 四文件确定性构建、gateway 编译和 3 个 Rust 嵌入测试通过。日志 `/tmp/prism-candidate-final.log`、`/tmp/prism-candidate-final-build.log`。此前失败来自旧 fixture 生命周期事件常量、草稿运行投影和隐藏 ID 定位，已修正后完整重跑。

## Integrated acceptance checkpoint — 2026-09-21

M4 progressed through the real embedded gateway/mock model-to-key-to-request-to-billing chain, 84 responsive/theme structural observations, and final five-sample performance comparison. Two actual regressions were repaired: delayed account search interrupting an open form, and null access-group labeling. Chromium full run: 286 passed / one fake-clock test wait failed; after correcting that wait, the complete affected group passed 8/8. Unit, type, deterministic four-file and three embedded Rust checks passed. See [integration evidence](../reports/prism-integration-acceptance-20260921.md) for exact scopes and separate runs.

Next: close the explicit whole-workspace keyboard/long/error-state and real-channel/manual-evidence readiness checklist, then signed release and Oracle post-deployment checks. M4 and M5 are not marked complete; no production release occurred. Current-round Codex with ChatGPT exclusion remains in force.

### 2026-09-21 — 键盘与状态验收

- 修复共享页内编辑关闭后焦点丢失：返回仍存在的操作入口，不抢占新页面/弹窗焦点。
- 真实网关三尺寸返回焦点、账号弹窗循环焦点/脏输入恢复/无效导入结果已验；八个工作区键盘导航已验。
- 12 条状态/详情回归与单独 3 条页内视觉/新增焦点回归通过；类型、四文件构建、gateway 和 3 条 Rust 嵌入检查通过。
- [逐工作区证据与缺口](../reports/prism-keyboard-readiness-20260921.md)明确区分导航、fixture 和真实操作。M4 全状态验收与 M5 签名上线继续开放。

### 2026-09-21 — 密钥、请求详情与长价格状态验收

- 真实 gateway/EgoLite：键盘选择及清空模型权限、零权限禁止创建、失败请求抽屉与诊断焦点、无效 JSON 原文保留及恢复、512 条长模型价格分页与移动布局已验。未签发新密钥、未保存价格目录。
- 计费生命周期、页内操作归属和密钥生命周期共 32 条 Chromium 回归通过（零重试）；本批无产品代码修改。
- [本批证据与边界](../reports/prism-operational-state-readiness-20260921.md)。下一步收敛仪表盘空错态、提供商/模型长内容及高级维护，再核对真实渠道历史证据和发布门禁。M4/M5 未标完成。
