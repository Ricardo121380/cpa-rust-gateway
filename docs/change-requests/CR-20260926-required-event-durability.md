# CR：必需请求事件持久化确认与恢复

2026-09-26；对应可靠性计划 M1；用户已授权 Codex 实施前后端边界，本次不部署。

## 行为契约

1. 核心事件端口增加可等待的有界确认。生产 Request 必须收到 SQLite commit 确认，才可调用上游；队列满、关闭、超时/存储故障返回安全 RecordingUnavailable。确认失败不自动重放请求。测试/嵌入 Noop 能力不当作生产耐久证明。
2. Attempt 先确认再继续输出或重试；Usage 在下游终态之前确认；RequestFinished 是下游交付观测，取消时不能异步等待，失败计入显式健康降级，持久 Request 保留未知，而非伪造成功。所有生产发射点处理结果。JSON 以真实 Actix body handoff 判定交付，后续 poll 等待确认；若 sized body 交付后直接 Drop，则检查有界入队结果。交付前取消不能被提前写成成功；服务器交付不等于远端客户端已收全。
3. Request 增加可选 started_at_ms；Usage 增加可选 attempt_id（已确认的 request-local attempt）。旧事件缺字段仍解码；新事件不通过 request_id 猜测跨 attempt 关联。事件类型+来源 ID 幂等、响应 ID 保留。
4. schema29 增加请求记录状态与持久隔离表，不改写原始事件/旧账本。请求活动、已完成、重启后未决分别建模；隔离保留安全事件及原因/指纹，坏记录不静默消失。恢复不制造 token、费用或成功终态。
5. 现有受管理鉴权保护的 observability/metrics 增加接受新请求、未确认数、最后成功写入、故障分类、重启未决计数。计费物化状态沿用 operations/billing-processing；healthz 仅进程存活。
6. SQLite 只由现有 spawn_blocking writer 访问。队列与 batch 有上限，无第二个无限队列、无每 chunk 写入、无同步 Actix SQL。

## 兼容与验证

管理 JSON 响应不改变；Prometheus 是增量指标，不产生前端 DTO。事件内部持久契约新增可选字段；核心错误分类增加 RecordingUnavailable，协议错误映射/快照同步。若实施需要管理 JSON 新字段，先改权威 OpenAPI 再生成。

schema29 回退只能在隔离副本演练后使用：新状态/隔离证据先保留，新增事件字段在旧程序拒绝 unknown_fields 的情况下不能直接交给旧二进制。生产回退使用升级前完整快照，不删除当前历史来伪装兼容；M1 本地回退测试不授权生产数据回滚。

用例见 M1-01–06：queue/full/disk故障上游计数零、独立进程kill/restart、迟到/乱序/重放、毒记录隔离持久化、11条历史隔离保留、JSON/SSE/取消/关停及租约回收。仅通过适用用例后标记本地完成，真实客户端与发布在M4。
