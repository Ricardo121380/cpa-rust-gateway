# P1-03 Canonical event state machine report

| Field | Value |
|---|---|
| Plan | `v1.0` |
| Task | `P1-03` |
| Date | `2026-07-18` |
| Branch | `codex/p1-03-canonical-event` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Added the 11-event protocol-neutral `CanonicalEvent` vocabulary with explicit payloads for
  response, message, text, reasoning, Tool, Usage, normal completion, and safe stream failure.
- Added a synchronous `CanonicalEventState` that validates ordering, correlation, terminality, and
  successful versus error completion without HTTP, Actix, SSE, async streams, Provider code,
  routing, credentials, cancellation, or buffering.
- Added `CanonicalResponse` for a caller-owned finite successful sequence. It validates before
  retaining events; a terminal `StreamError` returns only its safe `GatewayError`.
- Kept Tool arguments raw and complete at `ToolCallEnd`. The core accepts interleaved Tool argument
  fragments but leaves incremental JSON assembly and empty-argument normalization to later
  protocol work.
- Added JSON round-trip coverage for response IDs and safe errors. Error code JSON retains the
  established PascalCase encoding and error scope JSON retains the established snake_case encoding.
- Added a desensitized fixture covering all non-error event types, actual interleaved Tool
  arguments, an empty-argument Tool, Usage, and raw extensions. `Debug` diagnostics redact
  supplied IDs, text, Tool names/arguments, and raw JSON.

## Compatibility and error decisions

- An explicit `MessageEnd` while a Tool remains open is an event-order violation and returns
  `UpstreamProtocolError` with `Stream` scope. Only source EOF or normal `ResponseEnd` with
  unfinished work returns `StreamTruncated` with `Stream` scope.
- A response may normally end after no Usage, interim Usage, or final Usage. An explicitly final
  Usage update remains accepted at most once; this preserves the frozen behavior contract and
  avoids treating providers that omit a final snapshot as truncated.
- `ToolCallEnd` is the atomic core transition from already-complete arguments to emitted Tool
  output. Its `RawJson` value proves complete valid JSON without core-side normalization.

## Verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-core` | PASS; 38 unit tests plus doc tests |
| `cargo clippy --locked -p gateway-core --all-targets --all-features -- -D warnings` | PASS |
| `./scripts/check.sh fast` | PASS |
| `./scripts/check.sh full` | PASS; dependency policy and RustSec audit pass |
| `git diff --check` | PASS |

## Review

- Two independent read-only reviews found and verified fixes for the incorrect open-Tool
  `MessageEnd` error category, incompatible interim-Usage finality requirement, scope JSON
  encoding, fixture coverage, atomic Tool-end wording, and traceability overclaim.
- The final reviews passed. Scope remains limited to `gateway-core`, its core contract, a
  desensitized fixture, and task documentation; no P1-04 bounded-stream/backpressure/cancellation
  code or P1-05 HTTP/SSE/Provider adapter code was introduced.

## Limits and next task

P1-03 does not implement bounded delivery, cancellation, first-semantic-event tracking, HTTP/SSE
encoding, incremental Tool JSON assembly, empty-argument normalization, Provider execution,
authentication, routing, or persistence. `P1-04` remains `PENDING` and is the next task only after
it is explicitly marked as the sole `IN_PROGRESS` task.
