# P12-10I-04 Grok SSO→Build OAuth 追加探针回执

| 字段 | 值 |
|---|---|
| 日期 | 2026-08-05 |
| 变更边界 | `CR-P12-10I-005`；追加 5 个全新 SSO，固定串行、每个只尝试一次 |
| 来源 | Jakarta `grok-register` SSO map；不输出或持久化任何 SSO/OAuth 值 |
| CPAR/生产 | 未启动 staging 图；生产、旧 CPA、grok2api、CC Switch、Caddy/DNS 与公开流量未改变 |

## 固定步骤结果

| 项目 | 结果 |
|---|---|
| 新增 SSO 尝试 | `5`，未复用 P12-10I-03 的候选，串行、无重试 |
| Accounts 检查 | `4xx`（源 helper 按设计继续） |
| Discovery | `2xx` |
| Device Code | `2xx` |
| Verification redirect | `4xx` |
| Device Verify | `4xx`，固定终态 `device_verify_rejected` |
| 新 access/refresh 产生 | `0/5` |
| CPAR Build 导入/JSON/SSE | `0` |

这批与上一批的失败边界完全一致。累计 10 个独立 SSO 均未进入 token 颁发阶段；这支持“当前 SSO 会话/自动 Verify 方案被 Provider 拒绝”的判断，但不能证明剩余 SSO 池永久失效，也不能归因于 CPAR Build adapter。

## 安全与不变性

- 每个候选最多一次，未重放之前失败 tuple，也未跨账号 fallback。
- 仅输出固定步骤和状态类别；没有响应正文、Cookie、SSO、access token 或 refresh token 落盘。
- 未发送任何 CPAR Build 或 Console 推理请求，未写入 staging/生产数据库或 Config Version。

## 结论

`P12-10I-04` **BLOCKED_WITH_EVIDENCE**：追加 5/5 SSO 在 Device Verify 4xx 失败，未得到可用于 Build 的 refresh。继续无边界扫描同一 SSO 池没有新增信息；后续应改为当前有效账号的交互式 Browser SSO/device 授权或先修复/确认 Provider Verify 准入。
