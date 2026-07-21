# Requirements traceability index

本文件连接“需求来源 → Task → ADR/Contract → 实现/测试 → 验证报告”。实现代码前必须能从 Task 回溯到功能矩阵或已冻结技术基线。

## 权威来源

1. [开发计划 v1.0](06-development-plan.md)：Task、依赖、Gate 和变更控制。
2. [功能筛选矩阵](01-feature-selection-matrix.md)：功能 ID 和范围决定。
3. [关键行为契约](02-behavior-contracts.md)：跨协议和运行时不变量。
4. [目标架构](03-target-architecture-draft.md)：模块与依赖方向。
5. [上游聚合设计](05-upstream-aggregation-design.md)：Upstream、Endpoint、Route、Key 和 Snapshot。

## P0 追踪

| Task | 需求来源 | ADR/Contract | 实现或检查 | 验证证据 | 状态 |
|---|---|---|---|---|---|
| P0-01 | Plan 4.2、4.3、22；Matrix G22/J19/L34-L36 | SEC 目录约定 | `.gitignore`、Secret Scanner、Pre-commit | [P0-01 报告](reports/p0-01-repository-baseline.md) | DONE |
| P0-02 | Plan 1.2、4.3、23、24 | ADR/Contract/Report 目录约定 | 本索引与链接检查 | [P0-02 报告](reports/p0-02-document-traceability.md) | DONE |
| P0-03 | BL-01；Plan 4.1、G0 | 首批 ADR 在 P1 按需建立 | Workspace、Crate 边界、工具链 | [P0-03 报告](reports/p0-03-rust-workspace.md) | DONE |
| P0-04 | Plan 4.3、20、22；Matrix K12 | SEC 契约在相关实现阶段建立 | fmt、Clippy、test、deny、audit | [P0-04 报告](reports/p0-04-quality-gates.md) | DONE |
| P0-05 | Plan 20.2、20.3；Matrix K12 | 无 | 本地命令与 CI | [P0-05 报告](reports/p0-05-ci-baseline.md) | DONE |
| P0-06 | Plan P0-06、21；Matrix K12 | 无 | Mac/VPS 环境基线 | [P0-06 环境基线](reports/environment-baseline.md) | DONE |
| G0 | Plan G0、4.3、20-22 | 无 unsafe 例外 ADR | 干净 full gate、双构建 SHA、阶段审计 | [G0 报告](reports/g0-gate-report.md) | DONE |

## P1 追踪

