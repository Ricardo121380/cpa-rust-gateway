# P12-08F2 three-protocol by four-channel loopback matrix

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

Date: 2026-08-02

Plan: `v1.108`

## Result

The local Actix data-plane harness now executes the three public protocols against the four
production channel classes using the same F1 capability ledger and D3 request projector. It runs
without an external network, real Credential, server mutation, Config Version publication, or
production traffic.

The test does not force a false twelve-cell success matrix. Seven combinations are locally
supported; five are rejected before the synthetic upstream Attempt counter advances.

| Client protocol | OpenAI/Codex | Claude | Grok | Kiro |
|---|---|---|---|---|
| Chat Completions | `SUPPORTED` via Chat adapter | `UNSUPPORTED`: Reasoning-capable response cannot be represented safely | `UNSUPPORTED`: Reasoning-capable response cannot be represented safely | `UNSUPPORTED`: Reasoning-capable response cannot be represented safely |
| Responses | `SUPPORTED` via Responses adapter | `SUPPORTED` lossless bridge | `SUPPORTED` native Canonical | `UNSUPPORTED`: Provider runtime is Canonical-only for Messages |
| Messages | `SUPPORTED` lossless bridge | `SUPPORTED` native Canonical | `UNSUPPORTED`: Provider runtime is Canonical-only for Responses | `SUPPORTED` native Canonical |

## Executed evidence

For every `SUPPORTED` cell the harness executes four authenticated requests:

- non-streaming JSON Text plus final Usage;
- streaming SSE Text plus final Usage;
- non-streaming JSON Tool plus final Usage; and
- streaming SSE Tool plus final Usage.

That is 7 cells × 4 requests = 28 successful requests. Each request passes strict ingress decode,
the F1 adapter capability profile, D3 registered source/target projection, a single counted
synthetic Attempt, Canonical lifecycle validation, and the real client-protocol response encoder.

For every `UNSUPPORTED` cell the harness executes JSON and SSE Tool requests. All 5 cells × 2
requests return the protocol's stable safe upstream-protocol error and retain Attempt count zero.
Anthropic's public error envelope intentionally does not expose the internal error-code token, so
the assertion uses the stable safe message rather than requiring identical cross-protocol JSON.

## Review correction to F1

The first Tool request exposed that the Route predicate treats a Tool declaration as requiring
JSON Schema capability because every Tool has an input schema. F1 initially recorded Tools without
JSON Schema and therefore rejected all Tool traffic before an Attempt. E1-E4 already prove the
typed Tool-schema builders for every admitted adapter, so the ledger now records JSON Schema with
Tools. Vision remains unsupported. The F1 exact-matrix tests were updated to prevent regression.

## Verification

- `cargo test --locked --offline -p gateway runtime::tests::three_protocols_by_four_channels_obey_the_f2_loopback_matrix --no-fail-fast`
- `cargo test --locked --offline -p gateway runtime::tests --no-fail-fast`
- task Full gate, Clippy, document links, tracked Secret scan, dependency policy and RustSec audit

The existing E1-E4 tests continue to own concrete Provider request builders, decoders, credentials,
failure ownership and mock transports. F2 closes the public-protocol-to-channel routing seam; it
does not claim real account availability. G1 remains the controlled live-credential boundary.

## Remaining boundary

P12-08F3 next performs the value-free Client Key, Alias and client migration dry-run for OpenClaw,
CC Switch and the retained legacy key identities. It must not change the production hostname.
