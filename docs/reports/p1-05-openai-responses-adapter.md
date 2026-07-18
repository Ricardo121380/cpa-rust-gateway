# P1-05 OpenAI Responses adapter report

| Field | Value |
|---|---|
| Plan | `v1.0` |
| Task | `P1-05` |
| Date | `2026-07-18` |
| Branch | `codex/p1-05-openai-responses-adapter` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Added a pure `protocol-openai-responses` JSON/SSE codec with no Actix, HTTP writer, Provider,
  routing, retry, or `gateway-stream` dependency.
- Decodes the supported Responses request subset into `CanonicalRequest`: model, stream mode,
  text/history messages, Function tools/calls/results, reasoning effort, cache fields, and
  namespaced raw options. Unsupported execution controls are rejected explicitly.
- Rejects duplicate JSON member names before semantic decoding, including an embedded historical
  Function Call `arguments` JSON string.
- Encodes validated canonical responses as non-streaming Responses JSON or deterministic typed SSE
  events with monotonic sequence numbers, normal `response.completed`, and terminal safe
  `response.failed` rather than `[DONE]`.
- Keeps the FirstSemanticEvent commit boundary with P1-07: this codec classifies semantic frames
  but cannot mark delivery before a successful HTTP write.
- Rejects generic event/Usage raw extensions and usage data that Responses cannot represent
  losslessly; no cache-detail collapse or saturated token totals are emitted.

## Verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p protocol-openai-responses` | PASS; 12 unit tests plus doc tests |
| `cargo clippy --locked -p protocol-openai-responses --all-targets --all-features -- -D warnings` | PASS |
| `./scripts/check.sh fast` | PASS |
| `./scripts/check.sh full` | PASS; dependency policy and RustSec audit pass |
| `git diff --check` | PASS |

## Review

- The first independent protocol review found four boundary defects: silently discarded nested
  Usage extensions, duplicate-key bypass through Function Call argument strings, non-lossless
  usage cache details/overflow, and ambiguity about raw request-option retention.
- The final independent review passed after nested duplicate validation, lossless Usage rejection,
  checked token addition, and an explicit contract/test that unimplemented but losslessly retained
  request options are not P1 execution guarantees.
- A final review also confirmed non-assistant canonical output is rejected before lifecycle commit,
  terminal failures cannot emit completion, and all P1-05 frames remain pure protocol data rather
  than FirstSemanticEvent delivery commits.

## Limits and next task

This task intentionally stops at a codec boundary. It provides no Provider execution, bounded
transport integration, HTTP headers/status selection, authentication, or public endpoint. P1-06
remains `PENDING`; it may begin only as the plan's sole `IN_PROGRESS` task on a new task branch.
