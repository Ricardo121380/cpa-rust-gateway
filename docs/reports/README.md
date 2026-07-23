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
- [G3 阶段门禁报告](g3-gate-report.md)
- [G4 阶段门禁报告](g4-gate-report.md)

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
- [P3-09 controlled Mock HTTP aggregation E2E report](p3-09-controlled-mock-http-aggregation-e2e.md)
- [P3-10 authorized real-test Endpoint validation report](p3-10-real-test-endpoint-validation.md)
- [P4-00 execution acceleration report](p4-00-execution-acceleration.md)
- [P4-01 Endpoint-Credential Model Catalog singleflight report](p4-01-catalog-singleflight.md)
- [P4-02 CatalogSnapshot freshness and last-success fallback report](p4-02-catalog-snapshot-freshness.md)
- [P4-03 Catalog diff Preview/Apply and removal isolation report](p4-03-catalog-diff-preview-apply.md)
- [P4-04 Target-local Probe, EWMA, and Circuit recovery report](p4-04-probe-ewma-circuit-recovery.md)
- [P4-05 Exact-target Runtime Quota and controlled Reset recovery report](p4-05-runtime-quota-recovery.md)
- [P4-06 Fixed-input Route Explain and Candidate exclusion reasons report](p4-06-route-explain.md)
- [P4-07 Append-only SQLite Request/Attempt/Usage/Health event writer report](p4-07-append-only-sqlite-event-writer.md)
- [P4-08 Structured JSON, Prometheus, and OpenTelemetry telemetry fan-out report](p4-08-single-consumer-telemetry-fanout.md)
- [P4-09 Default-deny log redaction and bounded Body sampling report](p4-09-log-redaction-body-sampling.md)
- [P4-10 Read-only runtime management status and controlled Credential-account recovery report](p4-10-read-only-runtime-management-status.md)
- [P5-00 Phase-level delivery and default-ref cache report](p5-00-phase-level-delivery.md)
- [P5-01 Anthropic Messages adapter report](p5-01-anthropic-messages-adapter.md)
- [P5-02 Exact token-count capability report](p5-02-exact-token-count-capability.md)
- [P5-03 Anthropic Tool stream state report](p5-03-anthropic-tool-stream-state.md)
- [P5-04 Protocol transform admission report](p5-04-protocol-transform-admission.md)
- [P5-05 Endpoint protocol isolation report](p5-05-endpoint-protocol-isolation.md)
- [P5-06 Anthropic semantic and HTTP boundary report](p5-06-anthropic-semantic-http-boundary.md)
- [P5-07 Claude Code `--bare` loopback E2E report](p5-07-claude-code-bare-e2e.md)
- [P5-08 Anthropic adversarial stream properties report](p5-08-adversarial-stream-properties.md)
- [G5 Anthropic/Claude Code phase gate report](g5-gate-report.md)
- [P6-01 Grok Build OAuth credential and Device Code report](p6-01-grok-build-oauth.md) — complete
- [P6-02 Grok Build refresh singleflight and durable revision runtime report](p6-02-grok-build-refresh-runtime.md) — complete
- [P6-03 Grok Build Responses request, stream, and error report](p6-03-grok-build-responses.md) — complete under CR-P6-03-013; direct T18 remains `unattributed` and closed
- [P6-04 Build catalog, Billing, Quota Window, and Reset state](p6-04-build-runtime-catalog-quota.md) — complete
- [P6-05 Tenant-isolated Build cache identity and affinity](p6-05-build-cache-affinity.md) — complete
- [P6-06 Build Response Ownership and Reasoning Replay](p6-06-build-response-ownership-replay.md) — complete
- [P6-07 Build-specific failure classification](p6-07-build-failure-classification.md) — complete
- [P6-08 Grok Build clean-room differential report](p6-08-grok-build-clean-room-differential.md) — complete
- [G6 Grok Build phase gate report](g6-gate-report.md) — complete
- [P7-01 Kiro Credential runtime](p7-01-kiro-credential-runtime.md) — local phase-gate pass
- [P7-02 Kiro IDE/CLI endpoint policy](p7-02-kiro-endpoint-policy.md) — local phase-gate pass
- [P7-03 Kiro profile ARN lifecycle](p7-03-kiro-profile-arn.md) — local phase-gate pass
- [P7-04 Kiro Canonical conversation request](p7-04-kiro-conversation-request.md) — local phase-gate pass
- [P7-05 Kiro AWS EventStream framing](p7-05-kiro-eventstream-framing.md) — local phase-gate pass
- [P7-06 Kiro dynamic capability snapshots](p7-06-kiro-dynamic-capability-snapshots.md) — local phase-gate pass
- [P7-07 Kiro Tool, Thinking, and Claude Code compatibility](p7-07-kiro-tool-thinking-compatibility.md) — local phase-gate pass
- [P7-08 Kiro failure-owner classification](p7-08-kiro-failure-classification.md) — local phase-gate pass
- [P7-09 Kiro-RS differential and `--bare` E2E](p7-09-kiro-rs-differential-bare-e2e.md) — deferred to the final external-authentication package
- [P8-01 Grok Official API-key catalog](p8-01-grok-official-catalog.md) — local phase-gate pass
- [P8-02 Grok Official Responses HTTP/SSE](p8-02-grok-official-responses.md) — local phase-gate pass
- [P8-03 Grok Official rate-limit and billing metadata](p8-03-grok-official-metadata.md) — local phase-gate pass
- [P8-04 Grok Official Tool, Reasoning, and Search capability](p8-04-grok-official-capabilities.md) — local phase-gate pass
- [P8-05 Grok Official / Build state isolation](p8-05-grok-official-build-isolation.md) — local phase-gate pass
- [P8-06 Grok Official local differential, concurrent load, and error matrix](p8-06-grok-official-local-differential.md) — local phase-gate pass ([Full Gate evidence](p8-06-local-full-check.md))
- [P8-07 Grok Official authorized one-probe](p8-07-authorized-official-probe.md) — local safety harness pass; deferred to the final external-authentication package with P7-09
- [P9-01 Grok Web SSO credential, lineage, and lifecycle](p9-01-grok-web-sso-credentials.md) — local phase-gate pass; no SSO source or Web request used
- [P9-02 Grok Web browser egress-session fingerprint](p9-02-grok-web-browser-egress-session.md) — local phase-gate pass; no browser or Web request used
- [P9-03 Grok Web fixture Chat request and stream](p9-03-grok-web-fixture-chat-stream.md) — local phase-gate pass; no Web endpoint or request used
