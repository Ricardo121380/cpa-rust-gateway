# P12-08E2 Claude and Anthropic-compatible runtime

## Result

P12-08E2 is `LOCAL_PASS_PENDING_PHASE_GATE`. The production Anthropic Messages adapter now accepts
either a strict API key or an unexpired Claude OAuth credential, emits exactly one authentication
presentation, and classifies bounded Anthropic error envelopes into the existing Credential,
Quota, and Endpoint Health owners. Native Messages and the already-admitted Chat/Responses bridges
continue through the same bounded JSON/SSE runtime. No server, real credential, Config Version,
listener, or production traffic changed.

## Reference and porting boundary

The pinned behavioral reference is CLIProxyAPI v7.2.101 commit
`42a00a2a6521b867c27f7ad096d08699db8e6d19`:

- `internal/runtime/executor/claude_executor.go` and `claude_executor_request.go` establish the
  mutually-exclusive `x-api-key` versus OAuth Bearer presentation and the Messages request path;
- `internal/runtime/executor/claude_executor_auth.go` establishes replacement of access token,
  refresh token, expiry, and account metadata after refresh; and
- `internal/auth/claude/anthropic_auth.go` establishes the fixed token URL, public client identity,
  JSON refresh grant, required rotated refresh token, and expiry calculation.

CPAR deliberately does not port legacy device-identity fabrication, hidden request rewriting, raw
upstream error propagation, or foreground network refresh. DNS-pinned egress, bounded buffers,
strict typed conversion, value-free errors, and explicit state ownership remain authoritative.

## Credential and refresh boundary

An encrypted lease may contain either a non-empty visible-ASCII API key or strict tagged JSON:

```json
{"kind":"claude_oauth","access_token":"...","refresh_token":"...","expires_at_ms":1,"account_id":"..."}
```

OAuth JSON rejects duplicate and unknown names. API keys produce only `x-api-key`; unexpired OAuth
produces only `Authorization: Bearer`. Empty, malformed, or expired material fails before egress,
and every Debug implementation redacts tokens and account identity.

The refresh transaction fixes `https://api.anthropic.com/v1/oauth/token`, the pinned Claude client
identity, and the `refresh_token` JSON grant. A response must provide non-empty rotated access and
refresh tokens plus a positive expiry before the in-memory credential is replaced atomically. The
encrypted binding's `account_id` remains immutable: optional account/organization response data is
accepted without being allowed to silently rebind the Credential. The
ordinary request path performs no hidden refresh call: P12-08F1 must explicitly compose a
DNS-pinned worker and durable encrypted replacement before expired OAuth can recover automatically.

## Failure and state ownership

At most 64 KiB of a non-2xx body is inspected; oversized, malformed, or prose-only data cannot
invent retained state. The body is discarded after classification.

| Evidence | Gateway result | Sole state owner |
|---|---|---|
| `401` or `authentication_error` | `CredentialUnauthorized/Credential` | exact Endpoint/Credential reauthorization state |
| `429` or `rate_limit_error` | `ProviderRateLimited/QuotaWindow` | exact Endpoint/Credential quota window |
| `408`, `5xx`, or `overloaded_error` including `529` | `ProviderTransient/Provider` | selected Endpoint cooldown |
| unknown `403` | `EgressRejected/Egress` | no Credential/quota mutation |
| other 4xx or malformed signal | `ProviderPermanent/Provider` | no retained state |

A real loopback HTTP peer proves that `429` plus integer `Retry-After: 13` becomes an exact 13-second
quota failure. Existing orchestrator tests prove that a credential failure does not block a healthy
sibling, a rate limit affects only the selected binding, and a transient server failure cools only
the selected Endpoint before fallback.

## Protocol vertical slice

- Native Messages revalidates its strict payload and replaces only the selected upstream model.
- Chat and Responses requests use the D1-D3 typed bridge; Tool history and the pinned
  Reasoning-to-Thinking budget mapping are admitted only where lossless.
- Non-streaming and SSE use the production Anthropic decoders, including Tool lifecycle, Thinking
  progress, Usage timing/projection, terminal enforcement, chunk-independent arguments, frame/body
  limits, and semantic-progress deadlines.
- The provider builder test proves OAuth changes only the authentication header; the request body,
  target, version, media type, and API-key behavior remain unchanged.

## Verification and review

The following local evidence passed before the phase-wide gate:

- `cargo test --locked --offline -p provider-anthropic-compatible` — 14 passed;
- `cargo test --locked --offline -p gateway runtime::tests --no-fail-fast` — 64 passed, including
  Anthropic JSON/SSE, Tool, Thinking progress, Usage, bounded failures, and the new loopback 429;
- `cargo test --locked --offline -p gateway-router protocol_transform::tests --no-fail-fast` — 18
  passed for all nine native/Canonical/bridge request pairs and Tool/Thinking mapping;
- the three exact Router isolation regressions for credential unauthorized, quota, and Endpoint
  cooldown — each passed; and
- affected-package Clippy passed with warnings denied.

Review found and fixed two implementation issues before closeout: the first request type could
carry only `x-api-key`, and the first bounded classifier was OpenAI-specific. The final code uses
one opaque mutually-exclusive authorization type and one shared bounded body reader with separate
provider classifiers. The required Full gate is recorded at commit closeout.

## Remaining boundary

P12-08E3 next connects the already-developed Grok Build, Official, and Web providers to the unified
runtime. P12-08F1 still owns production graph composition and any Claude OAuth refresh worker;
P12-08G1 owns separately controlled real-credential receipts. This local slice does not claim live
Claude account availability or production readiness.