| Task | 需求来源 | ADR/Contract | 实现或检查 | 验证证据 | 状态 |
|---|---|---|---|---|---|
| P1-01 | Plan 7；Matrix B25/G05；Behavior 1、9、15；Channel analysis 5.4 | [BC-CORE-001](contracts/BC-CORE-001-request-context-and-errors.md) | `gateway-core` IDs、RequestContext、GatewayError 与 scope | [P1-01 报告](reports/p1-01-request-context-errors.md) | DONE |
| P1-02 | Plan 7；Matrix B01/B11/B13/B17/B27/K07/F02/F10/F11；Behavior 3、8、12、13 | [BC-CORE-002](contracts/BC-CORE-002-canonical-request.md) | `gateway-core` CanonicalRequest、消息、内容、Tool、Thinking 与 Raw Extension | [P1-02 报告](reports/p1-02-canonical-request.md) | DONE |
| P1-03 | Plan 7；Matrix B02/B12/B23/B27；Behavior 4、5、9 | [BC-CORE-003](contracts/BC-CORE-003-canonical-event-state-machine.md) | `gateway-core` CanonicalEvent、Response/Text/Reasoning/Tool/Usage 状态机 | [P1-03 报告](reports/p1-03-canonical-event-state-machine.md) | DONE |
| P1-04 | Plan 7；BL-05；Matrix B29/B30/G27；Behavior 1、6 | [BC-STREAM-001](contracts/BC-STREAM-001-bounded-canonical-stream.md) | `gateway-stream` bounded Canonical Event transport、backpressure、cancellation、FirstSemanticEvent | [P1-04 报告](reports/p1-04-bounded-stream.md) | DONE |
| P1-05 | Plan 7；Matrix A01/A03/A07/B01-B17/B23/B27；Behavior 4、5、9 | [BC-PROTOCOL-001](contracts/BC-PROTOCOL-001-openai-responses-adapter.md) | `protocol-openai-responses` Responses 入站、非流式和 SSE 编解码 | [P1-05 报告](reports/p1-05-openai-responses-adapter.md) | DONE |
| P1-06 | Plan 7；Matrix B01-B17/B23/B27；Behavior 4、5、9 | [BC-PROVIDER-001](contracts/BC-PROVIDER-001-deterministic-mock-provider.md) | `gateway-provider` 小能力 Trait 与 Deterministic Mock | [P1-06 报告](reports/p1-06-deterministic-mock-provider.md) | DONE |
| P1-07 | Plan 7；BL-01/BL-05；Matrix A01/A03/A07/B29/B30；Behavior 1、4、5、9 | [BC-HTTP-001](contracts/BC-HTTP-001-actix-responses-handler.md) | `gateway-http-actix` `/healthz` 与 `/v1/responses` Mock 垂直链路 | [P1-07 报告](reports/p1-07-actix-responses-handler.md) | DONE |
| P1-08 | Plan 7；BL-11/BL-22；Behavior 20 | [BC-AUTH-001](contracts/BC-AUTH-001-client-key-auth-port.md) | `gateway-auth` 内存 Client Key Port 与 `gateway-http-actix` Bearer 入口鉴权 | [P1-08 报告](reports/p1-08-in-memory-client-key-auth.md) | DONE |
| P1-09 | Plan 7；BL-04；Behavior 4、5 | [BC-CORE-003](contracts/BC-CORE-003-canonical-event-state-machine.md)、[BC-PROTOCOL-001](contracts/BC-PROTOCOL-001-openai-responses-adapter.md) | `protocol-openai-responses` Canonical Tool 片段随机切分、交错与显式空参数属性测试 | [P1-09 报告](reports/p1-09-tool-stream-property-tests.md) | DONE |
| G1 | Plan G1；BL-04/BL-05；CR-P1-G1-001 | [BC-CORE-003](contracts/BC-CORE-003-canonical-event-state-machine.md)、[BC-STREAM-001](contracts/BC-STREAM-001-bounded-canonical-stream.md) | P1-03 至 P1-09 完成审计与 G1 条件核验 | [G1 报告](reports/g1-gate-report.md) | DONE |

## P2 追踪

