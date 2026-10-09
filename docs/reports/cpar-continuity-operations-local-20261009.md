# CPAR 会话续接规则与实际操作验收：本地收口报告

日期：2026-10-09。实施编号 CPAR-48 / #57，关联 E-02、E-05、#36、父规格 #9。任务等级 L。

本批已完成五种 API 接入的十五个实际页面分支、Kiro/Kimi 提交后丢响应的实际页面恢复闭环、两渠道各六种设备边界，以及两个桌面视口的指针核对／应用。已修正实际页面暴露的 Grok API 固定组件预设错误。整体结论 **PARTIAL**：E-05 仍待明确用户裁定，#57 未完整验收，不关闭 #36/#57/#9，不冻结新发布候选。

## 范围、基线与源码

固定 base：`01dae81bb9778c92d7e76e49bbed14c3fdc565d7`；分支 `codex/prism-v4-delivery`。初始基线仅有用户 `AGENTS.md` 已跟踪修改和 4,474 个既有未跟踪文件。本批提交不含这些内容；最终核对确认原路径仍未跟踪、AGENTS 字节 hash 不变。未捕获全部既有未跟踪文件的初始内容 hash，因此不宣称已逐字节核对所有旧文件。

主代理亲自核对了 AGENTS、issue tracker、[上批报告](cpar-seo-closeout-local-20261009.md)、[#57 补充报告](cpar-seo-closeout-issue57-supplement-20261009.md)、[BC-RESP-003](../contracts/BC-RESP-003-exact-continuity-and-compaction.md)、上批最终评审回执、GitHub #9/#36/#57、适用开发门槛、前端完成定义和相关契约／交接报告。上批运行源码 `c3d9dd949c1fab69042da97deb9482a0aa14fd47` 与时间、证据继续保留原归属，不改标本次运行。

| 绑定对象 | 不可变源码／证据 |
| --- | --- |
| 本次正式实际页面与 stock binary | `6a37f74d63258d283da5f55e5e5ef31b9b9ed0c4`，clean checkout；binary SHA256 `75af11803870d1b9ccf6412c67f4f67b4278f15f4a47136a186daed3d5b92b80` |
| 同内容的 Task ID 标题合规提交 | `0cefefcf14e7d7f9e5fc02665284ed9850331434`，与上述源码 tree 完全一致；[标题映射](assets/cpar-continuity-operations-20261009-171952/commit-title-map.json) |
| 修正文档引用后 Full／前端全量检查 | `029332c5351c562d2dd3e5ef86c1faa84f5baab6`，clean checkout；相对页面源码只改变五个报告／历史回执文件，运行代码、harness、契约字节不变 |
| 本批范围及完整源对照 | [source-binding.json](assets/cpar-continuity-operations-20261009-171952/source-binding.json)；最终两轴评审另以其 exact `review_head` 绑定最终提交，不改写上述运行回执 |

前两个本批本地提交为满足 `docs/06-development-plan.md:1448` 的 Task ID 前缀规则修正了标题，正文、trailers、作者、时间及 tree 保持不变。原提交保留在专用 Git ref；没有回退既有版本，原执行回执仍写实际旧 SHA。

## A：E-05 仍待裁定

初始及最终只读 GitHub 查询均显示 #36/#57 OPEN、无评论裁定。#36 要求 revision 变化拒绝旧续接，BC-RESP-003 允许严格证明同授权的 Grok Build 正常刷新例外。会话已集中发送一次二选一卡，尚未收到明确选择；推荐项、现有实现、Full 通过和未答复均不构成裁定。

**AWAITING_USER_DECISION**。契约和续接规则未擅自改变；所选规则的 exact revision、严格同授权刷新、手动重新授权及缺失／冲突 lineage 的本批公开处理、持久化、续接验收 **NOT_RUN**。既有测试保持自己的来源，不能代替选择后的验收。已准备 [#36 同步草稿](cpar-continuity-issue36-draft-20261009.md)，两种条件文本均标未生效、未发布。

## B：Kiro／Kimi 实际设备页面

使用实际 gateway `serve` 提供的 `/admin-ui/`，隔离配置、临时数据库、合成账号和材料。EgoLite TaskSpace 36 / p1 执行真实页面指针操作；两次主恢复场景均从实际页面启动授权，不借用已有任务跳过前半段。

macOS 进程专用 DYLD interposition 只将 `auth.kimi.com`、`oidc.us-east-1.amazonaws.com` 的 DNS/TCP 目标转到自有 loopback TLS。生产固定 URL、原 SNI/Host、证书 hostname 校验、真实 workflow 的 JSON/form transport、handler、SQLite 提交、鉴权、Origin、CSRF 和凭据保护均沿实际路径。临时 CA 仅通过进程环境启用，不禁用 TLS 校验，不更改系统 DNS、代理、信任库或生产配置。OS sandbox 禁止非回环出站，独立 [network sandbox 原始事实](assets/cpar-continuity-operations-20261009-171952/accepted-stock-network-sandbox.json.gz) 留证。

环境结论是 **stock binary + 实际生产 transport + 进程专用合成网络**，不是正常外部网络下的真实渠道授权。没有测试专用 gateway 构建，也没有为验收放宽生产 URL 限制。浏览器故障在 CDP Response 阶段注入 `ConnectionClosed`：真实 poll 已产生 201，独立数据库读回确认原账号、回执已 completed 后，生成客户端尚未收到成功响应时丢弃交付。该故障层与上批 handler/TCP EOF 分开，不将两者拼接成一次新的整流程。

| 渠道／边界 | 本次行为与独立核对 | 结论 |
| --- | --- | --- |
| Kiro 主闭环 | 页面启动 → 提交后丢响应 → 显示待确认 → 关闭 → 切页 → 返回原任务 → 读取原账号／回执 → 连接／应用；账号 ID 与数据库前后相同 | PASS，实际页面／合成 transport |
| Kimi 主闭环 | 同上，实际 form transport 与原 Kimi receipt 恢复；保存和生效分别展示 | PASS，实际页面／合成 transport |
| 两主闭环的防重放 | start 各一次、token 交换各一次、无 cancel；重复 completed poll 409、无认证 receipt 404，未重复创建账号或错误关联 | PASS，管理 handler／数据库／上游计数 |
| denied、expired、普通 failed | 每渠道各三个场景，准确终态、无账号写入 | PASS，共六场景 |
| 本地设备码 TTL 到期 | 每渠道一场景，过期显示且 token 请求 0、账号 0 | PASS，共两场景 |
| transport 未确认 | 每渠道一场景，保留未知状态并读回；账号 0，无假成功 | PASS，共两场景 |
| pending 后重试成功 | 每渠道一场景，两次 token 请求、仅创建一个账号 | PASS，共两场景 |

完整场景：[主恢复 JSON](assets/cpar-continuity-operations-20261009-171952/final-device-main.json.gz)、[边界 JSON](assets/cpar-continuity-operations-20261009-171952/final-device-boundaries.json.gz)、[命令回执](assets/cpar-continuity-operations-20261009-171952/ego-final-device-desktop-receipt.json)。恢复按现有 TTL、管理会话和进程语义验收；对话框关闭／路由离开后返回不等于页面 reload，更不增加跨进程重启恢复承诺。

## C：五 API 分支与桌面指针

| 接入选项 | 零匹配：自动创建 | 唯一匹配：复用 | 多匹配：当前流程选择 | 边界 |
| --- | --- | --- | --- | --- |
| Responses | PASS | PASS | PASS | 合成 HTTPS，实际页面／管理处理／持久化 |
| Chat Completions | PASS | PASS | PASS | 同上 |
| Messages | PASS | PASS | PASS | 同上 |
| Kimi API | PASS | PASS | PASS | 同上；不是 Kimi 真实推理 |
| Grok API | PASS | PASS | PASS | 固定生产组件校验通过；真实推理 NOT_RUN |

每个分支逐项核对新增账号恰一、绑定恰一、精确 owner／endpoint／adapter／protocol／地址／path、最终 active 版本和发布次数。多个合法匹配时，未选择前保存不产生 POST，选择后精确绑定。复用保留已有停用和连接设置；既有 groups／keys／grants 不变，新模型无 grants，权限独立管理且不阻塞接入。详情见 [十五分支 JSON](assets/cpar-continuity-operations-20261009-171952/final-api-5x3-green.json.gz) 和 [原命令回执](assets/cpar-continuity-operations-20261009-171952/ego-final-api-5x3-green-receipt.json)。

实际 Grok 零匹配曾保存账号、模型、绑定，但校验返回 503 `endpoint_runtime`。原预设 `https://api.x.ai` + `/v1/responses` 虽能拼成正确 URL，却不满足 production adapter 对组件的固定要求。只将这一行改为 `https://api.x.ai/v1` + `/responses`，保持 readonly 和后端限制。回归测试经 Vite raw import 对齐 Rust 权威常量，未更改 OpenAPI 或手改生成客户端；前后端 FYI 记在 [cross-boundary-log](../cross-boundary-log.md)。原 503 JSON／截图与修复后三分支 PASS 分别保存。既有非规范组件不自动迁移或启用，需操作者核对后处理。

桌面使用 1280×800、1440×900，两次均准备十八项旧草稿，通过真实指针及滚轮打开流程、保存但不立即发布、核对完整旧清单、确认并应用一次；未以 Enter 代替指针。两个视口相关操作均 **PASS**，未复现长清单遮挡问题，未改全站 CSS。见 [桌面 JSON](assets/cpar-continuity-operations-20261009-171952/final-desktop-pointer.json.gz)、[1280 截图](assets/cpar-continuity-operations-20261009-171952/final-desktop-pointer-1280-before-apply.png)、[1440 截图](assets/cpar-continuity-operations-20261009-171952/final-desktop-pointer-1440-before-apply.png)。页面外层 window scrollY 未变化不代表操作失败；实际元素命中、可见完整清单、真实按钮请求及应用结果分别留证。

首次强制改密真实指针被暂态 `<div>` 拦截，随后初始化依赖失败；两份退出非零记录保留。同源第二次指针成功，命中诊断指向按钮自身。该现象未稳定复现，不认定布局缺陷已修复，留后续观察。整个 #57 的视觉／状态／主题／窄屏验收仍 **PARTIAL**。

[条款矩阵](cpar-continuity-issue57-matrix-20261009.md) 及 [机器矩阵](assets/cpar-continuity-operations-20261009-171952/issue57-matrix.json) 覆盖 88 条故事、F01–F08、T01–T15、十条 #57 要求。当前 198 个基础组合是真实授权审计行、执行数 0；历史 24 个 Kiro 上限、12 个丰富 metadata、49 个扩展准备行保留原 2026-10-01 源码与时间。actual_declared=null 不等于未声明。Web 原生多轮、Kiro native 硬上限仍缺实现；正确 fail-closed 不等于支持。Grok Web 工具和 Messages 精确 token 计数保持原明确不支持，未新增例外。Grok ordinary JSON 的角色／Usage 桥接已有本地支持，不再沿用过时的普通 JSON 缺口说法；非等价丰富 metadata 仍拒绝。

## 验证、失败记录与评审绑定

[证据索引](assets/cpar-continuity-operations-20261009-171952/evidence-index.json) 记录原路径、归档 hash、命令回执及场景来源。gzip 和字节复制只归档，不是重新执行；不覆盖旧证据。临时密码、API 材料、凭据文件、TLS 私钥、SQLite binary 和 gateway 首启日志不入库。

| 检查 | 源码与实际结果 |
| --- | --- |
| 正式实际页面 | clean `6a37f74d`：33 个场景 = 初始化／安全边界 2 + API 15 + 主恢复 2 + 设备边界 12 + 桌面指针 2；三个正式浏览器命令均退出 0 |
| 独立持久化核对 | [oracle 验证](assets/cpar-continuity-operations-20261009-171952/final-persistence-verification.json.gz)：活动账号 21、绑定 21、预期唯一账号 21；十个失败／过期账号在所有配置中均不存在；14 设备 session、14 token 请求；命令退出 0 |
| 首次 clean build | Vite 类型检查发现测试 Node fs import TS2307，退出非零；改为 Vite raw modules 后重新构建通过，两份原回执保留 |
| 首次 Full | clean `6a37f74d`，退出 1；Rust 1,493 通过／0 失败／12 忽略，但 Document links 缺历史 output 回执，后续 gate 未完成，不写 Full PASS |
| 修正后 Full | clean `029332c5`，退出 0，45 步全部 PASS；Rust 1,493 通过／0 失败／12 忽略，fmt、clippy、serve、多轮回归、文档／契约／secret／边界及供应链检查通过 |
| 最终前端全量 | clean `029332c5`，Vitest 69 文件、497 测试通过／0 失败／0 跳过；type-check 退出 0，Full 同时通过可复现双 build |
| 外部证据 | Provider／收费请求 0；远端 CI、新候选制品／签名、部署、生产 NOT_RUN；GitHub 只读，无 push／写入 |

Full 的文档失败通过按原字节归档上批 attestation 修复：SHA256 `613776394b46eac99279afd9872c5d71264066d85ae5fb4555ab3fcf3016b431`，原回执不变，[复制说明](assets/cpar-continuity-operations-20261009-171952/historical-attestation-copy.json) 与三处历史报告链接仅说明历史归属。旧审查 `review_head` 只证明旧批次，不能代替本批审查。

评审按 `code-review` 的 Standards／Spec 两轴独立只读执行，固定 diff 为 `git diff 01dae81bb9778c92d7e76e49bbed14c3fdc565d7...<review_head>`，覆盖本批提交文件和报告，不包含用户 AGENTS 修改。为避免将评审结果再写进自身被审查的 tree，**最终原终态、覆盖范围、硬发现处理、最终 HEAD 和源码对照**保存于本地旁置文件 `output/cpar-continuity-operations-20261009-171952/final-review-attestation.json` 及同目录 `review-results.md`。两轴结论各自判定，不把缺失终态或局部实现 PASS 当整批／#57 PASS；最终交付须核对 attestation 的 `review_head` 与最终 HEAD 相同。本报告不预写未取得的评审 verdict。

可复现关键命令为 `scripts/check.sh full`、`npm --prefix web/prism test`、`npm --prefix web/prism run type-check`。实际页面使用 `CPAR_UI_OPERATIONS=1` 启动 `scripts/acceptance/cpar_seo_ui_http.py` 和三个 `cpar-*-actual-*.mjs`／桌面 driver；具体 cwd、argv、UTC、exit、source_before/after、log hash 均在独立 receipt，网络环境和 binary 在 descriptor。共享 CARGO_TARGET_DIR 仅复用构建缓存，最终代码来自干净隔离 checkout。

早期 driver 的链接歧义、inventory envelope、模型名称／随机 ID、完整 PATCH DTO、reload 后登录、重复 observer、截图同名、快照写入竞态及合成资源重名等失败均保留，不混记为产品 RED。早期 Kiro 同 basename 图像曾被覆盖，保留最后原副本作为 pilot，不作正式 RED/GREEN；正式图像全部使用独立文件名。Grok 实际 503 属于产品 RED，与这些 driver 修正分开记账。

## 冻结新发布候选与后续门槛

**完整本地前置条件 NOT_MET**，逐项依据如下。

| 条件 | 本批结果与剩余门槛 |
| --- | --- |
| 固定 base、保留原工作区、范围提交及不可变源证据 | 已完成本批范围准备与源对照；最终提交／两轴 exact HEAD 由旁置 attestation 核对 |
| Kiro/Kimi 实际设备页恢复 | 本批两渠道合成 transport 的独立 PASS；真实渠道另待授权，不扩大恢复承诺 |
| 五 API 分支及桌面指针 | 聚焦 15 + 2 场景 PASS；全站状态／主题／窄屏仍 PARTIAL |
| E-05 统一语义与公开持久化／续接验收 | BLOCKED：待用户明确选择，所选规则 NOT_RUN |
| #57 全部原承诺的本地实现／兼容性验收 | PARTIAL：Web 原生多轮、Kiro native 硬上限仍缺实现，全量既有数据升级／回退与其他条款未完整覆盖 |
| 本批 clean Full、前端与独立评审 | Full／前端 PASS；两轴终态须绑定最终 HEAD，不以门槛通过关闭 #57 |
| 必需真实组合和实际扩展声明 | BLOCKED：缺具体账号／模型／路由／声明冻结及明确真实请求授权；基础组合真实执行 0 |

发布后的远端 CI、候选制品／签名、部署、生产另是独立门槛，全部 NOT_RUN，未获本批授权；不把这些动作悄然纳入本地实现。已准备 [#57 本地同步草稿](cpar-continuity-issue57-draft-20261009.md)，没有发布。

范围外后续观察：重复 AccessGroup 唯一 name/id 的错误返回 500；首次强制改密指针暂态拦截未稳定复现；旧 Grok 非规范组件需操作者显式核对。未为这些观察扩大到数据库／布局全站重构或自动迁移。自有两个 loopback serve 已按 stop file 正常停止，EgoLite TaskSpace 已 finish；未清理历史工作树、候选、临时材料或无关文件。
