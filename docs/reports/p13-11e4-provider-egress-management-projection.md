# P13-11E4 Provider-specific egress management projection report

Status: `READY_FOR_FORMAL_DELIVERY_GATE` — focused implementation gates, independent final review
and the single aggregate local Full passed; the formal Delivery Gate remains pending

Date: 2026-08-18

## Outcome first

P13-11E4 now provides a protected, GET-only, value-free projection of the exact Provider egress,
Provider session and clearance facts already composed inside CPAR. The implementation preserves the
three source domains instead of collapsing them into account health, binds every page to one Config
Version and one immutable runtime snapshot, and performs no Provider, proxy, DNS or recovery work.

The current process source projects composed Grok Build and Grok Console state only. Production Web
and clearance rows may therefore be empty. A synthetic clearance row used by the HTTP test proves
the serializer/contract shape only; it is not evidence of a live Web session, clearance executor or
Provider request. Generic-compatible egress remains under its separate P13-11B/D source owner.

This successor does not reopen or extend the formally accepted P13-11E E0-E3 tag/Gate:

- immutable tag: `phase-p13-provider-egress-complete`;
- exact commit: `ba2261a5414fe73d147a102a266abd3e9a7fbb5b`;
- formal Gate: `32044424886`;
- E0-E3 result: `DONE_WITH_BOUNDARY`;
- E5 real-network work: `DEFERRED_UNAUTHORIZED`.

## Authorities

- [E4 Change Request](../change-requests/CR-P13-11E4-PROVIDER-EGRESS-STATUS-001.md)
- [ADR-0098](../adr/ADR-0098-provider-specific-egress-management-projection.md)
- [BC-MGMT-019](../contracts/BC-MGMT-019-provider-specific-egress-runtime-status.md)
- [P13-11E4 Task Card](../06-development-plan.md)

ADR-0097, BC-SEC-008 and the original E0-E3 CR remain immutable historical authorities for the
runtime state and recovery-isolation decision. E4 is a separate management acceptance boundary.

## Delivered implementation

### E4A — atomic runtime snapshot and typed facade

- `gateway-router` captures egress, session and clearance maps plus one monotonic runtime revision
  under one read lock and evaluates all deadline-derived states at one fixed `sampled_at_ms`;
- mutation advances the revision, while read-only effective-state projection does not write back;
- `gateway-control` exposes a strict tagged domain union, deterministic ordering, bounded filters,
  stable keyset pagination and rejecting/snapshot facades;
- exact IDs, revisions, deadlines and domain shapes are validated before publication;
- duplicate or conflicting identities, poisoned domain/channel shape and over-limit state fail
  closed rather than yielding a partial snapshot.

`channel_id` is the exact configured `EndpointId`. `channel_kind` remains an explicit row field and
is uniqueness-validated for the exact Provider/Upstream/Endpoint identity, but is not redundantly
encoded in the cursor key. Provider-session rows are permitted only for Console and Web; clearance
rows only for Web; Web egress/clearance requires a named sticky target. A live
`refresh_in_flight` state requires the exact atomic owner to agree with the retained state, while
owner generation and ticket values are never exposed.

### E4B — process adapter and protected management contract

- the gateway process adapter snapshots the composed native Grok runtime without contacting a
  Provider or acquiring a serving lease;
- immutable snapshots are retained for a bounded interval, so a cursor continues against the same
  snapshot even when live runtime state advances;
- an unknown/expired retained snapshot or Config/runtime/observation/filter/key mismatch returns a
  safe `409`;
- IDs are byte-bounded and the opaque cursor is limited to 4096 characters;
- absent declared runtime is represented as an empty source scope, while invalid/poisoned source
  shape or a rejecting source returns safe `503`;
- `GET /admin/operations/provider-egress-status` reuses Management Key, peer/origin admission and
  exact `X-Config-Version`, returns `Cache-Control: no-store`, and has no unsafe method;
- the authoritative management OpenAPI, Prism contract and generated client were synchronized;
  `docs/cross-boundary-log.md` records an action-required Claude Code handoff;
- no formal frontend UI or operator action was added by this backend slice.

## Focused local evidence

| Surface | Result |
|---|---|
| gateway-router full suite | `170/170` passed |
| gateway-control full suite | `87/87` passed |
| gateway binary full suite | `114/114` passed |
| P13-11E4 management HTTP | `4/4` passed |
| management OpenAPI contract | `12/12` passed |
| existing management runtime regression | `3/3` passed |
| provider-grok full suite | passed; authorized/live tests remain intentionally ignored under the no-network boundary |
| historical P13-11E3 Web fixture | `11/11` passed; transport-free historical evidence only |
| strict Clippy | gateway-control, gateway-router, gateway-http-actix, gateway and provider-grok passed |
| formatting and client contract | `cargo fmt` and Prism check passed |

The tests cover atomic multi-domain snapshots during mutation, monotonic revision/no-op behavior,
deadline projection without writeback, strict domain/channel/target ownership, bounded IDs/cursors,
retained-snapshot pagination across live drift, exact filters, duplicate/conflict rejection,
management admission, strict query decoding, safe `400/409/503`, no-store responses, source-shape
unavailability and forbidden-field serialization.

## Review state

Independent final review passed with no remaining P0, P1, P2 or P3 finding. The review rechecked
channel/domain capabilities, clearance owner bijection, atomic runtime revision, bounded retained
snapshots and cursor replay, safe HTTP error mapping, truthful Build/Console-only source composition,
value-free projection surfaces, authoritative OpenAPI/Prism parity and the Claude Code handoff.

The aggregate `./scripts/check.sh full` passed `43/43` on Darwin arm64; see the immutable local
[aggregate receipt](evidence/p13-11e4-aggregate-full-20260818.md) and [phase review](evidence/p13-11e4-phase-review-20260818.md).
The GitHub formal Delivery Gate has not run for E4, so the local result is
`READY_FOR_FORMAL_DELIVERY_GATE`, not `DONE_WITH_BOUNDARY`. No focused slice triggers its own
expensive remote Gate; ordinary commit, push, review and merge remain independent from Gate
frequency.

## Redaction and side-effect boundary

E4 projection responses, errors, projection Debug values, cursors and receipts contain no
endpoint/proxy URL, DNS answer, API key, OAuth/SSO/Cookie/Header material, credential
plaintext/ciphertext, body, raw Provider response, client-key digest, refresh ticket or owner
generation. Reads do not decrypt Store material, obtain a serving lease, start
probe/refresh/reauth/replenishment, mutate a circuit, or call Autoreg.

No Provider, proxy, DNS, public CPAR, server, staging or production action was performed. Passing
these tests proves only that already-injected process facts are projected safely; it is not proof of
real account health, quota, endpoint connectivity, proxy usability, Build/Console fixed/pool
transport, production Web/clearance, or production readiness.

## Rollback

The facade remains injected and read-only. Removing the E4 route/facade restores the earlier
management surface without changing serving state, durable Config Version data, credentials, or the
immutable E0-E3 evidence. A missing or invalid source continues to fail closed rather than changing
the data plane.
