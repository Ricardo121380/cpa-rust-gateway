# Kimi Coding 显式服务选择

2026-09-27；本轮统一前后端修复，已实现，未发布。

`POST /admin/account-channels/kimi/prepare-target` 增加可选 query `upstream_id`。
省略时保留原语义：零服务创建标准目标，唯一服务校验后复用，多服务返回 409。
指定时仅接受 Kimi Coding 类型的已有服务，并保留出口、端点与 revision 校验；未知或其他渠道目标返回 409，不创建替代资源、不自动重试。

管理鉴权、CSRF、ConfigVersion、If-Match 不变。无需数据库迁移。
权威 OpenAPI 已更新，通过 sync-contract 生成客户端。Rust HTTP 回归覆盖多服务拒绝、未知选择拒绝、显式合法选择成功；前端回归覆盖 query 传递。
