# ADR-0066: Grok Web source-labelled quota observations

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P9-06` |
| Matrix / Contract | `C31`、`C33`、`C34`、`D28`、`E27-E29`; [BC-PROVIDER-020](../contracts/BC-PROVIDER-020-grok-web-source-labelled-quota.md) |

## Context

Grok Web may expose quota-related observations through more than one protocol surface. A local projection must not assert that a synthetic REST or gRPC-Web value is live billing authority, combine different sources opportunistically, or carry observations across an SSO credential or browser-egress rotation. P9 has no authority to request a real quota endpoint.

## Decision

1. Decode only two deliberately synthetic, bounded fixture shapes: REST `{tier, window}` and gRPC-Web `{quota:{tier, window}}`. Both require exact fields, strict JSON, bounded opaque tier/raw-window labels, non-zero totals/durations, `remaining <= total`, and a reset later than the observation.
2. Retain every snapshot under exactly `(source, coarse window kind)`. REST and gRPC-Web never overwrite each other, and a provider-defined window retains its opaque type internally without giving it billing semantics.
3. Bind state to account reference, SSO lineage, credential revision/expiry, and egress-session identity. Older observations are ignored; distinct same-instant observations are rejected without mutation; stale, expired, or mismatched sessions cannot write state.
4. Surface `Observed` confidence only. It denotes a provider-reported local projection, not scheduling authority, entitlement, or financial truth.

## Consequences

- Quota evidence cannot cross Web account, credential lifecycle, or egress-session boundaries, and it remains isolated from Build and Official state.
- Different Web protocol surfaces remain observable for future diagnosis without silent merging.
- P9-09 may introduce a live decoder or transport only under separately approved Canary scope; no P9-06 API sends HTTP, gRPC-Web, browser, Cookie, proxy, TLS, DNS, or server traffic.

## Alternatives considered

- Merge sources by “most recent” window: rejected because it hides protocol disagreement and could make an unverified source authoritative.
- Infer tier semantics or billing from labels: rejected because tier text is opaque and protocol drift-prone.
- Bind only to account: rejected because a credential refresh or egress change can invalidate Web session context.

## Validation and rollback

Three synthetic tests cover source/window isolation, stale/conflicting observation handling, exact session/expiry rejection, malformed/cross-shape values, and redacted diagnostics. Rollback removes this module, test, and documentation only; it performs no external request or configuration change.
