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
