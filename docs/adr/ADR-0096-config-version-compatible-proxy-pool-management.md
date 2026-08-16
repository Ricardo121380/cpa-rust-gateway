# ADR-0096: Config-Version-owned compatible proxy-pool management

Status: Proposed — P13-11D1 implementation in progress

Date: 2026-08-17

## Context

P13-11A through P13-11C established a provider-neutral egress profile, an Upstream-owned
`CompatibleEgressTransportRegistry`, and a serving handoff that holds one exact Credential lease
plus one exact egress lease. Those slices deliberately kept persistence out of scope. The current
deployment therefore builds one Direct registry per generic compatible Upstream and supplies no
fixed proxies, no proxy pools, and no non-default binding settings.

That default is safe but not operable. An operator cannot yet describe a durable proxy pool in a
draft Config Version, review it through the protected management plane, publish or roll it back
with the rest of the graph, or reconstruct the same pool after restart. Adding an unversioned
global proxy file would break Config Version rollback and could silently share an exit node across
Providers. Reading a local Clash or environment proxy at request time would also bypass the
existing egress, audit, and secret boundaries.

## Decision

P13-11D introduces Config-Version-owned compatible proxy configuration in three reviewable
slices. D1 owns persistence and AEAD, D2 owns protected management HTTP/OpenAPI, and D3 owns
active runtime composition. A/B/C remain immutably accepted under
`phase-p13-egress-complete`; D is not retroactively added to that tag.

### Durable resources

One Config Version may own these resources:

1. A **compatible proxy pool** has an opaque pool ID, exact owning Upstream ID, bounded display
   name, and administrative enabled bit.
2. A **compatible proxy node** has an opaque node ID, exact owning Upstream ID, an optional pool
   ID, bounded display name, enabled bit, selection weight, maximum concurrency, and one encrypted
   local-DNS SOCKS5 endpoint. A node with no pool is a standalone fixed proxy. A node with a pool
   belongs to exactly that pool and cannot be reused by another pool or Upstream.
3. A **compatible binding egress profile** belongs to one existing exact
   Endpoint-Credential binding. It selects `direct`, one standalone node, or one pool and carries
   the closed failure scope, stickiness, and bounded pre-submit retry settings already defined by
   P13-11A.

The model intentionally does not derive behavior from a relay name or credential format. A CPA
JSON export, Sub2API JSON export, direct OAuth account, plain API key, and custom relay all use the
same explicit resources.

### Proxy endpoint secrecy and admission

The proxy endpoint is sensitive operational configuration even when it contains no credential.
It is therefore sealed by the existing external-Master-Key `SecretStore` under a separate domain.
Associated data binds at least the schema version, Config Version ID, Upstream ID, optional pool
ID, and node ID so moving ciphertext between rows fails authentication.

Before sealing, the management service parses the supplied value through the existing
`UpstreamProxy::try_socks5` boundary. The first version accepts only
`socks5://host:explicit-port` with local upstream DNS pinning. It rejects `socks5h`, HTTP/HTTPS
proxy schemes, user-info, query, fragment, and non-root paths. Proxy authentication, remote-DNS
proxying, browser clearance, and FlareSolverr are not smuggled into this generic contract.

Ciphertext, key version, proxy URL/host/port, and any future authentication material are never
returned by list/get responses or included in `Debug`, audit events, error details, logs, or
runtime observations. A management response may report only `proxy_configured: true`.

### Ownership and configuration lifecycle

Pools and nodes are always scoped to one exact Upstream. A pool member must have the same owning
Upstream as its pool. A fixed target must reference a standalone node from the binding's Upstream;
a pool target must reference a pool from the same Upstream. The binding profile must match an
existing Endpoint-Credential binding whose Endpoint and Credential already share that Upstream.

All mutations are draft-only and use the existing exact `If-Match` Config revision. A resource
write, revision increment, and value-free management audit event commit in one immediate SQLite
transaction or all roll back. Config clone, publish, archive, rollback, backup, and restore treat
the new rows as part of the same graph. Deleting an owning binding, Upstream, pool, or Config
Version cannot leave a cross-version or dangling target.

