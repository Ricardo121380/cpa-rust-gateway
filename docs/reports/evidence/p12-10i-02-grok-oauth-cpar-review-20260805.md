# P12-10I-02 结果 Review

## Review 范围

审查 `p12-10i-02-grok-oauth-cpar-receipt-20260805.md` 的边界、凭证流向、真实 HTTP 证据、失败分类和回滚证据；不复述任何 OAuth/SSO 值、账号身份、endpoint、model 或正文。

## Findings

1. **BLOCKER — Build 外部 OAuth 未闭合。** 两个串行 refresh 尝试均失败，未有任何 Build credential 进入 CPAR；激活空 Build 图后 staging 启动拒绝，符合 fail-closed 预期。不能以本地 provider 单测或已有历史 sweep 替代本次 OAuth 证据。
2. **BLOCKER — Console 数据面未通过。** `/v1/models` 与 route 静态验证通过，第一条真实 Responses JSON 到达 CPAR，但 Attempt 在 egress admission 以 `EgressRejected/egress` 结束，未形成上游成功；不能把它归因成账号有效或公共 Console 可用。
3. **COVERAGE — SSE 未发送。** 首败停止规则阻止了后续 JSON/SSE 请求，避免无意义重试；因此 Console 的 SSE 仍是未覆盖项。
4. **PASS — Secret/隔离边界。** Build refresh 结果只在远端内存/管道中流转；Console SSO 只进入临时加密 pool；未写生产图、旧 CPA、grok2api、CC Switch 或公开流量。
5. **PASS — 回滚与生产不变性。** staging 已停止、恢复为生产快照副本并清理；生产服务保持 active，生产 listener/active-version 计数未变化。

## 判定

`BLOCKED_WITH_EVIDENCE`。本 Task 的失败是可复核的外部阻塞，不是代码通过，也不是生产发布。后续行动必须拆开：

- Build：取得可用的交互式 OAuth/device-code 结果后，重新登记独立受控复测；不重放本次失败 tuple。
- Console：先做只读 egress admission 分类和配置修复 review，再登记新的 CPAR JSON；修复前不发送 SSE。

## 本地复核门

- `./scripts/check.sh docs`：本次文档变更后执行。
- Git whitespace 与 tracked Secret scan：提交前执行。
