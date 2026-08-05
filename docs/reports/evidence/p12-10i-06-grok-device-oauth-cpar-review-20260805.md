# P12-10I-06 Grok Device OAuth 与 CPAR Build curl Review

## Findings

1. **PASS — OAuth 认证链路。** 已登录账号完成一次 Device OAuth 授权并得到可刷新的 Build 凭证；值未输出、未落盘。
2. **PASS — CPAR 真实数据面。** `/v1/models` 预检通过，首个 Responses JSON 请求经 CPAR 到达上游并成功返回。
3. **BLOCKER — Build 协议矩阵未闭合。** 第二个 Chat JSON 请求返回固定 `http_5xx`；按首败规则未发送 Messages 或 SSE，不能把单次成功扩大为三协议可用。
4. **PASS — 失败边界。** 无 retry、无跨 Provider fallback、无重复请求；仅保留计数与类别，未保留响应正文或凭证。
5. **PASS — 回滚与生产不变性。** 临时账号批次已回滚，staging 数据库完整性通过，staging listener 已清零，生产 CPAR/旧 CPA/grok2api/公开入口未改变。

## 结论

`BLOCKED_WITH_EVIDENCE`。问题不在“没有拿到 OAuth”，而在当前隔离 Build route 的后续 Chat 请求未形成可接受的 2xx 结果。下一次应先使用与远端 schema/发行制品匹配的受控 staging 组合，再登记新的单独 CR；不得重放本次已完成的 tuple 或直接宣称渠道通过。

## Review 门

- `./scripts/check.sh docs`
- Git whitespace 与 tracked Secret scan
- 计划状态、证据链接与生产不变性断言复核
