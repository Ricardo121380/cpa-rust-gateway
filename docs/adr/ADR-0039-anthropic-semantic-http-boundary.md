# ADR-0039: Anthropic semantic and HTTP boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-22` |
| Task | `P5-06` |
| Contract | [BC-PROTOCOL-005](../contracts/BC-PROTOCOL-005-anthropic-semantic-http-boundary.md) |

## Context

P5-01 established a pure Anthropic Messages codec, but its first slice deliberately rejected
Canonical reasoning, cache-detail Usage, and explicit stop semantics. It also did not register the
public `POST /v1/messages` handler. Guessing `tool_use` from a completed Tool count is not a
valid stop-reason conversion, and treating block-local `cache_control` as an ordinary unknown
extension would make later routing appear to understand a request semantic that it could not
safely reconstruct.

The Canonical core has open-ended Thinking and Usage shapes, but `ResponseEnd` had no explicit
completion semantic. A client-facing model alias must likewise be rewritten at the Anthropic
boundary just as it is for Responses.

## Decision

1. `ResponseEnd` carries optional explicit `stop_reason` and `stop_sequence`. The generic core
   still permits absent values for protocols that have no such public semantic, but the Anthropic
   encoder requires a non-empty reported stop reason and never derives one from content or Tools.
   Debug output exposes only whether a value was reported.
2. Anthropic request `thinking.type` maps to `CanonicalRequest.thinking.effort`; a positive
   `budget_tokens` is retained under the explicit `anthropic.thinking.budget_tokens` extension.
   Anthropic `cache_control` accepts the known `ephemeral` mode only. Its common `ttl` is mapped
   to request-level `prompt_cache_retention`; conflicting block retentions fail closed. The exact
   block-local control remains attached to its content/tool raw extension, so canonical or bridge
   routing cannot mistake a summary for placement-preserving serialization. Anthropic supplies no
   safe equivalent of `prompt_cache_key`.
3. The Anthropic encoder maps `ReasoningDelta` to `thinking`/`thinking_delta` content blocks and
   maps `cache_read_tokens` and `cache_creation_tokens` to Anthropic's
   `cache_read_input_tokens` and `cache_creation_input_tokens`. Partial Usage snapshots merge
   only missing fields before the final response. Generic `reasoning_tokens`, `cached_tokens`, or
   raw Usage extensions remain unrepresentable and fail as a stream protocol error rather than
   being aggregated or dropped.
4. `gateway-http-actix` registers `POST /v1/messages` using the same authenticated Snapshot
   resolution, bounded Canonical transport, cancellation, final-Usage observation, and
   first-semantic-event delivery boundary as `/v1/responses`. It selects
   `GatewayProtocol::AnthropicMessages`, emits Anthropic JSON/SSE/error envelopes, and constructs
   `AnthropicResponseMetadata` only from the resolved public model.
5. This task does not add a real Anthropic Provider, provider network traffic, or a permissive
   cross-protocol bridge. P5-04 remains fail closed when a body reconstruction cannot preserve the
   exact cache-control placement or other retained semantics.

## Consequences

- Claude-compatible clients can use the public Messages route with deterministic non-streaming and
  SSE behavior through the existing mock/Router seam.
- Stop reasons, stop sequences, Thinking fragments, and cache Usage fields have explicit fixture
  evidence instead of inferred output.
- A Request with incompatible cache controls fails before provider execution; an output with a
  usage semantic Anthropic cannot represent fails after the canonical boundary rather than leaking
  a partial response as complete.
- Public model rewrite and event protocol attribution apply equally to Messages and Responses.

## Alternatives considered

- Infer `tool_use` from the content blocks: rejected because a Tool-containing answer can end for
  another reason and a non-Tool answer can have a non-`end_turn` reason.
- Flatten every cache control into one request field: rejected because it erases the block placement
  required for a later reconstructed body.
- Serialize unsupported reasoning-token or generic cache totals as made-up Anthropic fields:
  rejected because it would claim protocol compatibility without a stable wire contract.
- Add a provider HTTP implementation in this task: rejected because P5-06 owns client protocol
  semantics and HTTP admission, not a new Provider transport or real traffic budget.

## Validation and rollback

- Frozen pure fixtures cover Thinking SSE blocks, explicit stop reason/sequence, cache Usage
  field names, partial Usage merging, and public model metadata.
- HTTP tests cover authenticated JSON/SSE Messages paths, Snapshot alias rewriting, and
  `AnthropicMessages` request attribution without a network Provider.
- Rollback removes the Messages route plus P5-06 codec/core extensions. It has no database
  migration, credential mutation, real Provider call, or cache-key persistence effect.
