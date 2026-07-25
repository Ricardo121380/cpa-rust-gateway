# P10-06 Runtime observability management workflows

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-06` |
| Date | `2026-07-24` |
| Branch | `codex/p10-control-plane` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope | Protected, secret-free runtime observations, fixed-input Route Explain, value-free request attempts, and a bounded quota-recovery controller request. |

## Delivered boundary

`ManagementResourceHttpState` now owns an explicit `ManagementRuntimeFacade` and a
`ManagementRuntimeClock`. The default facade rejects every operation with the fixed safe
`503 management_runtime_unavailable` result. An embedding must explicitly supply an isolated
runtime-management facade before any P10-06 view can return data.

The protected P10-02 `/admin` scope exposes exactly these P10-06 operations:

| Operation | Result boundary |
|---|---|
| `GET /admin/catalog/status` | Bounded Endpoint/Credential IDs, freshness class, and observation time only. |
| `GET /admin/runtime/availability` | Bounded binding-scoped availability category for Health, Quota, 403 and recovery state. |
| `POST /admin/runtime/quota/reset` | Validates the configured Endpoint/Credential binding, then requests controller-owned recovery only. It neither probes nor completes recovery. |
| `GET /admin/routes/{route_id}/explain` | Explicit-time, fixed-input Candidate decision projection; it never acquires a lease or advances a scheduler cursor. |
| `GET /admin/requests/{request_id}/attempts` | At most 128 value-free attempt rows, with fixed outcome categories and an optional closed execution-stage category. |

The recovery action does not take `If-Match`, does not publish a Snapshot, and does not advance
the configuration revision. It records the existing non-graph `quota_recovery_requested` audit
action only after the injected controller accepts the bounded request.

No handler or facade input has a Provider URL, Header, Body, Secret, credential ciphertext,
network client, Cookie, lease, scheduler cursor, or publication/backup handle. Output rows reject
duplicate binding/attempt identities, invalid categories, negative observation times, and excess
rows rather than serializing an ambiguous or unbounded view.

P12-05 may use the existing protected attempt route to supply an optional fixed stage category:
request conversion, egress admission, HTTP transport/status/content-type, body read, decoder, or
SSE bootstrap. It remains a process-local, value-free observation; its non-blocking bounded store
fails closed as unavailable on loss or contention and never changes the data-plane result.

## SPA and browser evidence

The generated management client is the only SPA request path. The Runtime panel reads Catalog
status and availability, sends a clearly labelled recovery **request**, explains a Route, and
looks up value-free attempts. Its copy states that it cannot send a Provider request, clear a 403
state, complete recovery, publish, roll back, or restore a backup.

A loopback-only fixture on `127.0.0.1:4181` served the built static SPA plus deterministic,
synthetic safe responses. A fresh isolated Chromium session successfully exercised all five
operations: Catalog status (`200`), availability (`200`), recovery request (`202`), Route Explain
(`200`), and attempt lookup (`200`). Its captured recovery request had the expected generated
client headers and the exact JSON target shape; there was no revision precondition. The final
attempt result contained only attempt ID, closed outcome, Endpoint ID and Credential ID.

After reload the page showed `Not connected`, rejected a new read locally before issuing a
management request, and retained no Local Storage, Session Storage, or Cookie values. The clean
browser console had zero errors. The fixture has no Provider client, persistence, external egress,
credential source, proxy, or recovery/probe implementation; it is browser-contract evidence only.

## Verification and review

| Check | Result |
|---|---|
| `cargo test --locked -p gateway-http-actix --test p10_06_management_runtime` | PASS — protected access, closed schemas, fixed observation time, safe projection mapping, request-only recovery, unchanged config revision, redacted attempts, and default fail-closed facade. |
| `cargo clippy --locked -p gateway-http-actix --test p10_06_management_runtime -- -D warnings` | PASS. |
| `cargo check --locked -p gateway-http-actix` | PASS. |
| `cd web/admin-ui && npm run check` | PASS — generated client, deterministic double static build and SPA boundary checks. |
| Loopback browser E2E | PASS — all five P10-06 operations, no browser persistence, reload disconnect, and clean console. |
| `cargo fmt --all -- --check`, `git diff --check` | PASS. |
| `./scripts/secret-scan.sh --all`, `./scripts/check.sh docs` | PASS. |
| Focused implementation review | PASS — P10-06 is runtime-facade-only, response-bounded, target/value-free, default-deny and excludes P10-07 publication/rollback/audit-history and P10-08 backup/restore controls. |

## Remaining boundary

This task does not make a production listener, deploy a runtime facade, send a Provider health or
quota probe, enable account recovery, publish a configuration, expose audit history, or implement
backup/restore. P10's single remote Delivery Gate remains deferred until all P10 tasks complete.