| Task | 需求来源 | ADR/Contract | 实现或检查 | 验证证据 | 状态 |
|---|---|---|---|---|---|
| P2-01 | Plan 8；Matrix `L01-L05`、`E04/E08/E20/E23/E29`、`J03/J18-J20`；Behavior 20 | [ADR-0001](adr/ADR-0001-version-scoped-control-plane-schema.md)、[BC-STORE-001](contracts/BC-STORE-001-versioned-control-plane-schema.md) | `gateway-store` versioned SQLite migration runner and `0001` control-plane schema | [P2-01 报告](reports/p2-01-control-plane-schema.md) | DONE |
| P2-02 | Plan 8；Matrix `D01/D04/D10/D11/D20/D21/D25/D31`、`H05/H06`、`J18-J20`、`L17-L36`；Behavior 3/17/19/20 | [ADR-0002](adr/ADR-0002-version-scoped-route-access-schema.md)、[BC-ROUTE-001](contracts/BC-ROUTE-001-versioned-route-access-schema.md) | Version 2 routing/access migration and uniqueness/FK tests | [P2-02 报告](reports/p2-02-route-access-schema.md) | DONE |
| P2-03 | Plan 8；Matrix `L34`、`J19`；Behavior 14/20 | [ADR-0003](adr/ADR-0003-xchacha20poly1305-secret-store.md)、[BC-SEC-001](contracts/BC-SEC-001-aead-secret-store.md) | `gateway-store` AEAD envelope, key ring, external master-key loader, and rotation helper | [P2-03 report](reports/p2-03-aead-secret-store.md) | DONE |
| P2-04 | Plan 8；Matrix `L35`、`J19`；Behavior 20 | [ADR-0004](adr/ADR-0004-client-key-hmac-credential.md)、[BC-AUTH-002](contracts/BC-AUTH-002-client-key-hmac-credential.md) | `gateway-auth` Client Key issue/parse/HMAC/constant-time verification primitives | [P2-04 report](reports/p2-04-client-key-hmac.md) | DONE |
| P2-05 | Plan 8；Matrix `J18-J20`、`L01-L05`、`L17-L35`；Behavior 14/20 | [ADR-0005](adr/ADR-0005-versioned-control-plane-repository-service.md)、[BC-CONTROL-001](contracts/BC-CONTROL-001-versioned-control-plane-repository-service.md) | Transactional `gateway-store` configuration graph Repository and `gateway-control` provisioning Service | [P2-05 report](reports/p2-05-control-plane-service.md) | DONE |
| P2-06 | Plan 8；Matrix `D01/D04/D10/D11/D20/D21/D31`、`H05/H06`、`J20`、`L17-L30`；Behavior 3/17/19/20 | [ADR-0006](adr/ADR-0006-validated-route-compiler.md)、[BC-ROUTER-001](contracts/BC-ROUTER-001-validated-route-compiler.md) | `gateway-control` validated Route Compiler with injected Catalog/Endpoint-capability views | [P2-06 report](reports/p2-06-validated-route-compiler.md) | DONE |
| P2-07 | Plan 8；Matrix `D01/D04/D10/D11/D20/D21/D31`、`H05/H06`、`J20`、`L17-L31`；Behavior 20 | [ADR-0007](adr/ADR-0007-route-snapshot-publication.md)、[BC-ROUTER-002](contracts/BC-ROUTER-002-route-snapshot-publication.md) | `gateway-router` immutable Snapshot registry plus `gateway-control` publish/rollback orchestration | [P2-07 report](reports/p2-07-route-snapshot-publication.md) | DONE |
| P2-08 | Plan 8；Matrix `E01/E02/E17/E20`、`H05/H06`、`J19`、`L35` | [ADR-0008](adr/ADR-0008-snapshot-client-key-authentication.md)、[BC-AUTH-003](contracts/BC-AUTH-003-snapshot-client-key-authentication.md) | Prefix-indexed Snapshot ClientKeyView and HMAC authenticator without request-path persistence | [P2-08 report](reports/p2-08-snapshot-client-key-authentication.md) | DONE |
| P2-09 | Plan 8；Matrix `L36`、`E10`、`J19`；Behavior 20 | [ADR-0009](adr/ADR-0009-egress-policy-ssrf-admission.md)、[BC-SEC-002](contracts/BC-SEC-002-egress-policy-ssrf-admission.md) | Version-scoped EgressPolicy persistence, publication-time static admission, and DNS-pinned SSRF admission | [P2-09 report](reports/p2-09-egress-policy-ssrf-admission.md) | DONE |
| P2-10 | Plan 8；Matrix `H05`、`J03/J18-J20`；Behavior 20 | [ADR-0010](adr/ADR-0010-local-management-lifecycle.md)、[BC-CONTROL-002](contracts/BC-CONTROL-002-local-management-lifecycle.md) | Transport-neutral local management lifecycle, durable audit events, restart rollback reconstruction, and `gateway admin` CLI | [P2-10 report](reports/p2-10-management-lifecycle.md) | DONE |
| G2 | Plan G2；P2 Matrix `D/E/H/J/L` | P2 ADR/Contract 集合 | P2-01 至 P2-10 完成审计、Secret/SSRF/热路径与 Snapshot 并发条件核验 | [G2 报告](reports/g2-gate-report.md) | DONE |

