# CR-PRISM-V4-001 · 管理目录、配置枚举及计费处理状态

日期：2026-09-09。状态：范围已由用户批准，后端契约与实现待 M2/M3 定稿。
本轮 Codex 统一负责两侧；本文件不是接口已交付的声明。

依据：[正式执行计划](../handoffs/prism-v4-execution-plan.md)。

## BE-FE-01：有效模型投影

在同源管理鉴权下提供 serving snapshot 的只读授权模型投影。上下文仅接受既有
Access Group ID 或 Client Key ID；不接收 Client Key secret。复用数据面授权及模型
可见性代码，返回 exact ID、安全来源标识、serving 配置/目录快照及观测证据。
不同授权上下文的结果分别与数据面验证；草稿不得冒充 serving。

## BE-FE-02：完整配置图

现有 Route GET/PATCH/DELETE 继续使用。新增 Route/Candidate/Alias 的完整枚举，包含
孤立草稿；分页绑定配置 ID 和 revision，上限、游标冲突遵循既有管理投影模式。
新增 Candidate PATCH/DELETE，保留 If-Match、配置可编辑性、引用约束、审计和持久化。
不能以运营 inventory 或前端已创建对象缓存替代完整枚举。

## B1：最小处理状态

为持久事件到计费账本的增量 worker 提供安全状态：消费 checkpoint、源水位、观察时间、
待处理程度和封闭失败分类。区分未启用、处理落后、失败及已追平的空账本。
不返回事件正文、秘密或内部异常消息；请求趋势、延迟分布不在本 CR 中。

## 定稿与验证

实现时先核对现有 facade、snapshot、查询/错误模型并更新本 CR 的最终路径与 operationId；
提交权威 OpenAPI 和后端测试后运行 sync-contract，再接前端。验证授权一致性、snapshot
跨页冲突、旧 revision 拒绝、孤立候选持久化、物化重启幂等和状态安全投影。
这些是本轮内部待实施项，不等待另一位实现者。

## BE-FE-02 候选写接口定稿（2026-09-10）

- `PATCH /admin/routes/{route_id}/candidates/{candidate_id}`：`updateRouteCandidate`，
  使用既有完整 `CandidateInput`，body ID 必须与路径一致，所属 Route 不可改。
- 同路径 `DELETE`：`deleteRouteCandidate`，只移除候选，保留 Route/grants。
- 均要求管理鉴权、`X-Config-Version` 与 `If-Match`，仅可编辑草稿；同一事务提交数据、
  revision 和 `route_candidate_updated` / `route_candidate_deleted` 审计。
- 成功返回新 ETag，PATCH 返回既有 Candidate DTO，DELETE 返回 204；409 不重放。
  复用既有错误信封与输入边界，无新增秘密字段。完整枚举接口仍在实施中。

## BE-FE-02 枚举接口定稿（2026-09-10）

- `GET /admin/routes` → `listRoutes` / `RoutePage`；`GET /admin/route-candidates` →
  `listRouteCandidates` / `CandidatePage`；`GET /admin/model-aliases` →
  `listModelAliases` / `AliasPage`。
- 同源管理鉴权及 `X-Config-Version`，limit 默认 100、范围 1–200。next_cursor 为空时结束；
  游标绑定资源类型、配置 ID 与 revision，类型/版本不匹配或 revision 变化返回 409，
  客户端须丢弃旧分页并从第一页重读。limit 可以调整，未知/重复参数拒绝。
- 响应为 config_version、revision（rev-N）、items、next_cursor；ETag 为该读取事务的
  revision。SQL keyset/LIMIT+1 下推，不以 grants 或 inventory 过滤，不读取秘密。
- RouteListItem 如实返回已有 round_robin / priority_failover；现有 Route 写契约仍只接受
  smooth_weighted_round_robin。旧策略不得被前端静默转换为支持的写策略。

## BE-FE-01 HTTP 定稿（2026-09-10）

