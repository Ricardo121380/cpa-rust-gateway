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
- [G9 Grok Web 阶段门禁报告](g9-gate-report.md)

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
- [P9-04 Grok Web Conversation account and egress binding](p9-04-grok-web-conversation-binding.md) — local phase-gate pass; no Web request used
- [P9-05 Grok Web Statsig cache and signer SSRF boundary](p9-05-grok-web-statsig-cache-ssrf.md) — local phase-gate pass; no Statsig/Web request used
- [P9-06 Grok Web source-labelled quota](p9-06-grok-web-source-labelled-quota.md) — local phase-gate pass; no Web/REST/gRPC-Web request used
- [P9-07 Grok Web 403 egress/account attribution](p9-07-grok-web-403-egress-account-attribution.md) — local phase-gate pass; no Web request or error body used
- [P9-08 Grok Web explicit Tool emulation](p9-08-grok-web-explicit-tool-emulation.md) — local phase-gate pass; default off and no Web request/Tool execution used
- [P9 local audit and G9 deferral](p9-local-audit-g9-deferred.md) — historical pre-Canary audit, superseded by P9-09/G9 closeout
- [P9-09 Grok Web authorized Canary](p9-09-authorized-web-canary.md) — complete; three bounded Canary observations and the P9 Delivery Gate passed
- [P9 local Full gate log](p9-09-local-full-check.md) — PASS
- [P10-01 Versioned management OpenAPI contract](p10-01-management-openapi.md) — local phase-gate pass; contract-only, no administrative listener
- [P10-02 Management HTTP admission boundary](p10-02-management-http-admission.md) — guarded Scope, no CRUD/UI route or listener bind
- [P10-03 Management SPA and generated client](p10-03-management-spa-generated-client.md) — independent static shell and reproducible OpenAPI-generated client build
- [P10-04 Protected management resource workflows](p10-04-management-resource-workflows.md) — local phase-gate pass; no deployment or Provider default path
- [P10-05 Protected routing and Client Key workflow plan](p10-05-execution-plan.md) — local phase-gate pass; draft configuration only
- [P10-05 Protected routing and Client Key workflows](p10-05-protected-routing-client-key-workflows.md) — local phase-gate pass; browser E2E and Key redaction evidence, no runtime or deployment
- [P10-06 Runtime observability management workflow plan](p10-06-execution-plan.md) — scoped implementation boundary
- [P10-06 Runtime observability management workflows](p10-06-runtime-management.md) — local phase-gate pass; loopback browser E2E and value-free runtime projections only
- [P10-07 Configuration lifecycle workflow plan](p10-07-execution-plan.md) — scoped protected lifecycle transitions and lifecycle audit only
- [P10-07 Configuration lifecycle](p10-07-configuration-lifecycle.md) — local phase-gate pass; protected lifecycle E2E and no-persistence browser evidence
- [P10-08 Encrypted backup and empty-target restore](p10-08-encrypted-backup-restore.md) — local phase-gate pass; encrypted SQLite recovery rehearsal, protected binary HTTP and no-persistence browser evidence
- [P10-09 Embedded management UI and inference isolation](p10-09-embedded-management-ui.md) — local phase-gate pass; embedded closed asset map, hardened static HTTP and data-plane route isolation evidence
- [G10 management-control-plane gate report](g10-gate-report.md) — local Phase Gate passed; awaiting P10's only GitHub Delivery Gate
- [P11-01 Differential Fixture Harness](p11-01-differential-fixture-harness.md) — local phase evidence
- [P11-02 Fault Matrix](p11-02-fault-matrix.md) — local phase evidence
- [P11-03 benchmark baseline](p11-03-benchmark-baseline.md) — local phase evidence
- [P11-04 load and soak](p11-04-load-soak.md) — accepted local synthetic evidence; P12 Canary remains required
- [P11-05 security audit](p11-05-security-audit.md) — local phase evidence
- [P11-06 recovery report](p11-06-recovery-report.md) — local phase evidence
- [P11-07 upgrade and rollback report](p11-07-upgrade-rollback.md) — local phase evidence
- [P11-08 `v0.1.0-alpha.1` Release Candidate ledger](p11-08-release-candidate.md) — local candidate inventory; no published artifact
- [G11 release-hardening gate report](g11-gate-report.md) — complete; GitHub Delivery Gate passed
- [P12-01 release artifact execution plan](p12-01-execution-plan.md) — pinned Linux artifact, private OCI archive, redacted SBOM, checksum and approved keyless-signing boundary
- [P12-01 release artifact](p12-01-release-artifact.md) — accepted revision-bound Linux binary/OCI/SBOM/manifest, GitHub OIDC keyless signature, Rekor inclusion and private workflow artifact
- [P12-02 deployment envelope execution plan](p12-02-execution-plan.md) — approved minimal `serve`, isolated loopback listeners, `LoadCredential`, state/log directories, systemd hardening and Linux verification plan
- [P12-02 deployment envelope acceptance](p12-02-deployment-envelope.md) — local Full gate, independent review, and repaired real-Linux systemd 255 syntax verification accepted; no Unit was installed or started
- [P12-03 server backup and rollback inventory plan](p12-03-execution-plan.md) — constrained server-local snapshot plan; no configuration, database, or credential material may enter the repository
- [P12-03 server backup and rollback receipt](p12-03-server-backup-rollback.md) — value-free incumbent CPA snapshot, version identity, integrity review, and exact non-log rollback procedure
- [P12-04 Staging execution plan](p12-04-execution-plan.md) — revision-bound private signed-artifact precondition, isolated loopback deployment boundary, and fail-closed Staging sequence
- [P12-04 isolated Staging receipt](p12-04-staging-receipt.md) — signed-artifact/digest provenance, loopback runtime acceptance, root-only credential metadata, new-instance-only rollback, and no incumbent change
- [P12-05 controlled Krill Staging execution plan](p12-05-execution-plan.md) — ephemeral CC Switch selected-Bearer boundary, production data-plane composition, minimum real validation sequence, and Staging-only rollback
- [P12-05 server-only Krill Models preflight](evidence/p12-05-server-models-preflight-20260725.md) — value-free direct-egress discriminator: selected base URL/Bearer passed with `2xx` JSON, without any Staging or incumbent change
- [P12-05 CR-006 Responses classifier capture failure](evidence/p12-05-cr-006-capture-failure-20260725.md) — conservative closure of the lost classifier output; no retry under CR-006
- [P12-05 server-only Krill Responses replacement classifier](evidence/p12-05-server-responses-classifier-20260725.md) — root-only value-free receipt: direct Responses returned `2xx` JSON and passed the visible decoder structure subset
- [P12-05 server-only Krill Responses full-decoder classifier](evidence/p12-05-server-responses-full-decoder-20260725.md) — root-only value-free receipt: direct Responses passed the full safe non-streaming decoder-contract mirror
- [P12-05 server-only Krill Responses exact-shape classifier](evidence/p12-05-server-responses-exact-shape-20260725.md) — root-only value-free receipt: the exact P12 outbound body passed the full non-streaming decoder-contract mirror
- [P12-05 CR-012 attempt-stage review](evidence/p12-05-cr-012-local-review-20260725.md) — bounded, value-free protected management projection reviewed and locally gated; it isolated the later CR-013 Messages lifecycle repair
- [P12-05 CR-013 Messages lifecycle review](evidence/p12-05-cr-013-local-review-20260726.md) — P12-only usage-order and explicit-stop repair reviewed and locally gated
- [P12-05 CR-013 artifact acceptance](p12-05-cr-013-artifact-acceptance-20260726.md) — revision-bound private artifact independently verified before its one isolated Messages validation
- [P12-05 CR-013 isolated Staging receipt](evidence/p12-05-cr-013-staging-receipt-20260726.md) — exactly one Messages request passed its lifecycle check and restored the preimage
- [P12-05 CR-014 Tool/Explain receipt](evidence/p12-05-cr-014-tool-explain-receipt-20260726.md) — truthful stopped `2xx` Tool result with no retained body, no Explain, and complete rollback
- [P12-05 CR-015 Tool/Explain receipt](evidence/p12-05-cr-015-tool-explain-receipt-20260726.md) — one valid no-op Tool representation, one selected/no-upstream Explain projection, and complete rollback
- [P12-05 CR-015 post-review](evidence/p12-05-cr-015-post-review-20260726.md) — local acceptance review, credential/egress boundary, and independent rollback verification
- [P12-06 OpenAI-compatible live differential](p12-06-openai-differential.md) — candidate preflight passed; paired corpus stopped because the incumbent Responses reference returns an unattributed internal 5xx
- [P12-07 executed differential gate](p12-07-executed-differential-gate.md) — the differential corpus now computes its gateway projection by driving real gateway, store, Grok, and Kiro code
- [P12-08C OpenAI-compatible Chat Completions adapter](p12-08c-openai-chat-adapter.md) — native payload preservation, strict JSON/SSE decode, exact format registry and DNS-pinned transport evidence
