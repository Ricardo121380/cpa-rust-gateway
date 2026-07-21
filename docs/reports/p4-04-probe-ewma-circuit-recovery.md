# P4-04 Target-local Probe, EWMA, and Circuit recovery

| Field | Value |
|---|---|
| Plan version | `v1.4` |
| Task | `P4-04` |
| Status | `LOCAL_PASS_PENDING_CI`; local implementation, review, and complete Gate passed; GitHub Code Gate pending |
| Scope level / execution budget | `M`; `<=25min` from Task Card to code commit |
| Task Card | `gateway-router` runtime health/probe boundary only; no real Provider request, Quota, Route Explain, SQLite, public API, P4-05+ behavior |
| References | `E08`, `E09`, `E11`, `E12`, `D20`, `D24`, `G20`, `H19`, `L30`; [ADR-0026](../adr/ADR-0026-target-local-probe-ewma-and-circuit-recovery.md); [BC-HEALTH-002](../contracts/BC-HEALTH-002-target-local-probe-ewma-and-circuit-recovery.md) |

## Scope

P4-04 adds transport-neutral, explicit-time health probe outcomes for exact Endpoint,
Endpoint/Credential, and Endpoint/Credential/model targets. It stores bounded fixed-point success
and latency EWMA, adds a model-specific runtime Circuit key to pre-lease scheduling, and turns an
expired Circuit retry instant into one controlled half-open ticket rather than ordinary traffic.

No request is sent: a later authorized executor may supply sanitized terminal results. This Task
does not classify quota, 403, or 429; persist health events; expose management HTTP; render Route
Explain; export telemetry; or retain URLs, Headers, bodies, Provider diagnostics, or Secret values.

## Local verification

| Command / review | Result |
|---|---|
| `cargo test --locked -p gateway-router` | PASS; 43 tests, including exact model-target EWMA isolation, time-regression rejection, one half-open ticket, stale-ticket rejection, success close, failure reopen, and healthy-sibling pre-lease selection. |
| `cargo clippy --locked -p gateway-router --all-targets --all-features -- -D warnings` | PASS after one direct review-correction batch. |
| `CHECK_REPORT_PATH=tmp/p4-04-full-check.md ./scripts/check.sh full` | PASS in 42 seconds (started `2026-07-21T15:55:54+08:00`, completed `2026-07-21T15:56:36+08:00`); it covers Fast, formatting, workspace Clippy/tests, links, Secret scan, whitespace, pinned tools, `cargo deny`, and RustSec audit. |

The Full report contains existing duplicate-version notices from `cargo deny`; `cargo audit` completed
successfully. No ignored real-test harness was invoked and no Provider request was sent.

## Review and execution measurement

Review confirmed that model state is exact `(EndpointId, CredentialId, upstream_model)` rather than
global/model-only; ordinary scheduling remains closed throughout half-open recovery; a stale ticket
cannot overwrite a newer Circuit; and EWMA uses explicit time plus integer arithmetic. Probe and
runtime state remain bounded and sharded. The new model predicate runs before a pool reserves a
Credential lease. No RouteSnapshot/public-model mutation, HTTP/Provider access, SQLite, quota,
management endpoint, persistent event, telemetry, body, Header, URL, or Secret enters the scope.

| Measurement | Evidence / value |
|---|---|
| Scope / budget | `M`; target `<=25min` from Task Card to code commit. |
| Task Card | `2026-07-21T15:32:40+08:00` durable plan-state anchor; the visible Task Card preceded focused code reading. |
| Local complete Gate | `2026-07-21T15:55:54+08:00` to `2026-07-21T15:56:36+08:00` (42s). |
| Repeated complete Gates | `0`; the one required complete Gate was not mechanically replayed. |
| Rework | `1` direct correction batch: focused Clippy feedback merged equivalent match arms, removed an unused test local, and simplified private helper shape without changing behavior. |
| Budget observation | The code-commit target will exceed the `M` budget by a small margin because the new model key, ticket generation, atomic completion boundary, scheduler integration, and required no-network contract were reviewed together; the next Task should reuse this health-report template rather than rediscover the boundary. |
| Code commit / Code Gate / docs closeout / docs Gate | Pending immutable evidence after this code delivery and its normal GitHub workflow. |

## Remote Code Gate

The normal cache-visible Code Gate will be started from this code delivery. No manual rerun will be
issued.

## Closeout boundary

After Code Gate success, one docs-only closeout will record immutable evidence and mark P4-04
`DONE`. P4-05 remains `PENDING` until its own Task Card is started.