## P3 追踪

| Task | 需求来源 | ADR/Contract | 实现或检查 | 验证证据 | 状态 |
|---|---|---|---|---|---|
| P3-01 | Plan 9；Matrix `C16`、`D10/D11`、`L06`；Behavior 3/17/20 | [ADR-0011](adr/ADR-0011-openai-compatible-responses-request-assembly.md)、[BC-PROVIDER-002](contracts/BC-PROVIDER-002-openai-compatible-responses-request.md) | `gateway-upstream` safe endpoint URL composition and `provider-openai-compatible` Canonical-to-Responses request builder | [P3-01 report](reports/p3-01-openai-compatible-responses-request.md) | DONE |
| P3-02 | Plan 9；Matrix `C16`、`K03-K06`、`L06`；Behavior 20 | [ADR-0012](adr/ADR-0012-dns-pinned-upstream-client-pool.md)、[BC-UPSTREAM-001](contracts/BC-UPSTREAM-001-dns-pinned-upstream-client-pool.md) | `gateway-upstream` bounded DNS-pinned Client Pool, timeouts, Direct/SOCKS5 isolation, and P3-01 exact-target handoff | [P3-02 report](reports/p3-02-dns-pinned-upstream-client-pool.md) | DONE |
| P3-03 | Plan 9；Matrix `L24`、`L25`；Behavior 3/20 | [ADR-0013](adr/ADR-0013-priority-tier-smooth-weighted-scheduler.md)、[BC-SCHEDULER-001](contracts/BC-SCHEDULER-001-priority-tier-smooth-weighted-scheduler.md) | `gateway-router` immutable priority-tier plans and atomic-cursor Candidate selection | [P3-03 report](reports/p3-03-priority-tier-scheduler.md) | DONE |
| P3-04 | Plan 9；Matrix `D12`、`D14`、`D17`、`E16`、`K06`、`L26`；Behavior 3/14/17/20 | [ADR-0014](adr/ADR-0014-endpoint-credential-pool-leases.md)、[BC-CRED-001](contracts/BC-CRED-001-endpoint-credential-pool-leases.md) | `gateway-control` AEAD/AAD pool compiler, `gateway-upstream` bounded atomic Credential leases, and `gateway-router` two-stage selection | [P3-04 report](reports/p3-04-endpoint-credential-pool.md) | DONE |
| P3-05 | Plan 9；Matrix `E08`、`E11`、`E12`、`K06`、`L30`；Behavior 2/3/17/20 | [ADR-0015](adr/ADR-0015-sharded-runtime-health.md)、[BC-HEALTH-001](contracts/BC-HEALTH-001-sharded-runtime-health.md) | `gateway-router` bounded Endpoint/Credential runtime health shards and health-aware P3-04 scheduling | [P3-05 report](reports/p3-05-runtime-health.md) | DONE |
| P3-06 | Plan 9；BL-05；Matrix `A22`、`E11`、`E12`、`E15`、`E16`、`G21`、`K03-K06`、`L20-L26`、`L30`；Behavior 1/6/17/20 | [ADR-0016](adr/ADR-0016-request-scoped-attempt-orchestration.md)、[BC-ROUTER-003](contracts/BC-ROUTER-003-request-scoped-attempt-orchestration.md) | `gateway-router` request-scoped Attempt selection, exclusion, bounded retry, health mutation, and FSE retry gate | [P3-06 report](reports/p3-06-attempt-orchestrator.md) | DONE |
| P3-07 | Plan 9；Matrix `A02`、`B26`、`L27-L31`；Behavior 3/17/19/20 | [ADR-0017](adr/ADR-0017-routesnapshot-public-model-view.md)、[BC-ROUTE-002](contracts/BC-ROUTE-002-routesnapshot-public-model-view.md) | `gateway-router` pinned public-model projection and `gateway-http-actix` Models/Responses public-name boundary | [P3-07 report](reports/p3-07-routesnapshot-public-model-view.md) | DONE |
| P3-08 | Plan 9；Matrix `G19`、`G21`；Behavior 1/5/9；`BL-09`、`BL-10` | [ADR-0018](adr/ADR-0018-bounded-request-attempt-usage-events.md)、[BC-OBS-001](contracts/BC-OBS-001-bounded-request-attempt-usage-events.md) | `gateway-core` event port, `gateway-observability` bounded priority queues, and HTTP/router Request/Attempt/Usage hooks | [P3-08 report](reports/p3-08-bounded-request-attempt-usage-events.md) | DONE |
| P3-09 | Plan 9；Matrix `C16`、`G05`、`G12-G15`、`G21`、`K03-K06`、`L20-L31`；Behavior 1/4/5/9/17/20 | [ADR-0019](adr/ADR-0019-controlled-mock-http-aggregation-e2e.md)、[BC-E2E-001](contracts/BC-E2E-001-controlled-mock-http-aggregation-e2e.md) | Router execution context plus two controlled loopback OpenAI-compatible HTTP Upstreams that compose request build, admitted transport, scheduling, Attempt failover, bounded HTTP output, and events | [P3-09 report](reports/p3-09-controlled-mock-http-aggregation-e2e.md) | DONE |
| P3-10 | Plan 9；Matrix `C16`、`G05`、`G12-G15`、`G21`、`K03-K06`、`L20-L31`；Behavior 1/4/5/9/17/20 | [ADR-0020](adr/ADR-0020-authorized-real-test-endpoint-validation.md)、[BC-E2E-002](contracts/BC-E2E-002-authorized-real-test-endpoint-validation.md) | Shared P3-09/P3-10 test-only aggregation harness plus ignored, explicitly authorized two-target real Endpoint validation; fixed four-call cap, one Attempt per request, bounded reads, and redaction assertions | [P3-10 report](reports/p3-10-real-test-endpoint-validation.md) | DONE |
| G3 | Plan G3；P3 Matrix `C/D/E/G/K/L` | P3 ADR/Contract 集合 | P3-01 至 P3-10 的独立上游聚合、调度、凭据、健康、Attempt、事件与受控真实验证条件核验 | [G3 报告](reports/g3-gate-report.md) | DONE |

