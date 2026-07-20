# P3-09 controlled Mock HTTP aggregation E2E report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-09` |
| Matrix / behavior | `C16`, `G05`, `G12-G15`, `G21`, `K03-K06`, `L20-L31`; Behavior 1/4/5/9/17/20 |
| Date | `2026-07-20` |
| Branch | `codex/p3-09-mock-upstream-e2e` |
| Rust | `1.97.1` |
| Result | PASS locally; GitHub implementation acceptance pending |

## Delivered scope

- Added Router-owned `ResponsesExecution`, so Snapshot-resolved Route identity, public response
  mode, and the downstream-owned retry/cancellation gate reach a routed executor without adding
  an Actix or `gateway-stream` dependency to `gateway-router`. Existing P1 executors retain the
  legacy default method.
- Moved finite Canonical-stream creation ahead of executor invocation in the Actix Responses path,
  ensuring the exact body-owned `TransparentRetryGate` is available for P3 aggregation attempts.
- Added the test-only `p3_09_aggregation_e2e` composition target. It runs two independent loopback
  OpenAI-compatible HTTP peers through exact P2 egress admission and P3-01 request construction,
  rather than replacing the transport with an in-memory Driver.
- Added deterministic coverage for equal-priority round-robin, pre-`ResponseStart` HTTP 5xx
  failover with correlated Request/Attempt/Usage records, and dropping a live SSE response body
  closing the active upstream connection without a fallback request.
- Added [ADR-0019](../adr/ADR-0019-controlled-mock-http-aggregation-e2e.md) and
  [BC-E2E-001](../contracts/BC-E2E-001-controlled-mock-http-aggregation-e2e.md), then marked
  P3-09 as the plan's sole in-progress task.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-http-actix --test p3_09_aggregation_e2e` | PASS; two controlled HTTP peers prove round-robin, 5xx fallback, exact upstream-model rewriting, Request/Attempt/Usage correlation, and SSE cancellation propagation |
| `cargo test --locked -p gateway-router -p gateway-http-actix` | PASS; legacy Router/HTTP behavior remains compatible with the routed execution seam |
| `cargo clippy --locked -p gateway-router -p gateway-http-actix --all-targets --all-features -- -D warnings` | PASS |
| `ruby scripts/check-crate-boundaries.rb` and `ruby scripts/check-source-policy.rb` | PASS; the concrete Provider/transport references are test-only dev dependencies |
| `cargo fmt --all -- --check`, `ruby scripts/check-doc-links.rb`, and `scripts/secret-scan.sh --all` | PASS; 100 Markdown files and 57 Rust files checked |
| `./scripts/check.sh fast` | PASS; complete workspace Fast gate, including the new E2E test target |
| `./scripts/check.sh full` | PASS; complete workspace Full gate, including dependency policy and RustSec audit; existing duplicate-version notices are policy-allowed warnings |

## Review

Review passed. The execution handoff contains only Router/core types: the default
`ResponsesExecutor::execute_routed` path consumes the extra context and calls the unchanged legacy
method, while the P3 test asserts that a Snapshot-resolved Route reaches the routed executor. The
Actix handler creates exactly one finite Canonical stream before the executor runs, then reuses its
sender/receiver for normal JSON/SSE transport; an executor failure drops that unopened stream rather
than leaving a producer task behind.

The E2E harness has no deployed network capability. Each peer requires an exact synthetic Host,
port, and loopback CIDR admission, retains only upstream model labels/counters, and bounds inbound
request, JSON response, and SSE-frame buffers. The review verified that the HTTP crate's library
source contains no concrete Provider/Upstream reference; those dependencies occur only in the test
target. The 5xx test observes one failed then one successful Attempt with the same Request/Usage
correlation, and the SSE test proves a body drop closes the raw peer without a fallback request.

The decoder remains intentionally fixture-limited and test-only. It is not presented as a complete
production OpenAI Responses decoder; P3-10 remains a separate authorized real-endpoint validation
task.

## Scope and deferred work

P3-09 does not contact a deployed Endpoint, use a production API Key/Credential, start a gateway
server process, persist events, write SQLite, add a generic production OpenAI response decoder,
or broaden Provider dependencies in `gateway-http-actix`'s library target. P3-10 owns minimal
authorized real-test Endpoint validation; P4 owns persistent event writing, richer status policy,
and observability exports.
