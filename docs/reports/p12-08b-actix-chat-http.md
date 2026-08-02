# P12-08B Actix Chat Completions HTTP report

| Field | Value |
|---|---|
| Plan | `v1.95` |
| Task | `P12-08B` |
| Date | `2026-08-02` |
| Branch | `codex/p12-deployment` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Contract | [BC-HTTP-002](../contracts/BC-HTTP-002-actix-chat-completions-boundary.md) |

## Outcome

The public Actix data plane now exposes an authenticated `POST /v1/chat/completions` handler. It
uses the P12-08A strict codec and the existing Snapshot model resolution, router executor, bounded
Canonical transport, Usage observer, SSE keepalive, cancellation, and First Semantic Event body
handoff rather than creating a parallel execution path.

Request observations now distinguish `openai_chat_completions` from `openai_responses` and
`anthropic_messages`. No request/response body, presented key, upstream model, or endpoint is added
to the observation.

## Review conclusion

- Authentication precedes body buffering; unauthenticated oversized input still returns 401 and
  never starts the executor.
- The route-specific 4 MiB bound and raw-byte decode preserve duplicate-name rejection and return a
  safe 413 Chat/OpenAI envelope on overflow.
- Streaming success commits no header before `ResponseStart` is encodable, and normal output orders
  finish before optional Usage and exactly one `[DONE]`.
- Existing generic body tests continue to prove keepalive cannot commit FSE and body drop cancels
  the source. All Responses and Messages HTTP tests remain green.

P12-08B does not add an upstream Chat Endpoint or claim cross-protocol routing. The current runtime
can expose the HTTP surface for mock/canonical tests, but P12-08C-D must complete before it is a
usable production compatibility path.

## Verification

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-http-actix` | PASS; 42 unit tests, integration suites and doc tests |
| `cargo test --locked -p gateway-core -p gateway-observability -p gateway-store` | PASS |
| `cargo clippy --locked -p gateway-http-actix --all-targets -- -D warnings` | PASS |
| `./scripts/check.sh docs` | PASS |
| `./scripts/check.sh fast` | PASS |
| staged Secret scan / `git diff --check` | PASS |

## Next boundary

P12-08C adds `openai/chat-completions` as a third exact upstream `ApiFormat` and implements its
OpenAI-compatible outbound request/response adapter under DNS-pinned transport tests.
