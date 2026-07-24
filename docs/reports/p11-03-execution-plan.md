# P11-03 Benchmark Baseline execution plan

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P11-03` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` — implementation, measured baseline, full local verification, and review are complete; G11 remains pending P11-04 through P11-08. |
| Branch | `codex/p11-release-hardening` |
| Task Card | A reproducible, offline Criterion measurement of the deterministic Mock Provider and the in-process Actix `POST /v1/responses` hot path, plus an immutable reviewed baseline and a fail-closed comparator. |
| References | [P11 plan](../06-development-plan.md#17-p11---发布加固), [P1 vertical slice](../06-development-plan.md#7-p1---最小可运行垂直切片), [P3 aggregation constraints](../contracts/BC-E2E-001-controlled-mock-http-aggregation-e2e.md), [performance discipline](../06-development-plan.md#21-性能与资源纪律) |

## Required acceptance

Create two Criterion benchmarks with no external I/O, ambient configuration, accounts, credentials,
or server access:

1. `mock_provider_canonical_drain` exercises the real `DeterministicMockProvider` with a valid,
   zero-delay Canonical text lifecycle and drains every event.
2. `http_responses_warm_path` exercises the real in-process Actix `/v1/responses` route, parser,
   Client-Key admission, Canonical-to-OpenAI response encoding, and the router's deterministic
   Mock Provider executor. It creates no listener or upstream transport.

`benchmarks/baseline.json` must retain only source revision, machine/runtime facts, benchmark
method, latency/throughput/RSS measurements, and fixed thresholds. A value-free comparator must
reject malformed candidates, benchmark-set changes, latency P99/RSS growth above 15%, throughput
loss above 10%, and a local `http_responses_warm_path` P99 above 5 ms. It must not overwrite the
approved baseline: a changed baseline requires review.

## Execution sequence

1. Add the locked Criterion dependency and two `harness = false` benchmark targets. Construct
   fixtures in process and prevent compiler elision with `std::hint::black_box`.
2. Add a portable benchmark runner that records Criterion samples and peak RSS into a candidate
   JSON document, and a separate fail-closed comparator/fixture test. The runner is opt-in and
   does not enter ordinary test or CI paths.
3. Run the benchmarks under the current controlled Mac environment, validate the measured
   candidate against itself to establish `benchmarks/baseline.json`, then run the comparator,
   focused tests/Clippy/format/docs/Secret checks and independent review.

## Explicitly out of scope

Real Provider calls, loopback/public upstream transport timing, load generation, connection-pool
or long-stream measurements, server RSS/P99 attestation, 24-hour soak, deployment changes, and
P11-04 through P11-08. P11-04 must separately validate the server limit of 10 ms and the 24-hour
resource/connection assertions.
