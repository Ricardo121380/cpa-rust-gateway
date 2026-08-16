# BC-SEC-006: Compatible-endpoint runtime egress composition

Status: `DONE_WITH_BOUNDARY`

## Scope

This contract defines the local P13-11B composition of a generic compatible endpoint profile with
an active Config Version, existing Credential pools, shared runtime Health/Quota, and a bounded
transport-node registry. It applies equally to CPA JSON, Sub2API JSON, direct OAuth/API-key input,
Krill, and any custom compatible relay. The source label never selects a different Provider or
proxy behavior.

## Required invariants

* Every composition has one exact `SnapshotVersion`, Upstream, Endpoint, protocol and
  `EgressPolicy`; mixed endpoint/upstream/protocol/policy bindings are rejected before publication.
* Credential priority, weight, and concurrency are delegated to the existing
  `EndpointCredentialPools`; no second cursor or scheduler is created.
* A candidate is eligible only when Endpoint Health, exact Credential Health/account status,
  optional model Health, binding/model Quota, expiry, Credential capacity and egress-node
  availability all pass at the same observed timestamp.
* One successful request lease owns both the exact Credential lease and the selected egress-node
  capacity. Dropping either request path releases both; a failed second acquisition releases the
  first before returning.
* Egress node state is separate from Credential Health/Quota. Cooling/disabled one node does not
  mark the Credential unauthorized or make another Upstream eligible.
* Every transport registry is bound to exactly one Upstream identity. A registry whose owner does
  not match the runtime profile is rejected before publication, so node capacity/cooldown cannot
  cross Provider boundaries.
* `Direct` uses the existing Direct transport profile. `FixedProxy` and `ProxyPool` accept only
  validated local-DNS `socks5` profiles in this slice; HTTP/HTTPS remote-DNS proxy URLs fail
  closed.
* The compiler accepts only the generic compatible adapter families and only an active,
  caller-selected Config Version. The supplied EgressPolicy snapshot must equal that graph, and
  every existing Credential pool entry must match the binding's revision, priority, weight and
  concurrency before publication. It does not read Store after construction, mutate a Config
  Version, or contact a Provider.
* IDs and labels are bounded; observations, Debug and errors contain only opaque IDs, enum states,
  counts, timestamps and safe revisions. URL/key/token/cookie/header/body/Provider response values
  are never retained in a public projection.

## Explicit non-goals

P13-11B does not implement HTTP/HTTPS remote-DNS proxy admission, Provider-native proxy pools,
Grok Web browser clearance, FlareSolverr, automatic reauth/refresh, Autoreg registration, serving
request-path transport switching, or real Provider/egress probes. Those require later
Provider-specific contracts and explicit evidence.

## Verification target

The local slice must prove direct/fixed/pool target resolution, bounded weighted node selection,
node cooldown/disabled/capacity isolation, exact Credential lease and release, expiry/Health/Quota
fail-closed behavior, mixed/foreign active-config rejection, credential-kind drift rejection, and
secret-free observations without Store, Provider or network I/O.

## Formal acceptance

This contract is accepted as P13-11B within the aggregate formal Gate:
`phase-p13-egress-complete` at `a716eaaa9d31c26b6d09489f3f7fdbb9b0e1ebeb`, run
[31959162202](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31959162202). All four
jobs passed. The contract does not authorize real proxy probes or Provider/staging/production
traffic.
