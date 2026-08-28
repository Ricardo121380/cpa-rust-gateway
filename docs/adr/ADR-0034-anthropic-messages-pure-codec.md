# ADR-0034: Anthropic Messages pure codec boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-22` |
| Task | `P5-01` |
| Contract | [BC-PROTOCOL-002](../contracts/BC-PROTOCOL-002-anthropic-messages-adapter.md) |

## Context

The gateway has a canonical request/event model and an OpenAI Responses pure codec, but no
Anthropic Messages ingress or egress boundary. Claude Code needs the Messages shape without
letting HTTP, credentials, routing, Provider transport, or a client-write success leak into the
protocol crate.

Unknown request fields must not disappear during admission, while fields that cannot yet be
represented on the first outbound text-only slice must not be silently fabricated. Tool incremental
streaming, Thinking, cache/stop semantics, and Provider token counting have later P5 owners.

## Decision

1. `protocol-anthropic` owns a pure JSON/SSE codec only. It depends on canonical core types plus
   the explicit `serde`/`serde_json` serialization pair; it does not depend on Actix, Router,
   Provider transport, credentials, or the bounded stream implementation. The crate-boundary
   allowlist records those direct dependencies.
2. The codec rejects duplicate JSON names recursively before decoding. It maps required model,
   system/user/assistant text, historical Tool Calls/Results, Tool definitions, and output mode to
   canonical data. Valid but not-yet-executed fields remain in explicit `anthropic.*` raw-extension
   namespaces.
3. Historical user Tool Results can split a Message into canonical `tool` and `user` records. The
   original Message extensions are attached exactly once to the first resulting record, preserving
   data without duplicating a protocol claim.
4. The first outbound response slice encodes a single assistant text block and exact reported
   input/output Usage. It emits typed Anthropic SSE frames once `MessageStart` exists, carrying
   canonical input Usage when it was reported and omitting the field when it was not; it never
   invents input Usage or commits client delivery.
5. Canonical Tool, Thinking, cache-detail, arbitrary event-extension, and stop-reason semantics
   are rejected as unrepresentable at this boundary until P5-03/P5-06 extend the codec with
   dedicated state and fixtures.

## Consequences

- A malformed/ambiguous request or impossible output fails safely as a core `GatewayError`.
- The HTTP layer can later use the codec without losing duplicate-key visibility or moving the
  `FirstSemanticEvent` commit before an actual successful write.
- A retained raw field is admission evidence, not permission for a later Provider to erase it.
  P5-04 must prove an equivalent bridge or exclude the Candidate.
- P5-02, P5-03, P5-06, and P5-07 extend this explicit boundary instead of hiding their behavior in
  a generic serializer.

## Alternatives considered

- Decode directly in Actix: rejected because HTTP JSON extraction hides duplicate-member handling
  and entangles transport with protocol semantics.
- Flatten unknown fields into canonical structs: rejected because they can collide with future core
  fields and force premature schema commitments.
- Treat unsupported Tool/Thinking events as text: rejected because it changes semantics and could
  cause a Tool to execute under a false representation.
- Emit zero or estimated input Usage: rejected because it would falsely claim a measured value.

## Validation and rollback

P5-01 fixtures cover canonical request mapping, duplicate rejection, extension retention,
non-streaming JSON, typed SSE ordering, safe errors, Usage preconditions, and diagnostic
redaction. Rollback removes the codec crate implementation and its public re-exports; it has no
schema, credential, transport, or external Provider traffic effect.
