# ADR-0042: Grok Build OAuth credential and Device Code boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-22` |
| Task | `P6-01` |
| Contract | [BC-CRED-003](../contracts/BC-CRED-003-grok-build-oauth-device-code.md) |

## Context

P6 starts the first provider-specific implementation. Grok Build uses OAuth credentials that may
arrive as a user-supplied JSON export or through the Device Authorization Grant. Those inputs carry
access tokens, refresh tokens, device codes, and optional `id_token` values. They must not leak in
debug output, logs, error strings, test artifacts, or an unbounded local parser. Nor can an
unverified `id_token` claim become a gateway account identity.

The phase plan requires local Mock evidence first. Real Build traffic, credential persistence,
refresh concurrency, request assembly, quota, and cache policy are intentionally separate P6 Tasks.

## Decision

1. Introduce `provider-grok`'s pure `GrokBuildCredential` with zeroizing access/refresh material,
   redacted `Debug`, an explicit expiry instant, non-secret client/scope metadata, and an acquisition
   source. Its importer accepts one bounded strict JSON object; duplicate keys at any nesting level,
   empty/whitespace/control-character tokens, unsupported token types, unknown fields, and multiple
   expiry representations fail closed. Only an integral `expires_in` is accepted; `id_token` is
   syntactically bounded then discarded without claim parsing.
2. Model Device Authorization as an injected synchronous `GrokBuildOAuthTransport` seam. The public
   request object reveals only fixed endpoint/kind; its form payload is private and redacted, so an
   external mock cannot destructure device or refresh tokens. The fixed endpoints, public client id,
   and scope are constants; no P6-01 path opens a network connection.
3. Parse Device Authorization responses into a local poller that enforces `interval`, honors
   `authorization_pending`, raises its wait interval on `slow_down`, and becomes terminal on grant,
   denial, or expiry. Polling before the authorized instant is rejected locally.
4. Keep the transport's token refresh operation pure and single-shot. P6-02 owns per-Credential
   singleflight, revision/CAS conflict protection, encrypted persistence, and recovery behavior.

## Consequences

- OAuth input admits a narrow, testable, secret-safe representation before any provider request
  construction or account state mutation.
- Device Code UI values remain available only through explicit accessors, while all generic
  diagnostics redact them. The fixed verification URI can be used by a future local UI without
  placing a code into logs.
- Tests use only synthetic values and a scripted in-process transport. They do not read ambient
  credentials, contact `auth.x.ai`, alter a server, or validate a real account.
- This task adds direct workspace-locked `serde`, `serde_json`, `url`, and `zeroize` edges to
  `provider-grok`; the crate-boundary allow-list documents their bounded use.

## Alternatives considered

- Deserialize to `serde_json::Value`: rejected because normal object deserialization silently
  accepts duplicate keys and could overwrite an earlier credential field.
- Expose a public enum containing Device/refresh-token fields to mock transports: rejected because
  any external implementation could destructure and print those fields.
- Trust `id_token` claims for tenant or account attribution: rejected because P6-01 has no issuer,
  audience, signature, nonce, or clock-validation boundary.
- Create a real HTTP/TLS client now: rejected because it would blur P6-01's mock-only OAuth proof
  with P6-03's Build request/stream and P2-09 egress admission responsibilities.

## Validation and rollback

`p6_01_build_oauth` covers strict import/redaction, nested duplicate rejection, expiry ambiguity,
Device poll timing/slow-down/terminal grant, refresh client binding, and mock request redaction.
Targeted tests, Clippy, formatting, crate-boundary validation, diff review, and the P6-01 local full
gate are recorded in the Task report. Reverting P6-01 removes only local provider code, synthetic
tests, documentation, and direct locked dependency edges; it has no schema, credential-store,
network, server, or external-account cleanup.
