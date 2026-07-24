# ADR-0037: Fail-closed protocol transform admission

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-22` |
| Task | `P5-04` |
| Contract | [BC-ROUTER-004](../contracts/BC-ROUTER-004-protocol-transform-admission.md) |

## Context

P2 persisted three Route transform modes: `passthrough`, `canonical`, and `lossless_bridge`.
Until P5-04, those labels were configuration only. A later Router could therefore select a
cross-protocol Candidate without proving that the input's roles, retained extensions, Tool
history, cache controls, or Thinking semantics survive the conversion.

`CanonicalRequest` intentionally retains unknown protocol data rather than deleting it. That is
necessary for ingress fidelity, but retention is not evidence that an arbitrary target protocol
can encode the field. A compatibility check must distinguish an exact native forward from a body
reconstruction, and it must be safe for Route Explain without exposing request/model/Tool data.

## Decision

1. `gateway-router` provides a pure `ProtocolTransformInput` analyzer with local
   `ProtocolFormat` values for `OpenAiResponses` and `AnthropicMessages`. P5-04 does not expand
   the global core protocol enum and does not perform HTTP, Endpoint selection, Provider calls,
   event writes, or Snapshot mutation.
2. `Passthrough` is approved only for equal source/target protocols with an `Exact` native body.
   Exact native forwarding preserves retained unknown fields without re-encoding them. A missing
   native body or a cross-protocol pass-through is rejected before scheduling.
3. `Canonical` is same-protocol only; `LosslessBridge` is cross-protocol only. Both reconstruct a
   target body from Canonical semantics and therefore fail closed on request/message/content/Tool
   extensions, opaque content, historical Tool calls/results, Thinking, cache controls, and roles
   outside the target's current protocol slice.
4. The analyzer requires the selected Endpoint's capability evidence for requested streaming,
   Tools, JSON Schema, and parallel Tools. A Tool declaration requires both `tools` and
   `json_schema`; explicit parallel Tool use additionally requires `parallel_tools`.
5. Rejections are fixed value-only codes. Neither the input nor the outcome stores a model name,
   URL, request text, Tool name/ID, cache value, or raw extension value. Its debug view redacts the
   complete Canonical request.

## Consequences

- A Candidate cannot quietly become a cross-protocol fallback merely because its configured
  transform mode says `lossless_bridge`.
- Native pass-through remains useful for protocol-specific extensions, but only while the exact
  original payload is still present. It must not be reconstructed from retained Canonical data.
- The current bridge accepts only a deliberately small common text/Tool-declaration slice. It is
  expected to exclude historical Tool conversations, Thinking, and cache controls until their
  later protocol contracts prove a representation.
- P5-05 owns using this admission result in Endpoint-aware aggregation and keeping same-Upstream
  protocol health/circuit state independent. P5-06 owns semantic mappings for Thinking, stop
  reasons, usage/cache fields, and model rewrite.

## Alternatives considered

- Treat retained raw extensions as target-compatible: rejected because retention preserves data
  locally but does not provide an encoder or a target semantic contract.
- Permit same-protocol Canonical conversion with unknown fields: rejected because Canonical
  reconstruction would still erase the unknown native representation.
- Add the protocol format to `gateway_core` immediately: rejected because only the Router needs
  this P5 admission vocabulary; P5-05 integration may later establish a wider stable boundary.
- Collapse all failures into one generic rejection: rejected because Route Explain needs a stable
  safe reason while still keeping every request value redacted.

## Validation and rollback

Unit tests cover transform topology, exact native-body requirements, clean cross-protocol
admission, role/extension/opaque/Tool/Thinking/cache rejection, streaming/Tools/JSON
Schema/parallel capability checks, and diagnostic redaction. Rollback removes only the pure
Router analyzer and its documents; no database schema, credential, network, Provider, or runtime
health state changes.
