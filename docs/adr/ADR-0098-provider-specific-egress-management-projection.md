# ADR-0098: Provider-specific egress management projection

- Status: Accepted — P13-11E4 `DONE_WITH_BOUNDARY`; formal tag and Delivery Gate passed
- Date: 2026-08-18
- Scope: protected, read-only projection of exact Provider egress/session/clearance runtime state

## Context

P13-11E E0-E3 established an exact Provider-aware runtime state model and formally closed it under
`phase-p13-provider-egress-complete`. The model intentionally separates Credential, Quota, Egress,
Provider Session and Clearance. Existing management endpoints expose selected Config Version
bindings and Provider account-pool summaries, but neither is an exact projection of the three E1
runtime maps.

A management UI needs observable source facts without gaining a second scheduler, direct access to
Provider crates, or permission to operate recovery. A single aggregate `runtime_status` would lose
the source domain and could falsely present a session or clearance problem as an account or egress
problem.

## Decision

Add an injected, read-only management facade and a dedicated
`GET /admin/operations/provider-egress-status` contract.

The facade produces one atomic, value-free snapshot. Each row represents exactly one source domain:

- `egress` binds one Provider/Upstream/Endpoint and direct or opaque named target;
- `session` additionally binds an exact Credential and credential/session revisions;
- `clearance` additionally binds the exact session lineage, target and clearance revision.

The row retains the closed Provider channel kind. The management field `channel_id` carries the
runtime's exact configured `EndpointId`; it is not a Provider-name-derived logical channel or a
second identity. The projection does not publish a synthesized overall status.

## Snapshot and lineage decision

The snapshot contains separate identities for:

1. active Config Version ID and Config revision;
2. runtime-state snapshot ID/revision;
3. fixed `sampled_at_ms` used to evaluate deadline-derived effective states.

A cursor binds all three, the exact filters, and the last stable row key. The application retains a
bounded set of immutable snapshots, so later pages continue against the exact retained snapshot even
if live runtime state advances. An unknown/expired retained snapshot or a mismatched Config lineage,
snapshot runtime revision, observation time, filter set, or stable key returns a safe conflict.
Config revision is never used as a runtime-state revision, and a later page cannot silently
recompute effective state at a newer clock value.

The source takes an atomic snapshot before pagination. The HTTP layer does not expose or iterate the
runtime's internal mutable maps directly. `channel_id` is the exact configured `EndpointId`.
`channel_kind` is retained and uniqueness-validated for that exact identity, but is not redundantly
encoded in the cursor key. Runtime IDs are byte-bounded and the opaque cursor is bounded to 4096
characters.

## Source ownership decision

The initial process composition may inject only the existing native Grok Build/Console source.
That limitation is represented honestly:

- Build and Console rows are present only when the exact runtime state exists;
- Web/session/clearance rows may be empty until the Web runtime source is explicitly wired;
- an empty clearance set is not `fresh`, `available`, or evidence of successful Web transport;
- generic compatible egress remains owned by the P13-11B/D compatible runtime source and is not
  silently merged with native Grok state;
- every other Provider/channel needs a separately injected exact source.

The facade may later compose multiple explicit source owners, but duplicate or conflicting exact
identities fail closed. No source may infer state from a credential envelope, relay name, or
Provider label.

Provider-session projection is valid only for Console and Web channels. Clearance is valid only for
Web, whose egress/clearance target must be named. A live `refresh_in_flight` projection requires the
exact atomic owner to agree with the retained state, but its generation and ticket are never
exposed. Invalid or poisoned source shape makes the complete read safely unavailable instead of
returning a partial or fabricated snapshot.

## Management and security decision

The route reuses BC-MGMT-003 Management Key and peer/origin admission, requires the selected Config
Version lineage, returns `Cache-Control: no-store`, and uses bounded strict query/response schemas.
It is GET-only. Existing browser policy remains authoritative; no unsafe management action is
introduced.

Responses, errors, Debug output and audit metadata never contain URLs, proxy endpoints, DNS
answers, API keys, OAuth/SSO/Cookie/Header material, credential plaintext/ciphertext, bodies, raw
Provider responses, client-key digests, refresh tickets or ownership generations. Deadlines and
opaque revisions are safe only when bound to their exact value-free lineage.

## Consequences

- Operators can distinguish egress, session and clearance facts without changing serving state.
- Pagination requires a runtime snapshot revision in addition to Config Version revision.
- The existing static account inventory and Provider account-pool summary remain unchanged.
- E4B updated the authoritative OpenAPI, synchronized Prism/generated client, and recorded the
  action-required Claude Code handoff; no formal frontend UI was added by the backend slice.
- The current process composition may display only Build/Console state. This is incomplete
  source coverage, not a failed or fabricated Web result.
- Generic compatible state requires its own explicit source adapter.

## Rejected alternatives

1. **Extend ProviderAccountPoolItem.runtime_status.** Rejected because it collapses failure domains
   and lacks exact egress/session/clearance lineage.
2. **Expose the router maps directly to HTTP.** Rejected because it lacks a stable atomic snapshot,
   pagination revision, source abstraction, and crate boundary.
3. **Treat missing state as available.** Rejected because absence is not health evidence.
4. **Add recovery actions with the read projection.** Rejected because actions have different
   authorization, audit, failure and real-network boundaries.
5. **Merge compatible and native state by Provider name.** Rejected because compatible egress is
   Upstream/Endpoint/Credential-owned and names do not prove a common scheduler or state owner.

## Evidence boundary

P13-11E4 local evidence passed: gateway-router `170/170`, gateway-control `87/87`, gateway bin
`114/114`, E4 HTTP `4/4`, management OpenAPI `12/12`, existing management runtime `3/3`, the
provider-grok full suite with authorized/live tests intentionally ignored, strict Clippy for all
touched Rust crates, fmt, and Prism check. E3 Web `11/11` remains historical transport-free
evidence; a synthetic HTTP clearance row proves serializer shape only. Independent final review
passed with no remaining P0-P3 finding. The aggregate local Full passed `43/43` from
`2026-08-18T06:59:51Z` through `2026-08-18T07:01:49Z`. Immutable annotated tag
`phase-p13-provider-egress-status-complete` points to exact closeout commit
`ce98faa9306d076f5af53b9eef0c818abb1cb9c8`; formal Delivery Gate
[32110872875](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/32110872875) passed
Authorize, Fast, Full supply-chain and Required in `4s` / `7m02s` / `1m00s` / `3s`.

This ADR authorizes and records the implementation, contract and formal process evidence only. It does not
demonstrate a real Provider account, proxy, DNS path, production Web clearance, Build/Console
fixed/pool transport, public CPAR endpoint, staging environment, production usability, Autoreg, or
real health. E5 remains separately unauthorized.

ADR-0097 and BC-SEC-008 remain the accepted E0-E3 design/security history. This ADR is their
management-projection successor and does not alter the old tag or Gate.
