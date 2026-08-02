# P12-08A OpenAI Chat Completions codec report

| Field | Value |
|---|---|
| Plan | `v1.94` |
| Task | `P12-08A` |
| Date | `2026-08-02` |
| Branch | `codex/p12-deployment` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope | Strict pure Chat request/response/SSE codec only |
| Contract | [BC-PROTOCOL-008](../contracts/BC-PROTOCOL-008-openai-chat-completions-codec.md) |

## Outcome

Added the workspace crate `protocol-openai-chat`. It admits the common Chat Completions text,
ordered history, function Tool, Tool-result, non-streaming, streaming, Usage, stop, and error shapes
into or out of Canonical without adding HTTP, routing, Provider, or credential dependencies.

This closes only P12-08A. CPAR does not yet expose `/v1/chat/completions`, and this result does not
claim that every public protocol can already use every Kiro, Grok, Codex, or Claude channel.
P12-08B-G and the P12 Phase gate remain required before production replacement.

## Review corrections

- Replaced silent saturating/default-zero Usage projection with required input/output counts,
  checked addition, and fail-closed rejection of incompatible cache count semantics.
- Deferred the optional usage-only SSE frame until after the finish chunk and immediately before
  `[DONE]`, matching the common Chat stream lifecycle.
- Omitted absent `tool_calls` instead of serializing a nonstandard null member.
- Added all UTF-8 split-point coverage for Tool argument deltas and kept reasoning fail-closed.
- Registered the new crate in the explicit dependency-boundary policy.

## Verification

| Command | Result |
|---|---|
| `cargo test --locked -p protocol-openai-chat` | PASS; 7 unit tests plus doc tests |
| `cargo clippy --locked -p protocol-openai-chat --all-targets -- -D warnings` | PASS |
| `./scripts/check.sh docs` | PASS |
| `./scripts/check.sh fast` | PASS |
| `git diff --check` | PASS |

## Next boundary

P12-08B may add the authenticated, bounded Actix `/v1/chat/completions` endpoint and verify JSON/SSE
HTTP behavior, keepalive, cancellation, and First Semantic Event commit. It must reuse the existing
Responses/Messages security and delivery contracts rather than bypassing them.
