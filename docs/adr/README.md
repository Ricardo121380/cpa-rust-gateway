# Architecture Decision Records

本目录保存影响多个 Crate、公开行为、安全边界或部署方式的架构决策。

## 编号与状态

- 文件名：`ADR-NNNN-short-title.md`。
- 状态只能是 `Proposed`、`Accepted`、`Superseded` 或 `Rejected`。
- 已接受 ADR 不直接覆盖；改变决定时新建 ADR，并在旧记录中链接替代项。
- 任何 `unsafe` 例外、核心依赖方向变化、公开协议变化或持久化模型变化都必须有 ADR。

## 必填结构

```text
Title
Status
Date
Task / Matrix / Contract references
Context
Decision
Consequences
Alternatives considered
Validation and rollback
```

## 当前索引

P0 期间不创建重复描述开发计划技术基线的 ADR。P1 开始只对具体实现选择建立记录，并在 [需求追踪索引](../traceability.md) 中登记。

- [ADR-0001 Version-scoped control-plane SQLite schema](ADR-0001-version-scoped-control-plane-schema.md) — `P2-01`。
- [ADR-0002 Version-scoped route and access schema](ADR-0002-version-scoped-route-access-schema.md) — `P2-02`。
- [ADR-0003 XChaCha20-Poly1305 Secret Store](ADR-0003-xchacha20poly1305-secret-store.md) — `P2-03`。
- [ADR-0004 Client Key HMAC credential](ADR-0004-client-key-hmac-credential.md) — `P2-04`。
- [ADR-0005 Versioned control-plane Repository and Service](ADR-0005-versioned-control-plane-repository-service.md) — `P2-05`。
- [ADR-0006 Validated Route Compiler](ADR-0006-validated-route-compiler.md) — `P2-06`。
- [ADR-0007 Immutable RouteSnapshot publication](ADR-0007-route-snapshot-publication.md) — `P2-07`。
- [ADR-0008 Snapshot Client Key authentication](ADR-0008-snapshot-client-key-authentication.md) — `P2-08`。
- [ADR-0009 EgressPolicy SSRF admission](ADR-0009-egress-policy-ssrf-admission.md) — `P2-09`。
- [ADR-0010 Local management lifecycle and durable publication audit](ADR-0010-local-management-lifecycle.md) — `P2-10`。
- [ADR-0011 OpenAI-compatible Responses request assembly](ADR-0011-openai-compatible-responses-request-assembly.md) — `P3-01`。
- [ADR-0012 DNS-pinned bounded upstream client pool](ADR-0012-dns-pinned-upstream-client-pool.md) — `P3-02`。
- [ADR-0013 Priority-tier bounded smooth weighted scheduler](ADR-0013-priority-tier-smooth-weighted-scheduler.md) — `P3-03`。
- [ADR-0014 Endpoint Credential pool leases](ADR-0014-endpoint-credential-pool-leases.md) — `P3-04`。
- [ADR-0015 Sharded runtime health state](ADR-0015-sharded-runtime-health.md) — `P3-05`。
- [ADR-0016 Request-scoped Attempt orchestration and transparent-retry gate](ADR-0016-request-scoped-attempt-orchestration.md) — `P3-06`。
- [ADR-0017 RouteSnapshot-derived public model view and Responses force mapping](ADR-0017-routesnapshot-public-model-view.md) — `P3-07`。
- [ADR-0018 Bounded non-blocking Request, Attempt, and Usage event port](ADR-0018-bounded-request-attempt-usage-events.md) — `P3-08`。
- [ADR-0019 Controlled Mock HTTP aggregation E2E](ADR-0019-controlled-mock-http-aggregation-e2e.md) — `P3-09`。
- [ADR-0020 Authorized real-test Endpoint validation](ADR-0020-authorized-real-test-endpoint-validation.md) — `P3-10`（Accepted）。
- [ADR-0021 Delivery gate classification and controlled single-probe diagnostic](ADR-0021-delivery-gate-classification-and-single-probe-diagnostic.md) — `P4-00`（Accepted）。
- [ADR-0022 Endpoint-Credential Model Catalog discovery singleflight](ADR-0022-endpoint-credential-catalog-singleflight.md) — `P4-01`（Accepted）。
- [ADR-0023 Cache-visible delivery and supplemental supply-chain split](ADR-0023-cache-visible-delivery-and-supply-chain-split.md) — `CR-EXEC-002` / `P4-02` delivery-flow validation（Accepted）。
- [ADR-0024 Catalog snapshot freshness and last-success fallback](ADR-0024-catalog-snapshot-freshness-and-last-success-fallback.md) — `P4-02`（Accepted）。
- [ADR-0025 Catalog diff Preview/Apply and removal isolation](ADR-0025-catalog-diff-preview-apply-removal-isolation.md) — `P4-03`（Accepted）。
- [ADR-0026 Target-local probe EWMA and controlled Circuit recovery](ADR-0026-target-local-probe-ewma-and-circuit-recovery.md) — `P4-04`（Accepted）。
- [ADR-0027 Append-only bounded SQLite event writer](ADR-0027-append-only-bounded-sqlite-event-writer.md) — `P4-07`（Accepted）。
- [ADR-0028 Exact-target runtime Quota and controlled Reset recovery](ADR-0028-exact-target-runtime-quota-and-controlled-reset-recovery.md) — `P4-05`（Accepted）。
- [ADR-0029 Fixed-input Route Explain without scheduling side effects](ADR-0029-fixed-input-route-explain.md) — `P4-06`（Accepted）。
- [ADR-0030 Single-consumer structured telemetry fan-out](ADR-0030-single-consumer-telemetry-fanout.md) — `P4-08`（Accepted）。
- [ADR-0031 Default-deny log redaction and bounded body sampling](ADR-0031-default-deny-log-redaction-and-body-sampling.md) — `P4-09`（Accepted）。
- [ADR-0032 Read-only runtime management status and Credential-account recovery](ADR-0032-read-only-runtime-management-status.md) — `P4-10`（Accepted）。
- [ADR-0033 Phase-level delivery and default-ref quality-tool cache](ADR-0033-phase-level-delivery-and-default-ref-cache.md) — `P5-00`（Accepted）。
- [ADR-0034 Anthropic Messages pure codec boundary](ADR-0034-anthropic-messages-pure-codec.md) — `P5-01`（Accepted）。
- [ADR-0035 Exact token-count capability and Anthropic route](ADR-0035-exact-token-count-capability.md) — `P5-02`（Accepted）。
- [ADR-0036 Anthropic Tool stream state and normalized object input](ADR-0036-anthropic-tool-stream-state.md) — `P5-03`（Accepted）。
- [ADR-0037 Fail-closed protocol transform admission](ADR-0037-protocol-transform-admission.md) — `P5-04`（Accepted）。
- [ADR-0038 Endpoint-format-isolated protocol routing](ADR-0038-endpoint-format-isolated-protocol-routing.md) — `P5-05`（Accepted）。
- [ADR-0039 Anthropic semantic and HTTP boundary](ADR-0039-anthropic-semantic-http-boundary.md) — `P5-06`（Accepted）。
- [ADR-0040 Claude Code loopback readiness and client-key boundary](ADR-0040-claude-code-loopback-client-boundary.md) — `P5-07`（Accepted）。
- [ADR-0041 Deterministic Anthropic adversarial protocol evidence](ADR-0041-deterministic-anthropic-adversarial-evidence.md) — `P5-08`（Accepted）。
- [ADR-0042 Grok Build OAuth credential and Device Code boundary](ADR-0042-grok-build-oauth-credential-boundary.md) — `P6-01`（Accepted）。
- [ADR-0043 Revision-guarded Grok Build OAuth refresh runtime](ADR-0043-grok-build-refresh-runtime.md) — `P6-02`（Accepted）。
- [ADR-0044 Fixed Grok Build Responses boundary](ADR-0044-grok-build-responses-boundary.md) — `P6-03`（Accepted）。
- [ADR-0046 Kiro Credential runtime boundary](ADR-0046-kiro-credential-runtime-boundary.md) — `P7-01`（Accepted）。
- [ADR-0047 Kiro IDE/CLI endpoint policy](ADR-0047-kiro-endpoint-policy.md) — `P7-02`（Accepted）。
- [ADR-0048 Kiro profile ARN lifecycle](ADR-0048-kiro-profile-arn-lifecycle.md) — `P7-03`（Accepted）。
- [ADR-0049 Kiro Canonical conversation request conversion](ADR-0049-kiro-canonical-conversation-request.md) — `P7-04`（Accepted）。
- [ADR-0050 Kiro AWS EventStream framing](ADR-0050-kiro-eventstream-framing.md) — `P7-05`（Accepted）。
- [ADR-0051 Kiro per-Credential dynamic capability snapshots](ADR-0051-kiro-dynamic-capability-snapshots.md) — `P7-06`（Accepted）。
- [ADR-0052 Kiro semantic Tool and Thinking mapping](ADR-0052-kiro-semantic-tool-thinking-mapping.md) — `P7-07`（Accepted）。
- [ADR-0053 Kiro failure-owner classification](ADR-0053-kiro-failure-owner-classification.md) — `P7-08`（Accepted）。
- [ADR-0054 Grok Official API-key catalog boundary](ADR-0054-grok-official-api-key-catalog-boundary.md) — `P8-01`（Accepted）。
- [ADR-0055 Grok Official text-only Responses boundary](ADR-0055-grok-official-responses-boundary.md) — `P8-02`（Accepted）。
- [ADR-0056 Grok Official rate-limit and billing metadata boundary](ADR-0056-grok-official-rate-limit-billing-metadata.md) — `P8-03`（Accepted）。
- [ADR-0057 Grok Official Tool, Reasoning, and Search capability boundary](ADR-0057-grok-official-tool-reasoning-capability-boundary.md) — `P8-04`（Accepted）。
- [ADR-0058 Grok Official runtime quota and failure isolation](ADR-0058-grok-official-runtime-isolation.md) — `P8-05`（Accepted）。
- [ADR-0059 Grok Official local differential, concurrent load, and error matrix](ADR-0059-grok-official-local-differential-and-error-matrix.md) — `P8-06`（Accepted）。
- [ADR-0060 Grok Official authorized one-probe boundary](ADR-0060-grok-official-authorized-one-probe.md) — `P8-07`（Accepted）。
- [ADR-0061 Grok Web SSO credential lineage and revisioned lifecycle](ADR-0061-grok-web-sso-credential-lineage-lifecycle.md) — `P9-01`（Accepted；local-only）。
- [ADR-0062 Grok Web browser egress-session fingerprint binding](ADR-0062-grok-web-browser-egress-session-fingerprint.md) — `P9-02`（Accepted；local-only）。
- [ADR-0063 Grok Web fixture Chat request and stream boundary](ADR-0063-grok-web-fixture-chat-stream-boundary.md) — `P9-03`（Accepted；local-only）。
- [ADR-0064 Grok Web Conversation exact account and egress binding](ADR-0064-grok-web-conversation-egress-binding.md) — `P9-04`（Accepted；local-only）。
- [ADR-0065 Grok Web Statsig signature cache and signer SSRF boundary](ADR-0065-grok-web-statsig-cache-ssrf-boundary.md) — `P9-05`（Accepted；local-only）。
- [ADR-0066 Grok Web source-labelled quota observations](ADR-0066-grok-web-source-labelled-quota-observations.md) — `P9-06`（Accepted；local-only）。
- [ADR-0067 Grok Web 403 egress/account attribution](ADR-0067-grok-web-403-egress-account-attribution.md) — `P9-07`（Accepted；local-only）。
- [ADR-0068 Grok Web explicit Tool emulation](ADR-0068-grok-web-explicit-tool-emulation.md) — `P9-08`（Accepted；local-only）。
- [ADR-0069 Versioned management OpenAPI contract](ADR-0069-versioned-management-openapi-contract.md) — `P10-01`（Accepted；contract-only）。
- [ADR-0070 Management HTTP admission boundary](ADR-0070-management-http-admission-boundary.md) — `P10-02`（Accepted）。
- [ADR-0071 Management SPA generated-client build](ADR-0071-management-spa-generated-client-build.md) — `P10-03`（Accepted；static-only）。
- [ADR-0072 Protected management resource workflows](ADR-0072-protected-management-resource-workflows.md) — `P10-04`（Accepted；local-only）。
- [ADR-0073 Protected routing and Client Key workflows](ADR-0073-protected-routing-client-key-workflows.md) — `P10-05`（Accepted；local-only）。
- [ADR-0074 Encrypted backup with empty-target restore](ADR-0074-encrypted-backup-empty-target-restore.md) — `P10-08`（Accepted；local Phase evidence）。
- [ADR-0075 Embedded management UI with inference-route isolation](ADR-0075-embedded-management-ui-inference-isolation.md) — `P10-09`（Accepted；local-only）。
- [ADR-0076 Provider-aware management inventory](ADR-0076-provider-aware-management-inventory.md) — `P13-04A`（Accepted；正式 Gate 已通过）。
- [ADR-0077 Durable usage and explicitly unpriced cost operations read model](ADR-0077-durable-usage-cost-operations-read-model.md) — `P13-04B`（Accepted；正式 Gate 已通过）。
- [ADR-0045 Grok Build runtime state and continuity isolation](ADR-0045-grok-build-runtime-continuity.md) — `P6-04` 至 `P6-07`（Accepted）。
- [ADR-0078 Versioned billing catalog and idempotent ledger](ADR-0078-versioned-billing-catalog-and-ledger.md) — `P13-05A`（Accepted；正式 Gate 已通过）
- [ADR-0079 Billing materialization and protected read model](ADR-0079-billing-materialization-and-read-model.md) — `P13-05B`（Accepted；正式 Gate 已通过）
- [ADR-0080 Protected immutable billing catalog management](ADR-0080-protected-billing-catalog-management.md) — `P13-05C`（Accepted；正式 Gate 已通过）
- [ADR-0081 Provider-owned account-pool facade](ADR-0081-provider-owned-account-pool-facade.md) — `P13-06A`（Accepted；正式 Gate 已通过）
- [ADR-0082 Provider runtime account-pool adapter](ADR-0082-provider-runtime-pool-adapter.md) — `P13-06B`（Accepted；正式 Gate 已通过）
- [ADR-0083 Provider account-pool operator actions and failure feedback](ADR-0083-provider-account-operator-actions.md) — `P13-06C`（Accepted；正式 Gate 已通过）
- [ADR-0084 Provider-scoped deterministic routing selector](ADR-0084-provider-scoped-deterministic-routing-selector.md) — `P13-07A`（Accepted；`phase-p13-routing-complete` / `0c338ee8eef76e470c55515a24728324684365c5` / [Gate 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495) 正式通过）
- [ADR-0085 Provider-scoped Route Explain composition](ADR-0085-provider-scoped-route-explain-composition.md) — `P13-07B`（Accepted；`phase-p13-routing-complete` / `0c338ee8eef76e470c55515a24728324684365c5` / [Gate 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495) 正式通过）
- [ADR-0086 Provider-scoped serving lease revalidation](ADR-0086-provider-scoped-serving-lease-revalidation.md) — `P13-07C`（Accepted；`phase-p13-routing-complete` / `0c338ee8eef76e470c55515a24728324684365c5` / [Gate 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495) 正式通过）
- [ADR-0087 Config-bound routing price evidence](ADR-0087-config-bound-routing-price-evidence.md) — `P13-07D`（Accepted；`phase-p13-routing-complete` / `0c338ee8eef76e470c55515a24728324684365c5` / [Gate 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495) 正式通过）
- [ADR-0088 Protected single-request Channel Pin diagnostic execution](ADR-0088-channel-pin-diagnostic-execution.md) — `P13-08`（Accepted；`phase-p13-channel-pin-complete` / `7e14a2733c461d04198a6413efda420a03545eea` / [Gate 31928169486](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31928169486) 正式通过；不授权 Provider/生产流量）
- [ADR-0089 Client-owned encrypted stored Responses](ADR-0089-client-owned-encrypted-stored-responses.md) — `P13-09A`（Accepted；`phase-p13-responses-complete` / `d419c4678bd2ff563046849cef800c1985d48688` / [Gate 31922870604](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31922870604) 正式通过）
- [ADR-0090 Gateway-owned stored Responses public lifecycle](ADR-0090-gateway-owned-stored-responses-public-lifecycle.md) — `P13-09B`（Accepted；同一 exact P13-09 tag / commit / [Gate 31922870604](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31922870604) 正式通过）
- [ADR-0091 Exact stored Response continuity and gateway-owned compaction](ADR-0091-exact-stored-response-continuity-and-compaction.md) — `P13-09C`（Accepted；同一 exact P13-09 tag / commit / [Gate 31922870604](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31922870604) 正式通过；exact lineage、独立 AEAD compact blob、无 fallback）
- [ADR-0092 Public OpenAI Responses WebSocket over the Canonical execution path](ADR-0092-public-responses-websocket.md) — `P13-10A`（Accepted；`phase-p13-websocket-complete` / `dc48ec40e4fb38961925f203bf3cd0f7434a34a0` / [Gate 31926927914](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31926927914) 正式通过；不授权 Provider/生产流量）
- [ADR-0093 Generic compatible-endpoint egress profiles](ADR-0093-generic-compatible-endpoint-egress.md) — `P13-11A`（Accepted；`phase-p13-egress-complete` / `a716eaaa` / [Gate 31959162202](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31959162202) 正式通过；不调用 Provider/网络）
- [ADR-0094 Generic compatible-endpoint runtime egress composition](ADR-0094-generic-compatible-endpoint-runtime-composition.md) — `P13-11B`（Accepted；同一 phase tag/commit/Gate；单一 Upstream transport registry；Direct/local-DNS SOCKS5；不调用 Provider/网络）
- [ADR-0095 Compatible serving transport handoff](ADR-0095-compatible-serving-transport-handoff.md) — `P13-11C`（Accepted；同一 phase tag/commit/Gate；复用 exact serving Credential lease，仅追加 egress lease/精确 failure scope；不调用 Provider/网络）
- [ADR-0096 Config-Version-owned compatible proxy-pool management](ADR-0096-config-version-compatible-proxy-pool-management.md) — `P13-11D DONE_WITH_BOUNDARY`（`phase-p13-egress-management-complete` / `1beb230248fb75ced146b87c547eb020ee9cd010` / [Gate 31996324578](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31996324578) 正式通过；不声称真实 Provider/代理/DNS/生产流量）
- [ADR-0097 Provider-specific egress, health, and recovery isolation](ADR-0097-provider-specific-egress-health-recovery.md) — `P13-11E E2 LOCAL_PASS_PENDING_PHASE_GATE`（Build/Console 已接入 CPAR exact lease/native adapter；E2 专项 4/4、gateway-router 158、gateway 109、strict Clippy 通过；未调用 Provider/网络，下一片 E3）
