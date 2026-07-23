# ADR-0059: Grok Official local differential, load, and error matrix

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P8-06` |
| Matrix / Contract | `C01`、`C03`、`C04`、`C31`、`C33`、`F07`、`G24-G27`; [BC-PROVIDER-017](../contracts/BC-PROVIDER-017-grok-official-local-differential-and-error-matrix.md) |

## Context

P8-01 through P8-05 now implement the local Official API-key catalog, Responses codec,
rate-limit metadata, Tool/Reasoning capability, and exact state-isolation boundary. The remaining
task must prove these local surfaces compose without changing P7/G7's fail-closed boundary.
`CR-P7-G7-001` still prohibits a real Official request, P8 phase closeout, Delivery Gate, merge,
release, or `DONE` before Kiro reauthentication and the P7 Delivery Gate complete.

The only safe current differential is therefore deterministic and synthetic: equivalent completed
and SSE payloads through the actual adapter/decoder, arbitrary byte chunking, bounded concurrent
decoder instances, and a value-free status matrix. It is not an xAI behavior, rate, capacity, or
cost measurement.

## Decision

1. Compare one complete Official JSON Response with the same SSE lifecycle through the real
   `GrokOfficialInferenceAdapter` and decoder. The final Canonical semantic projection must agree
   on Tool call name/arguments, Reasoning, text, and final reasoning-token usage across fixed
   byte splits `1, 2, 9, 31, 257`.
2. Exercise 96 independently instantiated synthetic SSE decoders on one bounded fixture with
   rotating legal chunk sizes, executed as 12 OS threads with a start barrier and eight decoders
   per thread. This verifies state has no cross-instance leakage or chunk-dependent completion;
   it is explicitly not a benchmark or throughput claim.
3. Verify 401, unknown 403, 408, 500, permanent failure, 429 without retry evidence, and 429 with
   `Retry-After`. Only 429 writes an exact Official target. Its no-header case is
   `Estimated/Estimated`; a positive retry value is `Header/Observed`; another binding remains
   absent in every case.
4. Do not send a live request, create a load harness against xAI, infer a live API contract, or
   convert a synthetic pass into G8/P8 phase completion evidence.

## Consequences

- The codec's valid semantic subset is proven invariant across its two admitted response modes and
  tested chunk boundaries, including P8-04 Tool/Reasoning capability.
- Concurrent local parsing does not share output lifecycle or tool state; malformed state cannot be
  masked by a neighbouring decoder.
- Official failures remain target-local and source/confidence-labelled. Build/Web scheduling or
  state cannot be affected by the matrix.
- Real Official compatibility, capacity, latency, pricing, and Header behavior remain unknown until
  G7 passes and a separate explicit E2E authorization is recorded.

## Alternatives considered

- Real load testing against xAI: rejected by `CR-P7-G7-001` and because it would consume quota
  without proving the local correctness properties above.
- Treat synthetic fixture success as upstream differential proof: rejected; only a live, explicitly
  approved reference can prove current remote behavior.
- Test just one SSE chunk size: rejected because decoder framing must not depend on transport
  segmentation.

## Validation and rollback

Synthetic tests cover adapter JSON/SSE projection, five chunk shapes, 96 concurrent decoder
instances across twelve simultaneously released OS threads, and the complete bounded failure matrix.
Formatting, Clippy, full workspace tests,
source/crate boundaries, document links, Secret checks, dependency policy, and RustSec audit must
pass locally. Rollback removes P8-06's test/ADR/contract/report/index links only. It changes no
endpoint, API key, account, server, proxy/TUN rule, production traffic, external quota, or P7/G7
state.
