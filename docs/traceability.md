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
