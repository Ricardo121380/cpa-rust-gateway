# ADR-0094: Generic compatible-endpoint runtime egress composition

Status: **Accepted — P13-11B formally gated as `DONE_WITH_BOUNDARY`**

Date: 2026-08-16

## Context

P13-11A established that a CPA export, a Sub2API export, a direct OAuth/API-key record, a relay
called Krill, and an operator's custom `base_url + api_key` are all one compatible-endpoint
configuration pattern. The profile alone is intentionally inert. A runtime must still combine it
with the active Config Version, the existing Endpoint Credential pool, the shared Health/Quota
registries, and a transport target without creating a second scheduler or a global proxy fallback.

The repository already has the safe transport primitive: `EgressPolicy` performs URL/SSRF
admission and `UpstreamTransportProfile` supports direct transport and local-DNS `socks5` while
preserving the admitted address set. It deliberately does not admit HTTP/HTTPS remote-DNS proxy
schemes. The runtime composition must preserve that boundary rather than bypassing it for a
generic relay.

## Decision

P13-11B adds a router-owned, provider-neutral runtime composition with four properties:

1. One composition is bound to one `SnapshotVersion`, one exact Upstream/Endpoint/Protocol and one
   version-scoped `EgressPolicy`. Its profiles are compiled from a caller-selected active graph;
   the resulting object never reads SQLite or publishes a new snapshot.
2. Credential selection remains owned by the existing `EndpointCredentialPools` priority/weight
   scheduler. Before a lease, the composition checks Endpoint and exact Credential/Model Health,
   account status, binding/model Quota, expiry, capacity, and the selected egress target. It then
   acquires one Credential lease and one egress-node lease. Both are released by RAII on every
   return, drop, cancellation, or driver failure.
3. Direct targets use the existing direct `UpstreamTransportProfile`. Fixed proxy and proxy-pool
   targets use only validated local-DNS `socks5` transport profiles. A bounded registry tracks
   node capacity and local cooldown/disabled observations independently from Credential Health and
   Quota. Every registry carries one owning Upstream identity and is rejected if reused for another
   Upstream/Provider; there is no cross-Upstream/Provider pool or implicit fallback.
4. The composition and its observations are value-free: no URL, API key, OAuth/SSO token, Cookie,
   proxy secret, request body, or Provider response is rendered in Debug/errors/observations.
   `EndpointUrl` is retained only inside the execution object and every real dial must still call
   `EgressPolicy::admit_url` for per-attempt DNS/address admission.

The active-config compiler only emits generic compatible adapters (`openai-compatible` Chat or
Responses and `anthropic-compatible.messages`). Grok Build/Console/Web, Kiro, official Codex
OAuth, browser clearance, and Provider-specific probes remain owned by their existing adapters or
the later P13-11C boundary. Autoreg registration and credential refresh remain outside CPAR.
Before publication it also verifies that the graph is active, the supplied EgressPolicy snapshot
matches the graph, and every pool entry carries the expected Credential revision and scheduling
metadata; stale composition inputs fail closed.

## Alternatives considered

* **Create a second credential scheduler for proxy-aware requests.** Rejected: it would desync
  priority/weight/capacity and let a diagnostic path perturb ordinary serving.
* **Treat a CPA/Sub2API/Krill label as a proxy selector.** Rejected: credential serialization and
  egress target are independent configuration dimensions.
* **Allow HTTP/HTTPS proxy URLs because other relay projects accept them.** Rejected for this
  slice: the existing SSRF/address-pinning contract can prove local-DNS SOCKS5 only; remote-DNS
  proxy schemes need their own explicit admission contract.
* **Use one global proxy pool across Providers.** Rejected: node health, account stickiness,
  clearance and failure ownership are scoped to the exact endpoint/provider binding.

## Consequences and boundary

P13-11B proves deterministic local composition, exact lease ownership, and health/egress isolation;
it does not make a proxy node reachable, authorize an account, or prove a Provider response. The
serving adapter request-time transport switch, Provider-specific sticky/probe/failure feedback,
and any real egress probe belong to P13-11C or a separately authorized Provider task.

## Formal phase acceptance

This decision was accepted in the aggregate P13-11 phase Gate:
`phase-p13-egress-complete` at `a716eaaa9d31c26b6d09489f3f7fdbb9b0e1ebeb`, formal run
[31959162202](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31959162202). Authorize,
Fast, Full supply-chain and Required all passed. The acceptance does not authorize real Provider
traffic, proxy probes, staging/production or the later protected management surface.
