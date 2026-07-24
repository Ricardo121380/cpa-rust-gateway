# P5-04 Protocol transform admission report

| Field | Value |
|---|---|
| Plan version | `v1.9` |
| Task | `P5-04` |
| Date | `2026-07-22` |
| Branch | `codex/p5-anthropic` |
| Status | `DONE` |
| Scope / budget | `S`; pure Router admission analyzer, unit matrix, and documentation only |
| References | Matrix `A07`, `B24-B28`, `F01-F04`, `L08`, `L21-L22`, `L40`; [ADR-0037](../adr/ADR-0037-protocol-transform-admission.md); [BC-ROUTER-004](../contracts/BC-ROUTER-004-protocol-transform-admission.md) |

## Delivered boundary

`gateway-router` now exposes a secret-safe `analyze_protocol_transform` seam. It distinguishes
`OpenAiResponses` and `AnthropicMessages`, checks the configured Snapshot transform mode, and
returns only `Approved` or a stable value-only rejection code. It has no side effects and does not
make a request to an Endpoint or Provider.

Exact native same-protocol pass-through can retain opaque native extensions because it forwards the
original body unchanged. Every reconstructed path is stricter: same-protocol Canonical and
cross-protocol Lossless Bridge reject unknown extensions, opaque blocks, historical Tool input,
Thinking, cache controls, and unsupported roles. Streaming, Tools, JSON Schema, and parallel Tools
must be present in the selected Endpoint capability set before admission.

## Review

The local review confirmed that the analyzer contains no request serializer, endpoint lookup,
credential/secret access, health/circuit mutation, event write, or runtime network call. The
initial implementation review also caught strict workspace lint requirements in the tests; the
final test helpers propagate construction errors instead of using `unwrap`/`panic`, and the
admission input's custom debug form redacts the entire request.

## Verification

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p gateway-router` | PASS; Router unit suite includes topology, lossless field/Tool/Reasoning matrix, capability matrix, and redaction coverage |
| `cargo clippy --locked -p gateway-router --all-targets --all-features -- -D warnings` | PASS |
| `git diff --check` | PASS |
| `CHECK_REPORT_PATH=tmp/p5-04-fast-check.md ./scripts/check.sh fast` | PASS; recorded after documentation/index updates |

## Known limits, rollback, and next Task

This Task admits or excludes a Candidate; it does not yet connect the outcome to Endpoint-aware
HTTP execution, health, circuit state, or event records. It also does not encode Thinking/cache
semantics or perform Claude Code client E2E. Reverting the Task removes only the Router analyzer,
its tests, and its documents; it has no migration, credential, or Provider-traffic consequence.

P5-05 is now the sole `IN_PROGRESS` Task. It consumes this admission contract while introducing
same-Upstream Responses/Anthropic Endpoint isolation. P5 still has one final Phase-level remote
Delivery Gate after G5.
