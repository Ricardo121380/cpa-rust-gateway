# P11-04 Load and Soak report

| Field | Value |
|---|---|
| Plan version | `v1.45` |
| Task | `P11-04` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P11-04-001`. |
| Branch | `codex/p11-release-hardening` |
| Scope | Deterministic loopback concurrency, bounded long streams, connection reuse, cancellation, RSS observation, and a user-accepted ≥10-hour local soak. |
| External state | None. The workload uses only process-local state and ephemeral `127.0.0.1` peers. |
| Final receipt | `docs/reports/evidence/p11-04-soak-final-rss-window-20260724.log` — a truthful user-stopped `INCOMPLETE` receipt after 36,818 seconds, accepted only by `CR-P11-04-001`. |

## Fixed workload boundary

The test harness has no public DNS, Provider endpoint, account, credential, proxy, browser, or
configured-upstream input. Its only HTTP peer binds an ephemeral loopback TCP port. The Egress
Policy admits only `http`, `p11-04-loopback.test`, that peer's ephemeral port, and `127.0.0.1/32`;
redirects are denied. The workload source, request body, and receipt contain no secret or response
value.

The opt-in runner accepts only `--smoke` or `--soak`, chooses a new absolute receipt path, and
uses fixed durations (10 seconds or 24 hours), four concurrent streams per batch, and a two-second
inter-batch pause. `INT`, `TERM`, and `HUP` produce a terminal `INCOMPLETE` runner state, so an
interrupted foreground process cannot look successful.

## Completed deterministic evidence

| Acceptance area | Evidence | Result |
|---|---|---|
| Concurrent finite long streams and slow consumer | 12 tasks each deliver `ResponseStart`, `MessageStart`, 128 text deltas, `MessageEnd`, and `ResponseEnd`; every spawned task is joined. | PASS |
| Cancellation / failure join boundary | 12 capacity-blocked producers are cancelled; each observes the `Cancelled` code and all 12 JoinSet entries are collected. A workload failure also drains the whole JoinSet before returning the first failure. | PASS |
| Keep-alive reuse | 24 requests go through the real `UpstreamClientPool` to the loopback peer; the peer observes fewer connections than requests after warm-up. | PASS |
| Smoke receipt | Post-review smoke completed 5 batches / 20 streams in 10 seconds; RSS was 8,044,544 B initially and 8,241,152 B on completion. | PASS |
| RSS detector | After two warm-up samples, every six-sample (25-minute) window fails closed when monotonic growth exceeds 15%; an early-window regression test prevents checking only the final window. | PASS |
| Runner boundary | Malformed mode/path inputs are rejected; a controlled `TERM` smoke interruption writes `runner_state=INCOMPLETE` and exits nonzero. | PASS |
| Receipt checker | Exact-field, timing, terminal-state, batch/stream, and full-history RSS checks accept a completed fixture and reject RSS-growth, incomplete, and malformed fixtures. The final shortened receipt is intentionally reviewed against the CR criteria rather than passed through its 24-hour checker. | PASS |
| Focused code checks | `cargo fmt --all -- --check`; `cargo test --locked -p gateway-http-actix --test p11_04_load_soak`; focused Clippy; `git diff --check`. | PASS |
| [Local Full gate](p11-04-local-full-check.md) | The first run exposed a pre-existing P10-08 temporary-directory collision under parallel test execution. The test now adds an atomic per-process suffix and retries a name collision; 10 focused repetitions and the rerun `./scripts/check.sh full` passed. | PASS |
| Documentation boundary | `./scripts/check.sh docs` checked 306 Markdown files, plan state (115 Tasks / one `IN_PROGRESS`), tracked Secret scan, and whitespace. | PASS |

## CR-P11-04-001 shortened local-soak acceptance

The user approved treating the long-running local test as complete rather than waiting for its
remaining 14 hours. This changes the local threshold only; it does not claim that the 24-hour run
completed or remove P12-10's real 72-hour Canary observation.

| Receipt fact | Observed value | Result |
|---|---:|---|
| Elapsed time | 36,818 s (10 h 13 m 38 s) | Meets the ≥10-hour CR threshold |
| Status samples | 123 | Well-formed and monotonic in timestamp/elapsed time |
| Final batch / stream count | 18,179 / 72,716 | Exact four-stream batch ratio |
| RSS | 8,110,080 B → 6,832,128 B | No sustained >15% monotonic-growth window (zero observed) |
| Terminal state | `runner_state=INCOMPLETE`, `exit_status=130`, `interruption_signal=INT` | Truthfully records the user-requested stop; not treated as a 24-hour completion |

The focused tests, runner/receipt regressions, documentation checks, review, and [local Full gate](p11-04-local-full-check.md) all pass. P11-04 is therefore `LOCAL_PASS_PENDING_PHASE_GATE`; P11-05 may begin as the sole next `IN_PROGRESS` Task.

## Focused review

PASS. The workload admits only an ephemeral loopback peer, has no credential or ambient proxy
input, and keeps its request/response values out of the receipt. Concurrent success, cancellation,
JoinSet draining, backpressure, keep-alive reuse, signal receipt, status-field exactness, and every
post-warm-up RSS window have direct regression coverage. `CR-P11-04-001` is narrow and honest: it
accepts the observed 10h13m local evidence while retaining the terminal `INCOMPLETE` fact and
preserving P12-10's real 72-hour Canary requirement.
