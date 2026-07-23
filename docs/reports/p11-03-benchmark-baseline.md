# P11-03 Mock Provider and HTTP benchmark baseline

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P11-03` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` — implementation, measured baseline, full local verification, and review completed; G11 remains pending P11-04 through P11-08. |
| Branch | `codex/p11-release-hardening` |
| Scope | Offline deterministic Mock Provider and in-process Actix `POST /v1/responses` warm path only. |
| Machine | macOS/Darwin `arm64`, `rustc 1.97.1 (8bab26f4f 2026-07-14)`; see [P0 environment baseline](environment-baseline.md). |
| Source revision | `ef5440228506ddec164f3b05e1a5c134869a4f99` plus the P11-03 task worktree under review. |

## What was measured

The committed [`baseline.json`](../../benchmarks/baseline.json) uses Criterion `0.7.0`, 30 samples,
one-second warm-up, and three-second measurement windows. Each sample is converted to one
per-operation latency; the recorded P99 is the 99th percentile of those raw per-operation samples.

| Benchmark | Real path | P50 | P99 | Lifecycle throughput | Peak RSS |
|---|---|---:|---:|---:|---:|
| `mock_provider_canonical_drain` | `DeterministicMockProvider` creates and drains a five-event zero-delay Canonical text lifecycle. | 510 ns | 538 ns | 1,962,540 / s | 12,288,000 B |
| `http_responses_warm_path` | In-process Actix `/v1/responses`: request parser, Bearer Client-Key admission, Router deterministic Mock Provider executor, Canonical lifecycle and OpenAI JSON response encoder. | 12.194 µs | 12.276 µs | 82,004 / s | 18,481,152 B |

`http_responses_warm_path` is below the P11 local warm-path maximum of 5 ms by more than two
orders of magnitude. These measurements do not claim the P11 server maximum of 10 ms; that is an
explicit P11-04 responsibility.

## Reproducibility and regression rule

```text
./scripts/run-p11-03-benchmarks.sh --candidate /tmp/p11-03-candidate.json --compare
```

The runner first builds with `cargo bench --no-run`; it then invokes only the emitted benchmark
binary with `--bench --noplot` under the operating system's resource meter. Thus each RSS value
is the benchmark process peak, excluding Cargo's compiler and dependency build processes.

`check-p11-03-benchmark-baseline.rb` rejects malformed documents, a changed benchmark identity,
changed local environment/method/thresholds, P99 or RSS growth over 15%, throughput loss over
10%, or HTTP P99 above 5 ms. It never updates the approved baseline; replacement is a reviewed
source change. Its positive and negative fixtures are executed by
`test-p11-03-benchmark-baseline.sh` and are part of the Fast/Full local gate.

## Boundaries

No listener, upstream transport, network call, browser/Proxy/TUN setting, environment credential,
Provider account, server process, database, or production route enters either benchmark. The HTTP
benchmark uses the existing P1 Router-facing deterministic executor, so it measures the actual
public handler shape without misrepresenting it as P3 upstream aggregation or a production load
test.

Connection pooling, concurrent load, long streams, backpressure under load, server resource
limits and the 24-hour soak remain P11-04 work. The Criterion sample set is a regression baseline,
not a substitute for those operational measurements.

## Verification and review

| Check | Result |
|---|---|
| `./scripts/run-p11-03-benchmarks.sh --candidate /tmp/p11-03-baseline-final-3.json --compare` | PASS — fresh 30-sample measurements meet the committed baseline's P99, throughput, RSS, and 5 ms local HTTP limits. |
| Comparator positive/negative fixture suite | PASS — malformed identity plus P99, RSS, and throughput regressions fail closed. |
| `cargo fmt --all -- --check`, workspace Clippy with `-D warnings`, workspace tests | PASS. |
| `./scripts/check.sh full` | PASS — shell/CI/plan checks, comparator test, format, Clippy, workspace tests, source/crate boundaries, docs, Secret scan, dependency policy, and RustSec audit. |
| Focused review | PASS — the two benchmarks use no network/listener/account/config input; the HTTP case goes through the real Actix public route; compiler RSS is excluded before measurement; only a reviewed source edit can replace `baseline.json`; no production path or P11-04 scope was claimed. |
