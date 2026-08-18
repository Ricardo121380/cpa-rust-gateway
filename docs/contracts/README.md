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
P12-08C 已建立 [BC-PROVIDER-023 OpenAI-compatible Chat Completions adapter](BC-PROVIDER-023-openai-compatible-chat-completions.md)。
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
P13-04A 已建立 [BC-MGMT-009 Provider-aware operational inventory](BC-MGMT-009-provider-aware-operational-inventory.md)（Accepted；正式 Gate 已通过）。
P13-04B 已建立 [BC-MGMT-010 Durable usage and explicitly unpriced cost operations](BC-MGMT-010-durable-usage-cost-operations.md)（Accepted；正式 Gate 已通过）。
后续契约随对应 Task 创建并在需求追踪索引中登记。
P13-05A 已建立 [BC-MGMT-011 Versioned billing catalog and durable ledger](BC-MGMT-011-versioned-billing-catalog-ledger.md)（正式 Gate 已通过）。
P13-05B 已建立 [BC-MGMT-012 Billing materialization and protected read model](BC-MGMT-012-billing-materialization-read-model.md)（正式 Gate 已通过）。
P13-05C 已建立 [BC-MGMT-013 Protected immutable billing catalog management](BC-MGMT-013-protected-billing-catalog-management.md)（正式 Gate 已通过）。
P13-06A 已建立 [BC-MGMT-014 Provider-owned account-pool inventory](BC-MGMT-014-provider-owned-account-pool-inventory.md)（正式 Gate 已通过）。
P13-06B 已建立 [BC-MGMT-015 Provider runtime account-pool adapter](BC-MGMT-015-provider-runtime-pool-adapter.md)（正式 Gate 已通过；不宣称新的真实多账号或 restart DB E2E）。
P13-06C 已建立 [BC-MGMT-016 Provider account operator actions and failure feedback](BC-MGMT-016-provider-account-operator-actions-and-failure-feedback.md)（正式 Gate 已通过；OpenAPI/Prism handoff 已同步留痕）。
P13-07A 已完成 [BC-ROUTE-006 Provider-scoped deterministic selector](BC-ROUTE-006-provider-scoped-deterministic-selector.md)（`DONE_WITH_BOUNDARY`；`phase-p13-routing-complete` / `0c338ee8eef76e470c55515a24728324684365c5` / [Gate 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495) 正式通过）。
P13-07B 已完成 [BC-ROUTE-007 Provider-scoped Route Explain composition](BC-ROUTE-007-provider-scoped-route-explain.md)（`DONE_WITH_BOUNDARY`；`phase-p13-routing-complete` / `0c338ee8eef76e470c55515a24728324684365c5` / [Gate 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495) 正式通过）。
P13-07C 已完成 [BC-ROUTE-008 Provider-scoped serving lease revalidation](BC-ROUTE-008-provider-scoped-serving-lease-revalidation.md)（`DONE_WITH_BOUNDARY`；same-scheduler exact lease revalidation；不改变公开协议或 Prism；`phase-p13-routing-complete` / `0c338ee8eef76e470c55515a24728324684365c5` / [Gate 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495) 正式通过）。
P13-07D 已完成 [BC-ROUTE-009 Config-bound routing price evidence](BC-ROUTE-009-config-bound-routing-price-evidence.md)（`DONE_WITH_BOUNDARY`；exact catalog binding + six-dimensional rate evidence；`phase-p13-routing-complete` / `0c338ee8eef76e470c55515a24728324684365c5` / [Gate 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495) 正式通过）。
P13-08 已完成 [BC-MGMT-017 Protected single-request Channel Pin diagnostic](BC-MGMT-017-channel-pin-diagnostic.md)（`DONE_WITH_BOUNDARY`；受保护管理端、单请求、首败、无 retry/跨 Provider fallback、value-free receipt；`phase-p13-channel-pin-complete` / `7e14a2733c461d04198a6413efda420a03545eea` / [Gate 31928169486](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31928169486) 正式通过；native adapter/真实 Provider 验收仍为显式边界）。
P13-09A 已完成 [BC-RESP-001 Client-owned stored Response foundation](BC-RESP-001-client-owned-stored-response-foundation.md)（`DONE_WITH_BOUNDARY`；AEAD、exact Client Key owner、TTL/GC/restart；`phase-p13-responses-complete` / `d419c4678bd2ff563046849cef800c1985d48688` / [Gate 31922870604](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31922870604) 正式通过）。
P13-09B 已完成 [BC-RESP-002 Stored Response public lifecycle](BC-RESP-002-stored-response-public-lifecycle.md)（`DONE_WITH_BOUNDARY`；gateway-owned `store:true`、exact owner GET/DELETE、JSON/SSE durability；同一 exact P13-09 Gate 正式通过）。
P13-09C 已完成 [BC-RESP-003 Exact stored Response continuity and compaction](BC-RESP-003-exact-continuity-and-compaction.md)（`DONE_WITH_BOUNDARY`；exact lineage/Credential revision、Provider capability、独立 AEAD compact blob、无 retry/fallback；同一 exact P13-09 Gate 正式通过）。
P13-10A 已完成 [BC-RESP-004 Public OpenAI Responses WebSocket](BC-RESP-004-public-responses-websocket.md)（`DONE_WITH_BOUNDARY`；复用 Canonical/Client Key/lease/stored continuity，text-only `response.create`、显式 capability、有界背压/关闭/取消；`phase-p13-websocket-complete` / `dc48ec40e4fb38961925f203bf3cd0f7434a34a0` / [Gate 31926927914](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31926927914) 正式通过；不等同 Realtime API）。
P13-11A 已建立 [BC-SEC-005 Generic compatible-endpoint egress profile](BC-SEC-005-generic-compatible-endpoint-egress.md)（`DONE_WITH_BOUNDARY`；`phase-p13-egress-complete` / `a716eaaa` / [Gate 31959162202](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31959162202) 正式通过；不调用 Provider/网络）。
P13-11B 已完成 [BC-SEC-006 Compatible-endpoint runtime egress composition](BC-SEC-006-compatible-endpoint-runtime-composition.md)（`DONE_WITH_BOUNDARY`；同一 phase tag/commit/Gate；active Config Version、exact pool/policy lineage、单一 Upstream transport registry、Direct/local-DNS SOCKS5、RAII lease；不调用 Provider/网络）。
P13-11C 已完成 [BC-SEC-007 Compatible serving transport handoff](BC-SEC-007-compatible-serving-transport-handoff.md)（`DONE_WITH_BOUNDARY`；同一 phase tag/commit/Gate；复用 serving exact Credential lease；模式超时不变；精确 egress lease/failure scope；不调用 Provider/网络）。
P13-11D 已完成 [BC-MGMT-018 Compatible proxy-pool persistence and protected management](BC-MGMT-018-compatible-proxy-pool-management.md)（`DONE_WITH_BOUNDARY`；`phase-p13-egress-management-complete` / `1beb230248fb75ced146b87c547eb020ee9cd010` / [Gate 31996324578](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31996324578) 正式通过；D2 已改变 management OpenAPI/Prism 并完成 Claude Code handoff，D3 已接入 active runtime；不调用 Provider/代理/DNS，不声称真实网络或生产结果）。
P13-11E 已建立 [BC-SEC-008 Provider-specific egress, health, and recovery isolation](BC-SEC-008-provider-specific-egress-health-recovery.md)（`E0-E3 DONE_WITH_BOUNDARY`；`phase-p13-provider-egress-complete` / `ba2261a5414fe73d147a102a266abd3e9a7fbb5b` / [Gate 32044424886](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/32044424886) 正式通过；E1 typed state、E2 Build/Console typed Direct exact-lease adapter、E3 Web transport-free sticky/session/clearance 与 atomic singleflight 已完成；Web 11/11、clearance 13/13；E4 `DEFERRED_OPTIONAL`、E5 `DEFERRED_UNAUTHORIZED`，未调用 Provider/网络）。
P13-11E4 已完成 [BC-MGMT-019 Provider-specific egress runtime status](BC-MGMT-019-provider-specific-egress-runtime-status.md)（`DONE_WITH_BOUNDARY`；受保护 GET-only、atomic source-domain rows、Config/runtime 双 revision、fixed observation/retained cursor、safe `400/409/503`、OpenAPI/Prism/client/handoff；aggregate Full `43/43`；`phase-p13-provider-egress-status-complete` @ `ce98faa9306d076f5af53b9eef0c818abb1cb9c8`；正式 [Gate 32110872875](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/32110872875) 全部成功；当前 source 仅 Build/Console，production Web/clearance 可空，generic compatible 独立 owner；无 action/recovery/Provider/网络）。
