# ADR-0073: Protected routing and Client Key workflows

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-24` |
| Task | `P10-05` |
| Matrix / Contract | `H03`、`H05-H06`、`H18`、`H21`、`L17-L20`、`L27-L29`、`L35`; [BC-MGMT-006](../contracts/BC-MGMT-006-protected-routing-client-key-workflows.md) |

## Context

P10-01 freezes the routing/access and Client Key management contract, P10-02 admits only
authorized management requests, P10-03 provides the generated same-origin client, and P10-04
establishes Version/ETag/audit graph transactions. P10-05 needs to make a draft public-model
graph and Client Key lifecycle operable without accidentally publishing it, deriving a Provider
request, treating a generic management page as a runtime dashboard, or leaking an issued Key.

The storage model predates P10-05 and can hold legacy Route policies that the frozen P10-01 HTTP
contract intentionally does not expose. Reclassifying those records in the response would make a
management read claim a topology/scheduler fact that storage does not contain.

## Decision

- Extend `ManagementMutationService` and the existing protected Actix handlers with focused
  Version-scoped Public Model/Alias/Route/Candidate/Access Group/grant/Client Key operations. Reuse
  the P10-04 exact revision and audit transaction rather than create a second mutation path.
- Treat Route, Candidate and grant parent identity as route/path state; decode only the corresponding
  frozen JSON body. The local validation operation checks stored draft graph references only. It
  does not compile/select a route, disclose runtime exclusions or reach an upstream.
- Let `ClientKeyService` issue into a service-injected, management-only seam. Persist the Prefix
  and HMAC digest before returning the complete Key; never add a Pepper source to the HTTP or SPA
  layer. The complete Key is an immediate response-only capability, not a retrievable resource.
- Fail closed when a legacy stored Route policy cannot be expressed by the P10-01 enum. Return the
  fixed internal management envelope rather than mapping it to
  `smooth_weighted_round_robin`.
- Extend the static SPA resource controls through generated operations only. Keep issue output in a
  distinct transient pane, clear it before subsequent activity/reload, and clear resource/parent
  identifiers whenever the resource type changes to prevent cross-resource accidental submission.

## Consequences

Operators can construct and change a complete draft `minimax-m3` access graph while each state
change remains revisioned and auditable. The graph is still unavailable to public requests until
the later publication work; P10-05 does not provide runtime status, Explain, publication/rollback,
backup/restore, static serving or a production listener.

Legacy Route records must be repaired through an explicit future migration or supported contract
evolution before they can be read through this P10-05 response shape. This is intentionally safer
than a semantically false response.

## Validation and rollback

The service, protected HTTP, SPA and local browser fixture tests use only in-memory or synthetic
fixtures. They cover ETags, stale rejection, topology ownership/cascades, Client Key redaction,
immediate display clearing and generated same-origin requests. The fixture contains no Provider
transport or usable credential.

Rollback removes only P10-05 handler/service/store/SPA additions and its local fixture evidence.
It leaves the P10-01 contract, P10-02 admission boundary, P10-03 generated build, P10-04
upstream resource workflows, published data-plane state and any external system unchanged.