`GET /admin/models/effective` / `listEffectiveModels`，要求管理鉴权与 serving 配置的
`X-Config-Version`。access_group_id 和 client_key_id 必须且只能选一个，拒绝 secret 和
未知/重复参数。limit 1–200（默认 100），next_cursor 绑定完整安全投影；授权上下文、
配置或来源投影变化返回 409，需要从第一页重读。无效/缺失上下文为 404；合法但无模型
返回空 items；非 serving 配置或未装配运行时为 503。无写操作，不发配置 revision ETag。

响应包括 config_version、access_group_id、client_key_id（可 null）、projection_id、
observed_at_ms、items、next_cursor。projection_id 是投影指纹，不冒充目录 snapshot 版本；
source 当前表达 Candidate/Endpoint/Upstream、协议和编译目录准入。目录 snapshot 的观测
时间、版本和硬过期证据仍需在 B3 中接通，未作为空值或猜测值伪造。

## BE-FE-01 / B3 目录证据定稿（2026-09-10）

EffectiveModelSource 新增必需 catalog_evidence 数组，元素为闭合 ModelCatalogEvidence：
credential_id、version、observed_at_ms、stale_at_ms、expires_at_ms、catalog_eligible。
证据来自当前 serving Candidate 固定的 durable discovery snapshot，不在查询时拼接另一个
最新目录。catalog_eligible 使用查询时刻判断；没有持久观测为 []，前端显示“未观测”。
这些证据参与 projection_id，变化导致续页409。权威契约定稿后已运行 sync-contract。

## B1 安全处理状态定稿（2026-09-10）

`GET /admin/operations/billing-processing` / `getBillingProcessingStatus`，管理鉴权、无配置
header、无查询参数、无写入和 ETag。响应闭合 BillingProcessingStatus：state、observed_at_ms、
source_ordinal、checkpoint_ordinal、checkpoint_updated_at_ms、unresolved_failures、failure_code。
state 为 disabled/starting/current/catching_up/needs_repair/failed/stopped；未观测字段为 null。
failed 保留上次成功观测时间和水位，failure_code 仅 batch_unavailable；未修复记录优先显示
needs_repair，不因 checkpoint 追平而掩盖。HTTP 只复制内存 monitor，不访问 SQLite。

## B2 运营读取执行边界（2026-09-10）

用量、账本和失败查询共享最多4个blocking任务名额。超额请求返回503与既有 Error 信封内的
封闭 code `management_operations_busy`；不新增响应 schema 或自动重试写入。名额归实际
blocking任务所有，HTTP取消不提前释放。原有存储异常分类保持不变。

## B2 用量生产聚合与游标（2026-09-10）

生产用量读取改为有序流式聚合；新 opaque cursor 内部增加可选 snapshot_ordinal，固定所有
关联事件的上界。旧 cursor 缺少该字段仍可读取，以当前 source 建立快照，后续新 cursor
携带上界。公开响应字段不变；observed_through_ms 始终覆盖整个筛选 snapshot，而非本页。
权威 operation 描述已更新并 sync-contract。费用口径仍沿用该接口原契约，账本金额从
独立 billing 查询读取；最终运营展示需在 M4 中逐项核对，不用 token 数猜测费用。


### M4 补充：资源修改审计读取

真实验收发现 `/admin/audit-events` 只读取配置生命周期流，资源修改虽已原子持久化，
前端无读取入口。新增 `GET /admin/resource-audit-events`（listManagementResourceAuditEvents），
管理鉴权、X-Config-Version 必填；limit 1–100（默认50），before_id 为可选独占追加ID。
响应 items + next_before_id，ID使用十进制字符串避免JS整数精度丢失。只含动作、actor、
时间、配置ID和资源类型/ID，不包含请求body、secret或ciphertext。按配置和ID筛选下推SQL，
最多读取limit+1条；后续新写入不进入续页。保留原生命周期审计接口及语义。
