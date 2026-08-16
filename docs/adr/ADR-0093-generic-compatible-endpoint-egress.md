# ADR-0093: Generic compatible-endpoint egress profiles

Status: **Accepted — P13-11A formally gated as `DONE_WITH_BOUNDARY`**

Date: 2026-08-16

## Context

CPAR accepts more than one credential envelope for an OpenAI-compatible upstream. A direct
official API-key record, a CPA JSON export, a Sub2API JSON export, and an operator-provided
`base_url + api_key` relay are different credential sources, but they are not different egress
architectures. A relay called Krill is one configured upstream instance, not a privileged global
provider or a reason to share state with another relay.

The existing `EndpointUrl` and `EgressPolicy` already own URL shape, SSRF, DNS pinning, and
redirect re-admission. What is missing is a typed, Provider-neutral description of the exact
endpoint/credential binding and its egress behavior. Without that description, a later proxy-pool
implementation could accidentally make relay names select fallback behavior, mix credential and
egress health, or replay a request after it has been submitted.

## Decision

P13-11A adds a local `gateway-upstream` foundation, `CompatibleEndpointEgressProfile`, with these
closed properties:

1. The profile carries one exact `UpstreamId`, `EndpointId`, `CredentialId`, non-secret credential
   source label, wire protocol, and version-scoped `EgressPolicyId`. It has no URL, API key,
   OAuth/SSO token, cookie, proxy credential, request body, or Provider response.
2. The source label is metadata only. `api_key`, `cpa_json`, `sub2api_json`, and direct OAuth do
   not select different transport logic. Adapter identity and protocol conversion remain owned by
   the existing protocol/Provider composition layers.
3. The target is exactly one of `Direct`, one named `FixedProxy`, or one named `ProxyPool`. A
   profile contains no fallback list and cannot implicitly borrow another Provider, channel,
   credential, or egress pool.
4. Failure ownership is explicit: `Endpoint`, exact `Credential`, or selected `EgressNode`.
   Account-only and account-plus-egress stickiness are explicit values. Direct transport cannot
   claim an egress-node failure or account-plus-egress stickiness.
5. Retry policy is either `None` or a bounded `PreSubmit` total-attempt budget of 1..=3. There is
   no post-submit replay value in this contract; Provider-specific adapters may narrow the policy
   further.
6. Construction checks bounded identities and exact endpoint/credential upstream ownership. URL
   composition and static policy checks delegate to `EndpointUrl` and `EgressPolicy`; actual DNS,
   proxy selection, lease acquisition, Health/Quota mutation, and Provider I/O remain outside this
   slice.

The profile is intentionally not persisted and does not add a management/OpenAPI surface. P13-11B
will compose it with the selected Config Version and existing runtime pools, and P13-11C can add
Provider-specific sticky-node health/probe behavior. Grok Web clearance/browser recovery remains a
separate capability boundary and is not inferred from a generic compatible endpoint profile.

## Alternatives considered

* **Special-case Krill.** Rejected: the endpoint name must not change credential, protocol, proxy,
  or failure semantics.
* **Treat every CPA/Sub2API JSON as a proxy pool.** Rejected: credential serialization and egress
  selection are separate concerns; a JSON envelope does not authorize a proxy or fallback.
* **Reuse one global proxy pool for every Provider.** Rejected: egress health, account stickiness,
  clearance, and failure ownership are Provider/channel-specific.
* **Put the profile in the public management API first.** Deferred: P13-11A first proves the
  local type and security invariants; management mutation/read surfaces require a separate contract
  and Claude Code handoff if they become part of the next slice.

## Consequences

Generic relay channels can be added without a Krill-specific branch. The same exact binding can
later be used with direct, fixed-proxy, or pool-backed egress while retaining the existing SSRF
admission boundary. The profile does not itself prove that a proxy node works, that a credential is
authorized, or that Grok Web has a valid browser session; those remain runtime evidence tasks.

P13-11A has no Provider, server, production, staging, OAuth, Autoreg, or network side effects.

## Formal phase acceptance

This decision was accepted in the aggregate P13-11 phase Gate:
`phase-p13-egress-complete` at `a716eaaa9d31c26b6d09489f3f7fdbb9b0e1ebeb`, formal run
[31959162202](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31959162202). Authorize,
Fast, Full supply-chain and Required all passed. The acceptance does not authorize real Provider
traffic, proxy probes, staging/production or Provider-specific recovery.
