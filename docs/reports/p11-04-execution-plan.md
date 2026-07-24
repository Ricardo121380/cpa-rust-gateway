# P11-04 Load and Soak execution plan

| Field | Value |
|---|---|
| Plan version | `v1.45` |
| Task | `P11-04` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Branch | `codex/p11-release-hardening` |
| Task Card | A deterministic, loopback-only workload harness and evidence recorder for concurrency, finite long streams, connection reuse, bounded backpressure, RSS sampling, and a user-accepted ≥10-hour local soak. |
| References | [P11 plan](../06-development-plan.md#17-p11---发布加固), [P11-02 fault matrix](p11-02-fault-matrix.md), [P11-03 benchmark baseline](p11-03-benchmark-baseline.md), [bounded Canonical stream](../contracts/BC-STREAM-001-bounded-canonical-stream.md), [P3 aggregation boundary](../contracts/BC-E2E-001-controlled-mock-http-aggregation-e2e.md) |

## Required acceptance

The workload must use only in-process state and ephemeral `127.0.0.1` peers. It must prove:

1. bounded multi-request concurrency produces complete responses without a hidden retry, task leak,
   or unbounded queue;
2. a finite long Canonical stream remains ordered and terminal while a slow consumer preserves
   backpressure;
3. the existing `UpstreamClientPool` reuses a keep-alive loopback connection after warm-up rather
   than opening one connection per request;
4. sampled process RSS has no sustained monotonic growth after warm-up, and failure/cancellation
   always joins workload tasks; and
5. an actual ≥10-hour, low-rate, bounded-concurrency loopback soak provides an auditable
   timestamped summary. A short smoke run validates the harness before the long run begins.

The long-run invocation is opt-in, foreground-safe, resumable only as a new receipt, and performs
no HTTP outside its own loopback peer. It must make a periodic line-oriented status file with only
counters, timing and RSS values; no request or response values are retained. A `INT`, `TERM`, or
`HUP` interruption emits a terminal `runner_state=INCOMPLETE` line before nonzero exit, so a
stopped run cannot resemble a completed soak. After two warm-up samples, every contiguous six
sample RSS window is fail-closed if it is monotonic and grows by more than 15%.

`CR-P11-04-001` accepts the completed 36,818-second observation as sufficient local evidence and
explicitly retains its user-requested `INT` terminal state. It does not reinterpret the receipt as
a completed 24-hour run, weaken the deterministic checks, or remove P12-10's real 72-hour Canary
observation.

## Implementation and validation sequence

1. Reuse the real bounded stream and P3/P11 test seams; add no production load generator, route,
   listener, environment credential discovery, or new Provider request path.
2. Add a deterministic integration harness for concurrency/long stream/pool/backpressure plus an
   ignored command surface that is constrained to a loopback test binary, fixed client count and
   finite duration. Include an explicit short smoke mode and test its malformed/unsafe argument
   rejection.
3. Run the focused tests, interruption-receipt regression, receipt-checker fixtures, and smoke
   receipt. Then run the bounded local soak, inspect every periodic status and its terminal result,
   run the appropriate local checks, review the resource and cancellation boundary, and only then
   mark P11-04 local-pass pending G11. Under `CR-P11-04-001`, an explicit user stop after the
   ten-hour minimum is valid evidence only when the receipt retains its `INCOMPLETE` terminal line.

## Explicitly out of scope

Public or Provider endpoint load, account/API Key/OAuth use, browser/TUN/proxy changes, server
load without a separate deployment approval, production configuration, P11-05 security audit,
P11-06 recovery drills, P11-07 migration drills, or P11-08 release packaging. A failed, stopped,
or interrupted 24-hour run is recorded as such and is never represented as a completed soak;
`CR-P11-04-001` only changes which truthful local receipt is sufficient before P12's real Canary.