## P4 追踪

| Task | 需求来源 | ADR/Contract | 实现或检查 | 验证证据 | 状态 |
|---|---|---|---|---|---|
| P4-00 | Plan 10；`CR-EXEC-001` | [ADR-0021](adr/ADR-0021-delivery-gate-classification-and-single-probe-diagnostic.md)、[BC-DELIVERY-001](contracts/BC-DELIVERY-001-delivery-gates-and-single-probe-diagnostic.md) | Fail-closed docs/code/tag CI classification, version-verified quality-tool cache, task-state guard, and isolated ignored one-probe diagnostic | [P4-00 报告](reports/p4-00-execution-acceleration.md) | DONE |
| P4-01 | Plan 10；Matrix `E20`、`G28`、`L09`、`L10`、`L33` | [ADR-0022](adr/ADR-0022-endpoint-credential-catalog-singleflight.md)、[BC-CATALOG-001](contracts/BC-CATALOG-001-endpoint-credential-catalog-singleflight.md) | `gateway-catalog` exact Endpoint/Credential Model discovery source boundary, in-flight singleflight, deterministic normalization, cancellation-safe publisher, and no result cache | [P4-01 报告](reports/p4-01-catalog-singleflight.md) | DONE |
| P4-02 | Plan 10；`CR-EXEC-002`；Matrix `E20`、`G28`、`L09`、`L10`、`L33`；Behavior 18 | [ADR-0023](adr/ADR-0023-cache-visible-delivery-and-supply-chain-split.md)、[BC-DELIVERY-002](contracts/BC-DELIVERY-002-cache-visible-delivery-and-supply-chain-split.md)、[ADR-0024](adr/ADR-0024-catalog-snapshot-freshness-and-last-success-fallback.md)、[BC-CATALOG-002](contracts/BC-CATALOG-002-catalog-snapshot-freshness-and-last-success-fallback.md) | `gateway-catalog` exact-target immutable last-success snapshots, explicit Fresh/Stale/Expired and refresh-due time, failure retention, plus Fast/Full delivery split | [P4-02 报告](reports/p4-02-catalog-snapshot-freshness.md) | DONE |
| P4-03 | Plan 10；Matrix `E20`、`G28`、`L09`、`L10`、`L33`；Behavior 18 | [ADR-0025](adr/ADR-0025-catalog-diff-preview-apply-removal-isolation.md)、[BC-CATALOG-003](contracts/BC-CATALOG-003-catalog-diff-preview-apply-removal-isolation.md) | `gateway-catalog` target-local successful-snapshot diff registry, generation-guarded Preview/Apply, three-successful-miss plus 24-hour removal isolation, and no static/manual deletion | [P4-03 报告](reports/p4-03-catalog-diff-preview-apply.md) | DONE |
| P4-04 | Plan 10；Matrix `E08`、`E09`、`E11`、`E12`、`D20`、`D24`、`G20`、`H19`、`L30` | [ADR-0026](adr/ADR-0026-target-local-probe-ewma-and-circuit-recovery.md)、[BC-HEALTH-002](contracts/BC-HEALTH-002-target-local-probe-ewma-and-circuit-recovery.md) | `gateway-router` exact probe-target EWMA, model binding Circuit isolation, controlled half-open recovery ticket, and pre-lease model availability | [P4-04 报告](reports/p4-04-probe-ewma-circuit-recovery.md) | DONE |
| P4-05 | Plan 10；Matrix `E19`、`G20`、`G26`；`BL-17` | [ADR-0028](adr/ADR-0028-exact-target-runtime-quota-and-controlled-reset-recovery.md)、[BC-CRED-002](contracts/BC-CRED-002-exact-target-runtime-quota-and-controlled-reset-recovery.md) | `gateway-router` bounded exact-target quota snapshots, source/confidence/reset evidence, 429 ownership, pre-lease quota filtering, and controlled recovery ticket | [P4-05 报告](reports/p4-05-runtime-quota-recovery.md) | DONE |
| P4-06 | Plan 10；Matrix `E15`、`E16`、`E23`、`G20`、`G21`、`L20-L26` | [ADR-0029](adr/ADR-0029-fixed-input-route-explain.md)、[BC-ROUTE-003](contracts/BC-ROUTE-003-fixed-input-route-explain.md) | `gateway-router` fixed-time/slot Route Explain, target-local Candidate/Binding reasons, request exclusions, and side-effect-free projected selection; `gateway-upstream` secret-free pool diagnostics | [P4-06 报告](reports/p4-06-route-explain.md) | DONE |
| P4-07 | Plan 10；Matrix `G19`、`G20`、`G21`；`BL-09`、`BL-10` | [ADR-0027](adr/ADR-0027-append-only-bounded-sqlite-event-writer.md)、[BC-OBS-002](contracts/BC-OBS-002-append-only-sqlite-event-writer.md) | `gateway-core` Health event contract plus `gateway-store` append-only schema, atomic/idempotent queryable event Store, and bounded asynchronous EventQueue receiver writer | [P4-07 报告](reports/p4-07-append-only-sqlite-event-writer.md) | DONE |
| P4-08 | Plan 10；Matrix `G19`、`G20`、`G21` | [ADR-0030](adr/ADR-0030-single-consumer-telemetry-fanout.md)、[BC-OBS-003](contracts/BC-OBS-003-single-consumer-telemetry-fanout.md) | `gateway-observability` fixed safe JSON/Prometheus/OpenTelemetry projections; `gateway-store` optional single-consumer fan-out so one admitted event remains observable and durable | [P4-08 报告](reports/p4-08-single-consumer-telemetry-fanout.md) | DONE |
| P4-09 | Plan 10；Matrix `G01`、`G03`、`G15`、`G22`、`K08` | [ADR-0031](adr/ADR-0031-default-deny-log-redaction-and-body-sampling.md)、[BC-OBS-004](contracts/BC-OBS-004-default-deny-log-redaction-and-body-sampling.md) | `gateway-observability` default-deny Header/Body log records, finite deterministic JSON sampling, recursive redaction, and Secret-leak regressions | [P4-09 报告](reports/p4-09-log-redaction-body-sampling.md) | DONE |
| P4-10 | Plan 10；`CR-P4-G4-001`; Matrix `G20`、`G21`、`G26`、`H19`、`H20` | [ADR-0032](adr/ADR-0032-read-only-runtime-management-status.md)、[BC-MGMT-001](contracts/BC-MGMT-001-read-only-runtime-management-status.md) | `gateway-router` read-only in-process exact account/Health/Quota projection, exact 403 binding state, controlled recovery ticket, and Route Explain account exclusion | [P4-10 报告](reports/p4-10-read-only-runtime-management-status.md) | DONE |
| G4 | Plan G4；P4 Matrix `C/D/E/G/H/L`；`CR-P4-G4-001` | P4 ADR/Contract 集合，特别是 [ADR-0032](adr/ADR-0032-read-only-runtime-management-status.md) / [BC-MGMT-001](contracts/BC-MGMT-001-read-only-runtime-management-status.md) | P4-00 至 P4-10 的 Catalog、Health/Quota、Explain、Event/SQLite、观测和只读管理状态条件核验 | [G4 报告](reports/g4-gate-report.md) | DONE |
| P5-00 | Plan 11；`CR-EXEC-007` | [ADR-0033](adr/ADR-0033-phase-level-delivery-and-default-ref-cache.md)、[BC-DELIVERY-003](contracts/BC-DELIVERY-003-phase-level-delivery-and-default-ref-cache.md) | Main/Phase-tag CI trigger, default-ref cache seed, Phase-local task-state guard, and fail-closed Fast + Full delivery | [P5-00 报告](reports/p5-00-phase-level-delivery.md) | DONE |
| P5-01 | Plan 11；Matrix `A07`/`B04`/`B09`/`B11`/`B13`/`B25`/`B27` | [ADR-0034](adr/ADR-0034-anthropic-messages-pure-codec.md)、[BC-PROTOCOL-002](contracts/BC-PROTOCOL-002-anthropic-messages-adapter.md) | `protocol-anthropic` pure Messages request/response/SSE codec, duplicate-name rejection, raw extension retention, and text/Usage fixtures | [P5-01 报告](reports/p5-01-anthropic-messages-adapter.md) | LOCAL_PASS_PENDING_PHASE_GATE |
| P5-02 | Plan 11；Matrix `A08`、`B22`、`B25` | [ADR-0035](adr/ADR-0035-exact-token-count-capability.md)、[BC-PROTOCOL-003](contracts/BC-PROTOCOL-003-exact-token-count-capability.md) | Canonical exact input-token count, explicit Provider capability/refusal, Snapshot Route identity handoff, and safe Anthropic `count_tokens` HTTP boundary | [P5-02 报告](reports/p5-02-exact-token-count-capability.md) | LOCAL_PASS_PENDING_PHASE_GATE |
| P5-03 | Plan 11；Matrix `B12`、`B14-B16`；Behavior 4、5 | [ADR-0036](adr/ADR-0036-anthropic-tool-stream-state.md)、[BC-PROTOCOL-004](contracts/BC-PROTOCOL-004-anthropic-tool-stream-state.md) | `protocol-anthropic` per-call Anthropic Tool start/delta/end state, stable IDs/indexes, `{}` normalization, frozen fixture, and 1-byte/interleaving property coverage | [P5-03 报告](reports/p5-03-anthropic-tool-stream-state.md) | LOCAL_PASS_PENDING_PHASE_GATE |
| P5-04 | Plan 11；Matrix `A07`、`B24-B28`、`F01-F04`、`L08`、`L21-L22`、`L40` | [ADR-0037](adr/ADR-0037-protocol-transform-admission.md)、[BC-ROUTER-004](contracts/BC-ROUTER-004-protocol-transform-admission.md) | `gateway-router` pure same-protocol Canonical / cross-protocol Lossless Bridge admission matrix, exact native pass-through proof, capability checks, and value-only rejection codes | [P5-04 报告](reports/p5-04-protocol-transform-admission.md) | LOCAL_PASS_PENDING_PHASE_GATE |
| P5-05 | Plan 11；Matrix `L08`、`L21-L22`、`L40` | [ADR-0038](adr/ADR-0038-endpoint-format-isolated-protocol-routing.md)、[BC-ROUTER-005](contracts/BC-ROUTER-005-endpoint-format-isolated-protocol-routing.md) | Exact Endpoint `api_format` propagation into `RouteSnapshot`, native protocol Candidate filtering before leases, and same-Upstream Endpoint Health/Circuit isolation | [P5-05 报告](reports/p5-05-endpoint-protocol-isolation.md) | LOCAL_PASS_PENDING_PHASE_GATE |
| P5-06 | Plan 11；Matrix `A07`、`B09`、`B23-B26`、`F01-F04`、`F09-F11` | [ADR-0039](adr/ADR-0039-anthropic-semantic-http-boundary.md)、[BC-PROTOCOL-005](contracts/BC-PROTOCOL-005-anthropic-semantic-http-boundary.md) | Explicit Canonical stop semantics, Thinking/cache Usage preservation, authenticated Messages JSON/SSE boundary, public-model rewrite, and Anthropic request attribution | [P5-06 报告](reports/p5-06-anthropic-semantic-http-boundary.md) | LOCAL_PASS_PENDING_PHASE_GATE |
| P5-07 | Plan 11；Matrix `A07`、`B09-B16`、`B25-B28` | [ADR-0040](adr/ADR-0040-claude-code-loopback-client-boundary.md)、[BC-E2E-003](contracts/BC-E2E-003-claude-code-loopback-bare-compatibility.md) | Body-free Claude Code base-URL probe, strict single-scheme Bearer/`x-api-key` admission, and ignored loopback `--bare` normal/Tool/parallel/Plan E2E | [P5-07 报告](reports/p5-07-claude-code-bare-e2e.md) | LOCAL_PASS_PENDING_PHASE_GATE |

## 后续 Phase 映射

| 矩阵模块 | 主要 Phase |
|---|---|
| A 接口与服务器 | P1、P3、P5、P12 |
| B 协议与流 | P1、P5、P6-P9 |
| C Provider | P3、P5-P9 |
| D 模型与路由 | P2-P4 |
| E 凭据与错误 | P2-P9 |
| F Thinking 与缓存 | P5-P9 |
| G 可观测性 | P3、P4、P10、P11 |
| H Management | P2、P4、P10 |
| I 插件 | Release 1 Drop/Deferred |
| J 配置与部署 | P0、P2、P10-P12 |
| K 性能 | P1、P3、P11 |
| L 上游聚合 | P2-P5；延后项进入 P13 |

具体 Task 开始时在本文件新增或更新一行；若无法映射到权威来源，必须先创建 Change Request。
