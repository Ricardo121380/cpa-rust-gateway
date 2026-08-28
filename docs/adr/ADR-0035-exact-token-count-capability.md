# ADR-0035: Exact token-count capability and Anthropic route

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-22` |
| Task | `P5-02` |
| Contract | [BC-PROTOCOL-003](../contracts/BC-PROTOCOL-003-exact-token-count-capability.md) |

## Context

Anthropic-compatible clients use `POST /v1/messages/count_tokens` before an inference request. A
byte, character, or unrelated-tokenizer estimate is unsafe: it can make client-side context and
budget decisions based on a value the selected model cannot honor.

P5-01 supplies a pure Anthropic decoder and encoder but deliberately has no HTTP, Snapshot, or
Provider capability knowledge. P5-05 will later select an Endpoint for an aggregated Route, so
P5-02 must introduce a narrow route-aware seam without prematurely choosing an Upstream or making
a Provider request.

## Decision

1. `gateway-core::ExactInputTokenCount` is an opaque value constructed only by a Provider or
   explicitly proven compatible local tokenizer. It has no estimator or conversion from request
   text.
2. `gateway-provider::TokenCountCapability` has exactly two states: an
   `ExactTokenCountAdapter`, or explicit unsupported capability. The unsupported state returns
   `TokenCountUnsupported/Model`; it never returns a best-effort number.
3. `gateway-router::CountTokensExecution` carries the authenticated canonical request and the
   Snapshot-approved `RouteId`. The canonical request intentionally keeps the client-supplied
   model reference, including an Alias, while `RouteId` is the proof of resolved routing. The
   initial direct Provider executor ignores that identity only for its explicitly local seam;
   P5-05 owns Endpoint aggregation based on it.
4. Actix authenticates, decodes, and resolves the model before invoking the count executor.
   Success is exactly `{"input_tokens": <exact u64>}`. The default executor fails closed with an
   Anthropic safe `invalid_request_error` and HTTP `422`; no field exposes an internal error code,
   Provider diagnostic, Endpoint, credential, or estimate.
5. This Task creates no Provider implementation, credential lookup, HTTP client, background
   tokenizer, real request, or configuration-driven fallback.

## Consequences

- Clients receive an unambiguous result only when an exactness attestation exists.
- A Snapshot Alias is admitted only if it resolves to an authorized Route; the executor can later
  choose the compatible Endpoint without treating an Alias as routing evidence.
- The existing OpenAI error encoder recognizes the new stable core error category so the expanded
  error enum remains exhaustively and safely mapped across protocol crates.
- P5-05 must replace or compose the direct executor with a Route/Endpoint-aware aggregation
  executor. It must not add an estimate to this capability.

## Alternatives considered

- Estimate from UTF-8 length, words, or a generic tokenizer: rejected because it cannot attest to
  selected-model compatibility.
- Return `0` or omit the count when unsupported: rejected because either form can be mistaken for
  a real count.
- Rewrite the canonical request model from Alias to public model: rejected because the client
  reference remains needed for Provider encoding and observability; the separately typed Route ID
  is the authorization and routing proof.
- Put Provider/Endpoint selection in the Actix handler: rejected because it would leak transport
  and credential concerns into HTTP and duplicate the Router's aggregation boundary.

## Validation and rollback

Unit and in-process Actix tests cover exact response shape, unsupported rejection, duplicate-safe
Anthropic decoding, capability refusal without estimation, and Snapshot Alias-to-Route identity.
Rollback removes the route and capability seam; it needs no schema migration, credential change,
network cleanup, or Provider-side action.
