# Executable behavior contracts

本目录把 [关键行为与兼容性契约](../02-behavior-contracts.md) 拆成可由 Fixture、属性测试和端到端测试验证的契约。

## 编号与文件约定

- 契约 ID：`BC-<domain>-NNN`，例如 `BC-STREAM-001`。
- 契约说明：`BC-<domain>-NNN-short-title.md`。
- Fixture 放入 `tests/fixtures/<domain>/`，不得包含真实凭据或未脱敏生产响应。
- 每份契约必须列出入口、前置条件、事件序列、不变量、错误语义和对应测试。

## 领域

```text
HTTP       公开接口与鉴权
STREAM     SSE、Chunk、终止和取消
TOOL       Tool 定义、参数、调用和结果
ROUTE      Alias、Candidate、Credential 与 Failover
CRED       凭据状态、刷新、Quota 和错误
CATALOG    模型发现、Fresh/Stale/Expired 与移除
CONT       Cache/Response/Replay/Conversation 连续性
SEC        Secret、SSRF、租户隔离和审计
CORE       框架无关的 Canonical Core
DELIVERY   CI 分层、任务状态与受控交付诊断
```

P1 已创建 [BC-CORE-001 Request context and errors](BC-CORE-001-request-context-and-errors.md)、
[BC-CORE-002 Canonical request](BC-CORE-002-canonical-request.md) 和
[BC-CORE-003 Canonical event state machine](BC-CORE-003-canonical-event-state-machine.md)，以及
[BC-STREAM-001 Bounded canonical stream](BC-STREAM-001-bounded-canonical-stream.md) 和
[BC-PROTOCOL-001 OpenAI Responses adapter](BC-PROTOCOL-001-openai-responses-adapter.md) 和
[BC-PROVIDER-001 Deterministic Mock Provider](BC-PROVIDER-001-deterministic-mock-provider.md) 和
[BC-HTTP-001 Actix Responses handler](BC-HTTP-001-actix-responses-handler.md) 和
[BC-AUTH-001 Client Key authentication port](BC-AUTH-001-client-key-auth-port.md)。
P2-01 已建立 [BC-STORE-001 Versioned control-plane schema](BC-STORE-001-versioned-control-plane-schema.md)。
P2-02 已建立 [BC-ROUTE-001 Versioned route and access schema](BC-ROUTE-001-versioned-route-access-schema.md)。
P2-03 已建立 [BC-SEC-001 AEAD Secret Store](BC-SEC-001-aead-secret-store.md)。
P2-04 已建立 [BC-AUTH-002 Client Key HMAC credential](BC-AUTH-002-client-key-hmac-credential.md)。
P2-05 已建立 [BC-CONTROL-001 Versioned control-plane Repository and Service](BC-CONTROL-001-versioned-control-plane-repository-service.md)。
P2-06 已建立 [BC-ROUTER-001 Validated Route Compiler](BC-ROUTER-001-validated-route-compiler.md)。
P2-07 已建立 [BC-ROUTER-002 Immutable RouteSnapshot publication](BC-ROUTER-002-route-snapshot-publication.md)。
P2-08 已建立 [BC-AUTH-003 Snapshot Client Key authentication](BC-AUTH-003-snapshot-client-key-authentication.md)。
P2-09 已建立 [BC-SEC-002 EgressPolicy SSRF admission](BC-SEC-002-egress-policy-ssrf-admission.md)。
P2-10 已建立 [BC-CONTROL-002 Local management lifecycle](BC-CONTROL-002-local-management-lifecycle.md)。
P3-01 已建立 [BC-PROVIDER-002 OpenAI-compatible Responses outbound request assembly](BC-PROVIDER-002-openai-compatible-responses-request.md)。
P3-02 已建立 [BC-UPSTREAM-001 DNS-pinned upstream client pool](BC-UPSTREAM-001-dns-pinned-upstream-client-pool.md)。
P3-03 已建立 [BC-SCHEDULER-001 Priority-tier bounded smooth weighted scheduler](BC-SCHEDULER-001-priority-tier-smooth-weighted-scheduler.md)。
P3-04 已建立 [BC-CRED-001 Endpoint Credential pool leases](BC-CRED-001-endpoint-credential-pool-leases.md)。
P3-05 已建立 [BC-HEALTH-001 Sharded runtime health state](BC-HEALTH-001-sharded-runtime-health.md)。
P3-06 已建立 [BC-ROUTER-003 Request-scoped Attempt orchestration](BC-ROUTER-003-request-scoped-attempt-orchestration.md)。
P3-07 已建立 [BC-ROUTE-002 RouteSnapshot public model view and Responses force mapping](BC-ROUTE-002-routesnapshot-public-model-view.md)。
P3-08 已建立 [BC-OBS-001 Bounded Request, Attempt, and Usage events](BC-OBS-001-bounded-request-attempt-usage-events.md)。
P3-09 已建立 [BC-E2E-001 Controlled Mock HTTP aggregation E2E](BC-E2E-001-controlled-mock-http-aggregation-e2e.md)。
P3-10 已建立 [BC-E2E-002 Authorized real-test Endpoint validation](BC-E2E-002-authorized-real-test-endpoint-validation.md)。
P4-00 已建立 [BC-DELIVERY-001 Delivery gates and authorized single-probe diagnostic](BC-DELIVERY-001-delivery-gates-and-single-probe-diagnostic.md)。
P4-01 已建立 [BC-CATALOG-001 Endpoint-Credential Model Catalog discovery singleflight](BC-CATALOG-001-endpoint-credential-catalog-singleflight.md)。
P4-02 已建立 [BC-DELIVERY-002 Cache-visible delivery and supplemental supply-chain split](BC-DELIVERY-002-cache-visible-delivery-and-supply-chain-split.md) 与 [BC-CATALOG-002 CatalogSnapshot freshness and last-success fallback](BC-CATALOG-002-catalog-snapshot-freshness-and-last-success-fallback.md)。
P4-03 已建立 [BC-CATALOG-003 Catalog diff Preview/Apply and removal isolation](BC-CATALOG-003-catalog-diff-preview-apply-removal-isolation.md)。
P4-04 已建立 [BC-HEALTH-002 Target-local probe EWMA and controlled Circuit recovery](BC-HEALTH-002-target-local-probe-ewma-and-circuit-recovery.md)。
P4-05 已建立 [BC-CRED-002 Exact-target runtime Quota and controlled Reset recovery](BC-CRED-002-exact-target-runtime-quota-and-controlled-reset-recovery.md)。
P4-06 已建立 [BC-ROUTE-003 Fixed-input Route Explain and Candidate exclusion reasons](BC-ROUTE-003-fixed-input-route-explain.md)。
P4-07 已建立 [BC-OBS-002 Append-only SQLite Request/Attempt/Usage/Health event writer](BC-OBS-002-append-only-sqlite-event-writer.md)。
P4-08 已建立 [BC-OBS-003 Single-consumer structured telemetry fan-out](BC-OBS-003-single-consumer-telemetry-fanout.md)。
P4-09 已建立 [BC-OBS-004 Default-deny log redaction and body sampling](BC-OBS-004-default-deny-log-redaction-and-body-sampling.md)。
P4-10 已建立 [BC-MGMT-001 Read-only runtime management status and Credential-account recovery](BC-MGMT-001-read-only-runtime-management-status.md)。
P8-04 已建立 [BC-PROVIDER-015 Grok Official Tool, Reasoning, and Search capability boundary](BC-PROVIDER-015-grok-official-tool-reasoning-capability.md)。
P8-05 已建立 [BC-PROVIDER-016 Grok Official runtime quota and failure isolation](BC-PROVIDER-016-grok-official-runtime-isolation.md)。
P8-06 已建立 [BC-PROVIDER-017 Grok Official local differential, concurrent load, and error matrix](BC-PROVIDER-017-grok-official-local-differential-and-error-matrix.md)。
P8-07 已建立 [BC-E2E-004 Grok Official authorized one-probe](BC-E2E-004-grok-official-authorized-one-probe.md)。
P9-01 已建立 [BC-CRED-006 Grok Web SSO credential lineage and revisioned lifecycle](BC-CRED-006-grok-web-sso-credential-lineage-lifecycle.md)。
P9-02 已建立 [BC-PROVIDER-018 Grok Web browser egress-session fingerprint](BC-PROVIDER-018-grok-web-browser-egress-session.md)。
P9-03 已建立 [BC-PROVIDER-019 Grok Web fixture Chat request and stream](BC-PROVIDER-019-grok-web-fixture-chat-stream.md)。
P9-04 已建立 [BC-CONT-003 Grok Web Conversation account and egress binding](BC-CONT-003-grok-web-conversation-egress-binding.md)。
P9-05 已建立 [BC-SEC-003 Grok Web Statsig cache and signer SSRF boundary](BC-SEC-003-grok-web-statsig-cache-ssrf-boundary.md)。
P9-06 已建立 [BC-PROVIDER-020 Grok Web source-labelled quota observations](BC-PROVIDER-020-grok-web-source-labelled-quota.md)。
P9-07 已建立 [BC-SEC-004 Grok Web 403 egress/account attribution](BC-SEC-004-grok-web-403-egress-account-attribution.md)。
P9-08 已建立 [BC-PROVIDER-021 Grok Web explicit Tool emulation](BC-PROVIDER-021-grok-web-explicit-tool-emulation.md)。
P5-00 已建立 [BC-DELIVERY-003 Phase-level delivery and default-ref cache](BC-DELIVERY-003-phase-level-delivery-and-default-ref-cache.md)。
P5-01 已建立 [BC-PROTOCOL-002 Anthropic Messages adapter](BC-PROTOCOL-002-anthropic-messages-adapter.md)。
P5-02 已建立 [BC-PROTOCOL-003 Exact token-count capability](BC-PROTOCOL-003-exact-token-count-capability.md)。
P5-03 已建立 [BC-PROTOCOL-004 Anthropic Tool stream state](BC-PROTOCOL-004-anthropic-tool-stream-state.md)。
P5-04 已建立 [BC-ROUTER-004 Protocol transform admission](BC-ROUTER-004-protocol-transform-admission.md)。
P5-05 已建立 [BC-ROUTER-005 Endpoint-format-isolated protocol routing](BC-ROUTER-005-endpoint-format-isolated-protocol-routing.md)。
P5-06 已建立 [BC-PROTOCOL-005 Anthropic semantic and HTTP boundary](BC-PROTOCOL-005-anthropic-semantic-http-boundary.md)。
P5-07 已建立 [BC-E2E-003 Claude Code loopback `--bare` compatibility](BC-E2E-003-claude-code-loopback-bare-compatibility.md)。
P5-08 已建立 [BC-PROTOCOL-006 Anthropic adversarial stream safety](BC-PROTOCOL-006-anthropic-adversarial-stream-safety.md)。
P12-08A 已建立 [BC-PROTOCOL-008 OpenAI Chat Completions strict codec](BC-PROTOCOL-008-openai-chat-completions-codec.md)。
P12-08B 已建立 [BC-HTTP-002 Actix Chat Completions boundary](BC-HTTP-002-actix-chat-completions-boundary.md)。
P6-01 已建立 [BC-CRED-003 Grok Build OAuth credential and Device Code](BC-CRED-003-grok-build-oauth-device-code.md)。
P6-02 已建立 [BC-CRED-004 Grok Build refresh singleflight and durable revision runtime](BC-CRED-004-grok-build-refresh-runtime.md)。
P6-03 已建立 [BC-PROVIDER-003 Grok Build Responses request and bounded decode boundary](BC-PROVIDER-003-grok-build-responses-boundary.md)。
P7-01 已建立 [BC-CRED-005 Kiro credential and refresh runtime](BC-CRED-005-kiro-credential-runtime.md)。
P7-02 已建立 [BC-PROVIDER-005 Kiro IDE/CLI endpoint policy](BC-PROVIDER-005-kiro-endpoint-policy.md)。
P7-03 已建立 [BC-PROVIDER-006 Kiro profile ARN lifecycle](BC-PROVIDER-006-kiro-profile-arn-lifecycle.md)。
P7-04 已建立 [BC-PROVIDER-007 Kiro Canonical conversation request conversion](BC-PROVIDER-007-kiro-canonical-conversation-request.md)。
P7-05 已建立 [BC-PROVIDER-008 Kiro AWS EventStream framing](BC-PROVIDER-008-kiro-eventstream-framing.md)。
P7-06 已建立 [BC-PROVIDER-009 Kiro dynamic capability and last-success snapshots](BC-PROVIDER-009-kiro-dynamic-capability-snapshots.md)。
P7-07 已建立 [BC-PROVIDER-010 Kiro semantic Tool, Thinking, and Claude Code mapping](BC-PROVIDER-010-kiro-semantic-tool-thinking-mapping.md)。
P7-08 已建立 [BC-PROVIDER-011 Kiro network, account, model, quota, and rate-limit classification](BC-PROVIDER-011-kiro-failure-owner-classification.md)。
P6-04 至 P6-07 已建立 [BC-PROVIDER-004 Grok Build runtime state and continuity boundary](BC-PROVIDER-004-grok-build-runtime-continuity.md)。
P8-01 已建立 [BC-PROVIDER-012 Grok Official API-key model catalog boundary](BC-PROVIDER-012-grok-official-api-key-catalog.md)。
P8-02 已建立 [BC-PROVIDER-013 Grok Official text-only Responses boundary](BC-PROVIDER-013-grok-official-responses-boundary.md)。
P8-03 已建立 [BC-PROVIDER-014 Grok Official rate-limit and billing metadata boundary](BC-PROVIDER-014-grok-official-rate-limit-billing-metadata.md)。
P10-01 已建立 [BC-MGMT-002 Versioned management OpenAPI contract](BC-MGMT-002-versioned-management-openapi.md)。
P10-02 已建立 [BC-MGMT-003 Management HTTP admission](BC-MGMT-003-management-http-admission.md)。
P10-03 已建立 [BC-MGMT-004 Management SPA generated client](BC-MGMT-004-management-spa-generated-client.md)。
P10-04 已建立 [BC-MGMT-005 Protected management resource workflows](BC-MGMT-005-protected-management-resource-workflows.md)。
P10-05 已建立 [BC-MGMT-006 Protected routing and Client Key workflows](BC-MGMT-006-protected-routing-client-key-workflows.md)。
P10-08 已建立 [BC-MGMT-007 Encrypted control-plane backup and empty-target restore](BC-MGMT-007-encrypted-control-plane-backup.md)。
P10-09 已建立 [BC-MGMT-008 Embedded management UI and inference-route isolation](BC-MGMT-008-embedded-management-ui-inference-isolation.md)。
后续契约随对应 Task 创建并在需求追踪索引中登记。
