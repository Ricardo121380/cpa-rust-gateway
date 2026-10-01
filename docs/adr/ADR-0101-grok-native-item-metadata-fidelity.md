# ADR-0101: Grok native Responses item and part metadata fidelity

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-10-01` |
| Task | `CPAR-C / CPAR-35, CPAR-38, CPAR-39`; issues #44, #47, #48; evidence aggregation #57 |
| Matrix / Contract | `B11`, `B12`, `B16`, `C01`, `C03`, `C18`, `C31`, `F02`; [ADR-0100](ADR-0100-exact-protocol-semantics-and-usage-evidence.md), [BC-PROVIDER-015](../contracts/BC-PROVIDER-015-grok-official-tool-reasoning-capability.md), [BC-CORE-003](../contracts/BC-CORE-003-canonical-event-state-machine.md) |

## Context

The C batch requires native item identities, reasoning, citations, and replayable history to
survive the actual public execution path. Official and Console previously flattened reasoning
and rejected representable summary metadata. Build already emitted native metadata but its
inference adapter discarded reasoning when the request did not explicitly ask for it. Native
event-only fields could also disappear before the completed snapshot was checked.

The Canonical stream and public Responses codec already have explicit item/part metadata. The
missing boundary is exact provider decoding and request replay, not a new provider identity or
authentication scheme. Passing local fixtures cannot close real-channel combinations.

## Decision

1. Official and Console emit `OutputItemStart` / `OutputItemEnd` plus indexed semantic deltas for
   reviewed native message, reasoning, and function-call items. Preserve stable item/call/name
   identities, native order, reasoning `summary` and `content` as separate ordered parts,
   message `phase`, reviewed URL citations, and final null/empty `logprobs` representations.
2. Build, Console, and Official share only closed event/part validation and correlation helpers.
   Authentication, endpoint/header profiles, quota, egress, account state, and encrypted-reasoning
   ownership stay isolated. Official and Console have no ownership path for non-null ciphertext
   and reject it, including a string resembling a gateway-owned token.
3. Validate native event-only fields before dispatch. Unknown semantic fields, nonempty logprobs,
   invalid or changed identities/indices, contradictory part snapshots, citation changes, phase
   changes, and terminal reordering are protocol errors. Missing legacy indices mean part zero;
   supplied indices must match the item and part they name. An optional function-event `call_id`
   or `name` must confirm the started call.
4. Preserve incremental annotations and annotations present in an empty initial text part. Final
   item metadata must confirm observed annotations and completed part snapshots. Parts and
   annotation indices are bounded to 64; output items are bounded to 256; retained annotation
   payloads are bounded to 1 MiB in addition to existing parser/text bounds.
5. Preserve validated upstream reasoning regardless of whether the caller requested thinking.
   Responses exports its native structure. Chat/Messages can represent one plain reasoning body
   exactly; richer summary, multiple reasoning parts, phase, and citations must fail closed on
   those bridges. Request controls still determine the submitted upstream parameters.
6. Official/Console request builders preserve the reviewed assistant history identities, status,
   phase, text-part annotations/logprobs, reasoning parts, and function-call identity. Unknown
   extensions fail before transport. There is no account fallback or same-attempt replay.

## Consequences

- This supplements the codec subset in [ADR-0057](ADR-0057-grok-official-tool-reasoning-capability-boundary.md).
  Its provider isolation and native Search non-capability remain unchanged.
- Native metadata survives JSON/SSE projection, event serialization, and explicit public Responses
  history replay. This does not verify each channel's stored continuity, compaction, or WebSocket.
- A supported native Responses response may be unrepresentable in Chat/Messages. That remains a
  visible incomplete combination; it is not a new approved exclusion from spec #9.
- Kiro output-cap mapping and Web native multi-turn continuity remain software/protocol blockers.
  Web tools and exact Messages token counting retain the two previously accepted restrictions.
- No persistence schema, management API, frontend business code, credential, service, or deployment
  changes are part of this decision.

## Alternatives considered

- Flatten all reasoning and drop metadata: rejected because the caller loses summary/body hierarchy,
  identity, attribution, and replay semantics.
- Reuse the whole Build decoder/profile for Official or Console: rejected because its ownership and
  runtime boundaries differ. Only the pure closed validation helpers are shared.
- Silently accept unknown event fields until a terminal snapshot arrives: rejected because event-only
  semantics may never recur in that snapshot.
- Suppress unrequested reasoning: rejected because request preferences do not authorize deletion of
  a validated upstream response.

## Validation and rollback

`cpar_c_native_metadata` covers JSON/SSE chunk invariance, public Responses encoding and history
request replay, incremental/initial annotations, multiple indexed summary/body/text parts,
intermediate/final contradictions, output order, optional call identifiers, and exact plain-reasoning
bridges. `runtime::batch_c::grok_native_metadata_survives_public_responses_and_rejects_lossy_bridges`
exercises all three native runtime factories and the public HTTP/protocol path with injected
transports and synthetic credentials. Required local Full and two-axis review are recorded at an
immutable source revision in the C convergence evidence. Real-provider acceptance remains separate.

Rollback reverts this implementation, tests, contract supplement, and index entry together. There
is no data migration or runtime mutation to undo. The previous source still fails the newly added
fidelity regressions; rollback does not make those combinations accepted.
