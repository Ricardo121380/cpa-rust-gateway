# ADR-0095: Compatible serving transport handoff

Status: **Accepted locally — P13-11C LOCAL_PASS_PENDING_PHASE_GATE**

Date: 2026-08-16

## Context

P13-11A defined a Provider-neutral compatible-endpoint egress profile and P13-11B compiled that
profile against the active Config Version, existing Credential pools, shared Health/Quota, and a
single-Upstream transport registry. Serving still uses the legacy Endpoint transport profile
directly, so the composed egress lease is not yet the request-time transport owner.

The serving path already has one authoritative `RouteCredentialScheduler` and one
`CredentialLease`. Acquiring another Credential lease inside the compatible runtime would double
capacity accounting and could select a different Credential. Replaying an HTTP request after the
transport has accepted it is also unsafe because CPAR cannot prove that no request bytes reached
the upstream.

## Decision

P13-11C adds an exact selected-Credential egress handoff:

1. The existing scheduler remains the only Credential lease owner. The compatible runtime accepts
   that exact live lease, rechecks its kind/revision plus Health/Quota/expiry lineage, and acquires
   only egress-node capacity.
2. The selected proxy identity is overlaid on the existing response-mode-specific transport
   profile. JSON/SSE connect, TTFB, idle, total and pool bounds remain unchanged.
3. The egress lease is held by the returned Canonical event source until completion, cancellation,
   error, or drop. A failed start releases both the ordinary Credential lease and egress capacity.
4. A pre-response transport failure is recorded once according to the profile's exact failure
   scope: Endpoint, Endpoint+Credential, or selected egress node. The orchestrator receives a
   closed `compatible egress` failure and must not add a second Endpoint-wide cooldown.
5. `CredentialAndEgress` stickiness is process-local and exact: the first selected node is retained
   for that Credential; an unavailable sticky node fails closed instead of silently rotating.
6. HTTP submission is never retried inside the adapter. Existing Route attempt budgets remain the
   only request-level retry owner, preserving First Semantic Event and Provider isolation.

The default deployment composition remains Direct because proxy-pool membership is not yet a
durable Config-Version field. Synthetic fixed/pool registries may exercise the same handoff without
network access. A later task may add protected configuration, explicit probes, recovery, and
Provider-specific browser/clearance behavior.

## Consequences

- Generic Chat/Responses/Messages adapters use the P13-11 runtime on every serving attempt.
- Native Grok/Kiro adapters remain on their dedicated transport paths.
- A Client request cannot select a proxy, node, failure scope, or retry policy.
- No Store read, DNS lookup, proxy probe, Provider call, or production mutation is introduced by
  runtime composition itself.
- No management OpenAPI or Prism surface changes in P13-11C.

## Rejected alternatives

- **Acquire a second compatible Credential lease.** Rejected because it can double-count capacity
  and diverge from the scheduler's exact selection.
- **Retry `UpstreamClientPool::send` on another proxy.** Rejected because a connection-layer error
  does not prove that the request was never submitted.
- **Keep using the Endpoint-wide direct profile.** Rejected because it leaves P13-11B outside the
  real serving path and cannot isolate egress-node failures.
- **Expose proxy selection in the public request.** Rejected because egress is an operator-owned
  Provider/Endpoint policy, not client input.
