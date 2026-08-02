# P12-08C OpenAI-compatible Chat Completions adapter report

| Field | Value |
|---|---|
| Plan | `v1.96` |
| Task | `P12-08C` |
| Date | `2026-08-02` |
| Branch | `codex/p12-deployment` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Contract | [BC-PROVIDER-023](../contracts/BC-PROVIDER-023-openai-compatible-chat-completions.md) |

## Outcome

The closed API Format vocabulary, publish-time compiler and P12 composition root now bind the exact
pair `openai/chat-completions` + `openai-compatible.chat-completions`. A Chat request carries its
strictly decoded native payload and trusted ingress protocol through the Router, so the native
adapter preserves existing Chat fields and changes only the upstream model.

The implementation was reviewed against CLIProxyAPI `v7.2.101`'s native OpenAI translator. CPAR
ports its payload-preservation intent and JSON/SSE response vocabulary, while retaining exact egress
admission, bounded response/frame buffers, checked Usage, Canonical lifecycle validation, unique
`[DONE]`, and safe diagnostics. This also removes the former inference of client protocol from an
Anthropic-only extension marker: protocol identity is now an explicit trusted execution field.

## Review conclusion

- Native body retention is request-scoped, redacted, and available only to the same-protocol Chat
  adapter; P12-08D bridges must use Canonical and explicit transform admission.
- The request builder re-runs strict decode and verifies response mode before egress. Foreign
  extensions and reserved-field collisions fail before transport.
- Non-streaming and streaming upstream decoders reject multiple choices, unsupported semantics,
  invalid finish reasons, Usage overflow and truncated streams.
- DNS-pinned tests prove only the exact admitted URL receives the request. Existing Responses and
  Messages routes remain green.
- No server, credential, production graph, listener, Caddy, DNS or public traffic changed.

## Verification

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-protocol -p gateway-router -p gateway-control -p protocol-openai-chat -p provider-openai-compatible -p gateway-http-actix` | PASS |
| `cargo test --locked -p gateway` | PASS; 68 unit tests and component smoke |
| `cargo clippy --locked -p protocol-openai-chat -p provider-openai-compatible -p gateway-router -p gateway-http-actix -p gateway --all-targets -- -D warnings` | PASS |
| `./scripts/check.sh docs` | PASS |
| `./scripts/check.sh fast` | PASS; full workspace tests, Clippy, serve envelope, boundaries and secret scan |

## Next boundary

P12-08D ports and verifies the three-protocol conversion matrix. It must use CPA `v7.2.101`
translator fixtures for Chat/Responses/Messages Text, Tool, history, Usage and termination behavior,
while rejecting every combination whose semantics CPAR cannot preserve.
