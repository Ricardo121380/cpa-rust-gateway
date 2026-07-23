# ADR-0055: Grok Official text-only Responses boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P8-02` |
| Matrix / Contract | `C01`、`C03`、`C31`、`G24`、`G27`; [BC-PROVIDER-013](../contracts/BC-PROVIDER-013-grok-official-responses-boundary.md) |

## Context

P8-01 established the isolated Official API-key and model catalog boundary. Inference now needs a
native vertical slice for the fixed xAI Official Responses URL. Grok Build's Responses module is
not a safe reusable dependency: it embeds an OAuth credential type, CLI-only headers, Build error
meaning, and its own later continuity obligations. `C31` requires source separation even when
the public endpoint schema resembles OpenAI Responses.

The P8 sequence still assigns rate/quota and billing state to P8-03, and Tool, Reasoning, and
Search semantics to P8-04. Therefore P8-02 must make a small but complete text-only path that
rejects deferred semantics, rather than serializing a partial request and silently losing data.

## Decision

1. Define native Official `POST https://api.x.ai/v1/responses` endpoint, request builder, bounded
   JSON/SSE decoder, injected transport, DNS-pinned production transport, and `InferenceAdapter`
   in `provider-grok` without importing Build Responses code or another Provider crate.
2. Construct a request with only `Accept`, `Accept-Encoding: identity`, request-scoped Bearer
   authorization, and JSON content type. The shared client receives it only after an admitted URL
   exactly equals the fixed Official Responses URL; it receives no Build CLI headers, cookie,
   browser marker, user-agent emulation, or proxy discovery.
3. P8-02 accepts extension-free text messages in `developer`, `system`, `user`, and `assistant`
   roles, preserving their order in explicit Responses message items. Tools, Thinking, cache
   fields, provider extensions, opaque parts, historical Tool data, and unsupported roles are
   rejected with `ClientRequestError/Request` before dispatch.
4. Successful non-streaming bodies and SSE records use strict duplicate-field JSON parsing. Only
   completed assistant `message` output with `output_text` is represented. The native SSE state
   accepts the standard creation/progress/item/content/text/completed/failed sequence under a
   per-record 64 KiB limit and produces the canonical lifecycle exactly once per semantic event.
5. Before a Canonical `ResponseStart`, non-2xx, wrong content type, malformed output, and transport
   errors are generic safe protocol errors. After `ResponseStart`, malformed/truncated transport
   or SSE failures emit exactly one Canonical `StreamError`. P8-03 alone assigns HTTP status,
   headers, quota, credential, billing, retry, or health action.

## Consequences

- Official APIs remain independent from Build OAuth state and from its error/continuity behavior.
- Arbitrary network chunking cannot alter the emitted canonical text lifecycle; a malformed complete
  record is atomic and cannot leave state partially advanced.
- Official Tool/Reasoning/Search output intentionally fails closed until P8-04 explicitly maps it.
- The production transport has no ambient API key, file, environment, proxy, server, or live
  request behavior; all P8-02 evidence is a scripted synthetic transport.

## Alternatives considered

- Reuse `build_responses`: rejected because it would couple Official API keys to Build OAuth/CLI
  profile and violate `C31` even if the outward JSON appears compatible.
- Reuse `provider-openai-compatible`: rejected because Provider-private cross-crate request and
  state reuse is prohibited by crate boundaries and would hide Official-specific egress/state.
- Permit Tool/Reasoning/Search fields but drop them: rejected because it changes user semantics.
- Infer 401/403/429 or quota mutation now: rejected because P8-03 owns that classification.
- Run a real Official test request: rejected by `CR-P7-G7-001` until G7 and P7 Delivery Gate pass.

## Validation and rollback

Synthetic fixtures prove exact request method/headers/target admission, diagnostic redaction,
text non-stream lifecycle, arbitrary SSE chunk invariance, deferred-semantic rejection, generic
pre-start failure, and a single post-start `StreamError`. Local formatting, Clippy, full Provider
tests, source/crate-boundary, document-link, Secret, supply-chain, and RustSec checks are required
before this task gains local evidence.

Rollback removes only the P8-02 module, fixture tests, ADR, contract, report, and traceability
entry. It changes no schema, route, persisted state, credential, server, proxy/TUN setting,
network traffic, or production configuration.
