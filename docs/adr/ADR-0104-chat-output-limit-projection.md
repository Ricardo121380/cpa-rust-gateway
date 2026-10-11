# ADR-0104: Strict Chat output-limit projection

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-10-11` |
| Task | CPAR-38 / #47; delivery #57; parent #9 |
| Contracts | [BC-PROTOCOL-008](../contracts/BC-PROTOCOL-008-openai-chat-completions-codec.md), [BC-ROUTER-004](../contracts/BC-ROUTER-004-protocol-transform-admission.md) |

## Context

The public Chat request retained `max_completion_tokens=4096`, but router admission
recognized only legacy `max_tokens`. A real request and the isolated process reproduction
both returned HTTP400 before any Attempt. Removing the control would change the request.
The codec already retains it; the existing router projection owns the missing proof.

## Decision

Admit one modern Chat total-output limit and map it exactly to Responses
`max_output_tokens`. Responses-to-Chat now emits the modern spelling, retaining the
reasoning-inclusive total. Native Chat keeps its input spelling and value. Existing legacy
`max_tokens` positive-integer mappings remain available within the established compatible
protocol slice; this is not a new universal equivalence or a promise that a model accepts
legacy parameters. Native model restrictions still apply.

The representation domain remains positive u64. Missing OpenAI controls remain absent;
explicit null remains rejected under CPAR's existing strict execution policy. Any two
Chat limit fields, equal or different, and foreign limit namespaces are rejected before
credential selection. No value selection, overwrite, field deletion or capability change.
Every mode, including exact passthrough, uses the existing admission boundary.

The generic bounded Channel Pin must construct its control in the source protocol's
namespace; the router then projects it. Native Build and Codex probe policy do not change.
Pin cannot prove a separate public request's parameters or history.

## Consequences

The modern Chat request can execute without substituting legacy parameters. Shared
projection and the product binary change, so old Responses real-provider evidence stays
bound to its original source and cannot qualify the successor binary. All six basic
combinations require fresh source-bound real acceptance before claiming successor coverage.
No schema, credential lifecycle, retry or billing behavior changes.

## Alternatives considered

- Drop the new field, retry with legacy spelling or lower a capability: loses user intent.
- Merge equal fields or choose a larger/smaller limit: no precedence/equivalence proof.
- Treat null as omission: silently drops a retained control and changes existing admission.
- Add a Grok-only converter: duplicates semantics owned by the shared router.

## Validation and rollback

Official evidence checked on 2026-10-11: [Chat create](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
and [output token counting](https://developers.openai.com/api/docs/guides/token-counting).
They establish modern total-output semantics and legacy deprecation, not precedence or
provider acceptance. Public process tests preserve the actual 4096 control, Request/Attempt/
Usage lineage and independent loopback receive counts; native Build assembly has separate
in-process transport evidence. Router tests cover native/bridge/passthrough modes and u64
boundaries. Rollback restores the prior rejection behavior; existing state/Usage compatibility
requirements in ADR-0100 and ADR-0103 still apply. This decision authorizes no deployment.
