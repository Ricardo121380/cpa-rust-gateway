# Verification reports

本目录保存 Task、Phase Gate、基准、差分、安全和部署验证证据。

## 命名规则

- Task：`p<phase>-<task>-short-title.md`。
- Gate：`g<phase>-gate-report.md`。
- 基准：`benchmark-YYYY-MM-DD-short-title.md`。
- 安全：`security-YYYY-MM-DD-short-title.md`。

## 报告最低内容

- 计划版本、Task/Gate、日期和环境。
- 改动范围与对应 Matrix/Contract/ADR。
- 执行命令及退出状态。
- 可复查结果和已知限制。
- 失败、偏差、回滚与后续任务。

报告不得保存 Secret、Cookie、Authorization Header、原始 Cache Key、生产 Body 或未脱敏日志。需要原始材料时，只记录受控外部位置和不可逆摘要。

## 已完成阶段

- [G0 阶段门禁报告](g0-gate-report.md)
- [G0 干净检出完整门禁日志](g0-clean-full-log.md)
- [G0 可复现构建日志](g0-reproducible-build-log.md)
- [G1 阶段门禁报告](g1-gate-report.md)
- [G2 阶段门禁报告](g2-gate-report.md)

## 已完成任务

- [P1-01 Request context and errors report](p1-01-request-context-errors.md)
- [P1-02 Canonical request report](p1-02-canonical-request.md)
- [P1-03 Canonical event state machine report](p1-03-canonical-event-state-machine.md)
- [P1-04 Bounded canonical stream report](p1-04-bounded-stream.md)
- [P1-05 OpenAI Responses adapter report](p1-05-openai-responses-adapter.md)
- [P1-06 Deterministic Mock Provider report](p1-06-deterministic-mock-provider.md)
- [P1-07 Actix Responses handler report](p1-07-actix-responses-handler.md)
- [P1-08 In-memory Client Key authentication report](p1-08-in-memory-client-key-auth.md)
- [P1-09 Tool stream property-test report](p1-09-tool-stream-property-tests.md)
- [P2-01 Versioned control-plane schema report](p2-01-control-plane-schema.md)
- [P2-02 Versioned route and access schema report](p2-02-route-access-schema.md)
- [P2-03 AEAD Secret Store report](p2-03-aead-secret-store.md)
- [P2-04 Client Key HMAC credential report](p2-04-client-key-hmac.md)
- [P2-05 Versioned control-plane Repository and Service report](p2-05-control-plane-service.md)
- [P2-06 Validated Route Compiler report](p2-06-validated-route-compiler.md)
- [P2-07 Immutable RouteSnapshot publication report](p2-07-route-snapshot-publication.md)
- [P2-08 Snapshot Client Key authentication report](p2-08-snapshot-client-key-authentication.md)
- [P2-09 EgressPolicy SSRF admission report](p2-09-egress-policy-ssrf-admission.md)
- [P2-10 Local management lifecycle report](p2-10-management-lifecycle.md)
- [P3-01 OpenAI-compatible Responses request assembly report](p3-01-openai-compatible-responses-request.md)
- [P3-02 DNS-pinned upstream client pool report](p3-02-dns-pinned-upstream-client-pool.md)
- [P3-03 Priority-tier smooth weighted scheduler report](p3-03-priority-tier-scheduler.md)
- [P3-04 Endpoint Credential pool report](p3-04-endpoint-credential-pool.md)
- [P3-05 Sharded runtime health report](p3-05-runtime-health.md)
- [P3-06 Attempt Orchestrator report](p3-06-attempt-orchestrator.md)
- [P3-07 RouteSnapshot public model view report](p3-07-routesnapshot-public-model-view.md)
- [P3-08 bounded Request, Attempt, and Usage events report](p3-08-bounded-request-attempt-usage-events.md)
