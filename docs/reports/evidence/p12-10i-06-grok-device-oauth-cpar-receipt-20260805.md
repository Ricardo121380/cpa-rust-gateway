# P12-10I-06 Grok Device OAuth 与 CPAR Build curl 回执

| 字段 | 值 |
|---|---|
| 日期 | 2026-08-05 |
| 变更边界 | `CR-P12-10I-007`；单个 Build 账号、一次 Device OAuth、Oracle Singapore 隔离 loopback staging |
| 凭证处理 | 仅在受控远端内存/临时管道中流转；未写入本机、receipt、日志或 Git |
| HTTP 边界 | 通过 CPAR staging base URL + client key 的真实 curl；预检后按首败停止 |
| 生产边界 | 生产 CPAR、旧 CPA、grok2api、CC Switch、Caddy/DNS 与公开流量未改变 |

## 结果

| 项目 | 结果 |
|---|---|
| OAuth grant | `1` 个成功；未输出或持久化 token 值 |
| `/v1/models` 预检 | `PASS`；目标 Grok route 可见 |
| 请求预算 | `6`（Responses/Chat/Messages × JSON/SSE） |
| 实际请求 | `2`；首个 Responses JSON 成功，第二个 Chat JSON 触发 `http_5xx` |
| 成功数 | `1` |
| SSE / Messages | 未发送；首败后停止 |
| retry / fallback | `0` |
| upstream_request | `sent_via_cpar` |

## 回滚与不变性

- native account import rollback：`PASS`。
- staging database：`accounts=0`、`foreign_key_check=0`、`quick_check=ok`。
- staging listeners：`0`；incumbent CPAR 仍 active，原有两个 loopback listener 保持。
- 临时 staging unit、账号批次、脚本与临时目录均已清理。

## 判定

`BLOCKED_WITH_EVIDENCE`。这枚 OAuth 已证明能够通过 CPAR 发出并完成一次 Responses JSON 请求，但 Chat JSON 首败为 `http_5xx`，因此不能宣称 Build 的三协议 JSON/SSE 矩阵通过，也未继续发送其余请求。