### Runtime composition

D3 will decrypt and validate the active Config Version once while composing the data plane. It
will build the existing `CompatibleFixedProxyInput`, `CompatibleProxyPoolInput`, and
`CompatibleEndpointBindingRuntimeSettings` values. Serving will keep using the P13-11B/C
registry and lease path; it will not read SQLite, decrypt a node, infer a proxy from a client
request, or create a second scheduler.

No configured compatible resources means the same Direct-only default as today. Malformed
ciphertext, a missing key, unsafe SOCKS5 endpoint, empty enabled pool, cross-Upstream reference,
unknown target, duplicate identity, weight/capacity overflow, or Config Version drift fails closed
before publication. It must not prevent an already-valid incumbent process from continuing to
serve its pinned graph.

### Protected management boundary

D2 will expose typed resource operations only under the existing protected `/admin` scope. Reads
require Management Key and `X-Config-Version`; writes additionally require same-origin CSRF when
an Origin is present and exact `If-Match`. The public inference API does not accept pool IDs,
node IDs, or proxy values.

The authoritative source is `docs/openapi/management-v1.json`. When D2 changes it, the same
change must run `npm --prefix web/prism run sync-contract` and append a precise handoff to
`docs/cross-boundary-log.md`. Claude Code may implement a management control using the generated
client but must never hand-edit generated files or display proxy endpoint material.

## Consequences

- A published Config Version can reproduce and roll back its complete generic compatible egress
  graph instead of depending on ambient machine configuration.
- Proxy node state remains separate from Credential Health/Quota/Circuit and cannot cross an
  Upstream ownership boundary.
- Proxy endpoint material gains the same external-key, authenticated-encryption, corruption, and
  rotation discipline as other secrets.
- The schema and management surface become larger; D is therefore split so Store/AEAD,
  management contract, and serving composition can each be reviewed independently.
- Generic persistence does not make Grok Web clearance, Console bootstrap, Kiro, or other native
  adapter egress automatically compatible. Provider-specific capabilities remain explicit.

## Rejected alternatives

### Use system, environment, Clash, or deployment proxy settings

Rejected because they are not Config-Version-owned, cannot be safely reviewed or rolled back,
and can silently affect unrelated Providers.

### Store a raw proxy URL in `egress_policies`

Rejected because `EgressPolicy` is the endpoint URL/SSRF admission contract, not a transport-node
secret or capacity pool. Combining them would also expose proxy topology through existing safe
policy reads.

### Share one global pool across Providers

Rejected because availability, capacity, sticky state, failure attribution, and operational trust
belong to the owning Upstream/Provider instance.

### Let clients select a pool or node

Rejected because it would turn an internal operator policy into a public routing bypass and allow
tenants to steer traffic across egress trust boundaries.

### Add proxy authentication or remote-DNS SOCKS in D1

Rejected for the first slice. The existing transport proves only local-DNS SOCKS5 address pinning.
Broader schemes require a separate security review and explicit transport support.

## Verification

P13-11D cannot be accepted until the corresponding contract and report prove:

- migration up/down, graph clone/rollback/reopen, foreign-key and atomic revision behavior;
- AEAD associated-data row-swap rejection, missing/wrong key and corruption failure, rotation, and
  secret-free diagnostics;
- exact Upstream/Endpoint/Credential/pool/node ownership and all bounded enum/weight/capacity/retry
  validation;
- protected management auth/CSRF/version/revision/audit and response redaction after D2;
- active Config compilation into the existing registry with default Direct behavior, no Store on
  the request path, and no cross-Provider fallback after D3;
- focused tests, strict Clippy, formatting, docs, secret scan, diff review, and one aggregate P13
  Delivery Gate only at the D closeout boundary.

No local deterministic test is evidence that a real proxy, DNS route, Provider account,
FlareSolverr flow, server, staging, or production traffic works.
