# P12-08D2 three-protocol response and SSE projection

| Field | Value |
|---|---|
| Plan | `v1.100` |
| Task | `P12-08D2` |
| Date | `2026-08-02` |
| Branch | `codex/p12-deployment` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Legacy reference | CLIProxyAPI `v7.2.101`, commit `42a00a2a6521b867c27f7ad096d08699db8e6d19` |
| Network or deployment change | None |

## Outcome

The protocol layer now decodes complete JSON and arbitrarily chunked SSE from Chat Completions,
Responses and Messages into the shared Canonical lifecycle, then projects that lifecycle into each
of the three client response protocols. All nine decoded source/target pairs reach both the real
non-streaming encoder and the real stateful SSE encoder in tests.

The new projection boundary is incremental and transactional: a rejected event commits neither
its upstream lifecycle transition nor its target lifecycle transition. It preserves Tool argument
correlation, aggregate Usage, supported stop semantics, explicit stream failure and terminal order.
It never turns Reasoning into visible Chat text.

This slice does not register or publish any new runtime pair. The existing private runtime
Responses decoder intentionally remains in place until P12-08D3 performs one reviewed integration;
there is no server, credential, Config Version, route, listener or traffic change in D2.

## Implemented boundary

- `protocol-openai-responses` owns a transport-free, bounded upstream decoder for complete JSON and
  typed Responses SSE. Buffered and streamed fixtures produce the same final semantic projection.
- SSE accepts arbitrary byte boundaries, including one-byte chunks; it requires an explicit unique
  terminal event and never synthesizes success at EOF.
- Response bodies, individual SSE residue, accumulated Tool arguments, output items, Tool calls,
  identifiers and progress-free frames all have fixed limits. Duplicate JSON names, unknown event
  types, late semantic bytes, incomplete Tool calls and oversized residue fail closed.
- Chat and Responses retain input/output totals while dropping only cache details their public
  encoder does not represent. Messages retains input/output and cache creation/read details while
  dropping OpenAI-only reasoning/cached counters.
- Successful stop reasons are normalized only through a finite target-specific table. Chat maps
  refusal/content filtering to `content_filter`; Responses emits typed incomplete termination for
  max-output and refusal/content-filter outcomes.
- `ProtocolResponseProjector` validates source and target lifecycle state transactionally and
  passes a Canonical `StreamError` through as the sole failed terminal.
- Debug output reports only bounded shape and lifecycle metadata, never text, reasoning, Tool
  arguments, identifiers or raw extension values.

## Behavior classification

| Classification | Result |
|---|---|
| `PARITY` | Three source decoders to three target encoders; JSON/SSE final semantic equivalence; ordered Text, Tool, Usage and terminal events; explicit stop normalization |
| `INTENTIONAL_HARDENING` | Strict duplicate-name and unknown-event rejection; fixed memory/count/progress bounds; explicit terminal required; transactional lifecycle; late bytes after terminal reject; value-free diagnostics |
| `UNSUPPORTED_FAIL_CLOSED` | Reasoning to Chat, opaque event/Usage extensions, unknown stop reasons, non-Messages stop sequences, encrypted or otherwise unrepresentable Reasoning content, and upstream shapes without a proven Canonical meaning |

No observed D2 difference remains unclassified.

## Verification

| Command or evidence | Result |
|---|---|
| `cargo test --locked -p protocol-openai-chat -p protocol-openai-responses -p protocol-anthropic -p gateway-router` | PASS; Router 92, Anthropic 45, Chat 11, Responses 22 executed tests; one existing opt-in random property test ignored |
| All nine decoded response source/target pairs through real JSON and SSE encoders | PASS |
| Responses buffered JSON versus arbitrarily chunked SSE final projection | PASS |
| Stateful projection equivalence, transactional rejection and failed terminal | PASS |
| Missing/duplicate/unknown/oversized/late terminal-path negative tests | PASS |
| `cargo clippy --locked -p protocol-openai-chat -p protocol-openai-responses -p protocol-anthropic -p gateway-router --all-targets -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `./scripts/check.sh docs` | PASS |
| `git diff --check` | PASS |

## Review conclusion and remaining boundary

- The protocol codecs and projector are pure: they own no HTTP client, DNS, credential, route,
  retry, clock, database or publication state.
- The fixtures are synthetic and contain no account, endpoint, credential, production body or log.
- Chat cannot represent Reasoning without semantic loss. D3 must leave any potentially
  Reasoning-producing source-to-Chat pair ineligible unless its selected capability contract proves
  that Reasoning cannot occur; an already-started stream must never silently expose or discard it.
- D3 must replace the runtime-private Responses decoder with this protocol-owned decoder, connect
  the incremental projector and target encoder, and publish only registry pairs whose request and
  response capability proofs both pass.
- P12-08D4 still owns the offline legacy differential corpus. P12-09 production cutover remains
  pending.
