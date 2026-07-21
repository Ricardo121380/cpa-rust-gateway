# P5-01 Anthropic Messages adapter report

| Field | Value |
|---|---|
| Plan | `v1.9` |
| Task | `P5-01` |
| Date | `2026-07-22` |
| Branch | `codex/p5-anthropic` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope / budget | `M`; pure codec, fixtures, and documentation only |
| References | Matrix `A07`, `B04`, `B09`, `B11`, `B13`, `B25`, `B27`; [ADR-0034](../adr/ADR-0034-anthropic-messages-pure-codec.md); [BC-PROTOCOL-002](../contracts/BC-PROTOCOL-002-anthropic-messages-adapter.md) |

## Task Card and scope

Implemented `protocol-anthropic` as a protocol-pure request/response/SSE codec. It decodes
Anthropic Messages JSON into canonical requests, preserves unimplemented fields under explicit raw
namespaces, rejects ambiguous JSON, emits a text/Usage Messages response or typed SSE sequence,
and maps safe core errors.

Excluded from this Task: Actix routes, authentication, model routing, Provider transport,
credentials, token counting, outbound Tool delta state, Thinking, cache/stop semantics, real
Provider traffic, and Claude Code E2E. Those boundaries remain assigned to P5-02 through P5-07.

## Invariants and review changes

- Recursive duplicate JSON-name rejection happens before `serde_json::Value` semantic decoding.
- Known `tool_use`/`tool_result` blocks in an invalid role are rejected rather than downgraded to
  opaque content.
- A split historical Tool Result retains its message-level unknown fields exactly once.
- The output codec requires exact canonical input/output Usage and does not emit a guessed token
  value. Unsupported Tool/Thinking/cache/extension output fails closed for its later Task owner.
- The codec contains no HTTP or Provider dependency and cannot mark a client write as delivered.
- Its only new direct third-party dependencies are `serde` and `serde_json`, both recorded in the
  crate-boundary allowlist; the Full Gate caught and verified that boundary update.

## Verification

| Command | Result |
|---|---|
| `cargo fmt --all` | PASS |
| `cargo test --locked -p protocol-anthropic` | PASS; 9 unit tests plus doc tests |
| `cargo clippy --locked -p protocol-anthropic --all-targets --all-features -- -D warnings` | PASS |
| `git diff --check` | PASS; the SSE snapshot keeps each frame's final separator asserted in code without a whitespace-only EOF line |
| `./scripts/check.sh full` | PASS after synchronizing the explicit `serde`/`serde_json` protocol-crate boundary; complete workspace format, Clippy, tests, source policy, crate boundaries, docs, secret scan, dependency policy, and RustSec audit passed |

The model handoff did not preserve a reliable monotonic start timestamp, so this report does not
claim a false Task wall-clock duration. It records the bounded scope and command evidence; the P5
closeout will aggregate measured verification timing for the complete Phase.

## Review conclusion

The adapter preserves the protocol/core/transport boundary and does not silently turn incomplete
or unsupported semantics into text. It is safe to use as the P5-02 and P5-03 Phase-local
dependency, but it is not release acceptance until the one G5 Phase tag Gate passes.

## Rollback and next Task

Reverting this Task removes only a pure codec and test fixtures; it has no migration, credential,
network, or Provider-traffic consequence. P5-02 may now add the explicitly accurate
`count_tokens` capability and reject unsupported route capability without estimating a count.
