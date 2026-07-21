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
后续契约随对应 Task 创建并在需求追踪索引中登记。
