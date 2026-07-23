# P8-06 Grok Official local differential, load, and error matrix report

| Field | Value |
|---|---|
| Plan version | `v1.38` |
| Task | `P8-06` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P7-G7-001` |
| Scope / budget | `M`; deterministic local fixture and state-matrix evidence only; no xAI request/load, account, server, route, or persisted-state change |
| References | Matrix `C01`、`C03`、`C04`、`C31`、`C33`、`F07`、`G24-G27`; [ADR-0059](../adr/ADR-0059-grok-official-local-differential-and-error-matrix.md); [BC-PROVIDER-017](../contracts/BC-PROVIDER-017-grok-official-local-differential-and-error-matrix.md) |

## Delivered evidence

One bounded completed JSON fixture and its SSE lifecycle now run through the actual Official
adapter/decoder boundary with the same final Tool call, Reasoning, assistant text, and
reasoning-token projection. SSE chunking at 1, 2, 9, 31, and 257 bytes changes only framing, not
the final semantic result.

The local concurrency test releases 12 OS threads at one barrier; each executes eight independent
in-memory SSE decoders with rotating legal chunk sizes (96 decoders in total). Every instance
completes with the same projection. This demonstrates decoder-state isolation under real local
thread concurrency; it is deliberately not a throughput, capacity, latency, or live-provider load
claim.

The failure matrix covers 401, unknown 403, 408, 500, permanent failure, and both 429 forms. Only
429 creates state, and it writes only its exact Official Endpoint/Credential target. Missing retry
metadata records the existing bounded `Estimated/Estimated` fallback; positive `Retry-After`
replaces it as `Header/Observed`. The Build comparison target stays absent throughout.

No xAI endpoint, API key, OAuth source/cache, server process, account, route, proxy/TUN rule, or
production configuration was read or changed. No Official E2E or external load was sent. P7/G7
remain blocked on Kiro account reauthentication, so this closes only P8's local task sequence—not
G8, P8 phase closeout, remote Delivery Gate, merge, release, or `DONE`.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p8_06_official_differential` | PASS; 3 tests cover adapter JSON/SSE semantic differential, 96 local decoders across 12 barrier-synchronized OS threads, and exact status/quota matrix. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --test p8_06_official_differential -- -D warnings` | PASS. |
| Full local gate | PASS: [`./scripts/check.sh full`](p8-06-local-full-check.md) covered workspace tests, source/crate boundaries, document links, Secret checks, dependency policy, and RustSec audit. |
| Focused code review | PASS: the changed test has no live transport, uses a synthetic key and in-memory body only, keeps each decoder local to one worker, synchronizes actual OS-thread start with a barrier, and preserves exact-target-only failure assertions. No synthetic result is represented as xAI proof. |

## Deferred phase acceptance

P8 local implementation is now complete, but `CR-P7-G7-001` keeps G8, phase closeout, the P8
Delivery Gate, merge, release, real Official E2E, and `DONE` blocked. When Kiro OAuth is repaired,
complete P7-09/G7/P7 Delivery first; then acquire explicit Official E2E authorization before G8.
