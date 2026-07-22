# ADR-0044: Fixed Grok Build Responses boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-22` |
| Task | `P6-03` |
| Matrix / Contract | `C28`; Behavior 4/5/12; [BC-PROVIDER-003](../contracts/BC-PROVIDER-003-grok-build-responses-boundary.md) |

## Context

P6-01/P6-02 provide a redacted, revision-guarded Build OAuth Credential, but deliberately do not
construct an inference request or interpret an upstream response. Grok Build's OAuth profile uses a
fixed CLI chat-proxy Responses endpoint and identity headers rather than the generic
OpenAI-compatible API-key profile. The request must still pass P2 exact-target egress admission,
and the response needs bounded non-streaming and SSE decoding before later P6 Tasks attach quota,
cache affinity, response ownership, or account-state policy.

The generic `provider-openai-compatible` crate has a similar request encoder, but the crate
boundary forbids one Provider-private crate from importing another. Reusing it would make Grok
Build's fixed OAuth behavior depend on another Provider's private API and blur independent
Provider evolution.

## Decision

1. Keep the Grok Build request encoder and decoder entirely in `provider-grok`. It encodes the
   frozen, lossless Responses subset locally and never imports another Provider crate.
2. Fix Build inference to `https://cli-chat-proxy.grok.com/v1/responses`, with OAuth `Bearer`,
   `x-xai-token-auth: xai-grok-cli`, the frozen CLI version, and the frozen User-Agent. The
   selected upstream model is the only serialized model identity. The historical `Connection`
   header is deliberately omitted because it is hop-by-hop and the shared transport owns pooling.
3. Require `AdmittedEgressTarget` to equal that exact fixed URL before constructing the shared
   `UpstreamHttpRequest`. The outbound wrapper stores Authorization in zeroizing memory and
   redacts target, header values, and body content from `Debug`.
4. Decode non-streaming JSON up to 1 MiB and SSE records up to 64 KiB. Every object is parsed with
   duplicate-name rejection, SSE input is committed atomically per supplied byte chunk, and HTTP
   error bodies are reduced to a bounded status/signal envelope without retaining their text.
5. Preserve only representable Canonical output. Response and output-item identities must agree;
   Tool call id/name/final arguments must agree with the declared and incremental forms; blank or
   empty-object Tool arguments normalize to `{}`; non-object or incomplete arguments fail closed.
   Each done item must be marked completed, and final output identities must exactly match the
   completed declared-item set. P6-07, not this decoder, maps HTTP status/signals to Credential,
   Quota, retry, or health state.

## Consequences

- The shared P3 transport can execute the resulting exact-target request without the Provider
  crate creating a socket, changing proxy/TUN settings, or bypassing egress admission.
- A malformed stream cannot partially advance a caller-visible decoder state, and an ambiguous
  Tool completion cannot reach a downstream protocol adapter.
- The response layer exposes safe error signals only. It does not claim that `401`, `403`, `429`,
  Billing, quota, or transient failures have an account-level meaning.
- P6-03's real test-account validation remains separately authorized. Under `CR-P6-03-001`, it
  may use a finite documented matrix, but each ignored-harness process still has exactly one send
  and no tracked file carries a real credential, target-private model mapping, proxy profile,
  request body, or response data.

## Alternatives considered

- Reuse `provider-openai-compatible`: rejected because the Provider-private dependency direction
  is explicitly forbidden.
- Accept `serde_json::Value` directly: rejected because duplicate object names would be silently
  overwritten.
- Forward `Connection: Keep-Alive`: rejected because shared transport already controls connection
  reuse and rejects hop-by-hop request headers.
- Classify or mutate account state while parsing an error body: rejected because that would
  conflate syntax evidence with P6-04/P6-07 policy and recovery ownership.

## Validation and rollback

The committed corpus contains only synthetic endpoint-independent fixtures. It proves exact fixed
request construction and egress handoff, redacted diagnostics, arbitrary 1/2/7/31/257/4096-byte
SSE segmentation equivalence, strict duplicate-name rejection, truncation, atomic state recovery,
empty Tool normalization, Tool-name consistency, bounded error signals, and non-streaming/SSE
semantic projection equivalence. Targeted Clippy, format, crate-boundary, diff, and secret checks
run locally without a socket.

Rollback removes only the Build Responses module, synthetic fixtures/tests, and its P6-03
documentation. It changes no schema, persisted OAuth runtime state, router policy, proxy/TUN rule,
server, account, or external Provider state.
