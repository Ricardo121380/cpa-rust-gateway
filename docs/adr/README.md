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
