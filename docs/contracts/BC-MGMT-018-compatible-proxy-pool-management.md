# BC-MGMT-018: Compatible proxy-pool persistence and protected management

Status: `P13-11D2 LOCAL_PASS_PENDING_PHASE_GATE`; D1 and D2 locally implemented; D3 not started

## Purpose

Define the fail-closed backend contract by which a Config Version owns generic compatible proxy
pools, proxy nodes, and exact Endpoint-Credential egress profiles. This contract extends the
P13-11A/B/C runtime foundation; it does not replace the existing EgressPolicy, Credential pool,
Health/Quota, or Provider-specific adapter contracts.

## Resource model

### Compatible proxy pool

Each pool has exactly these non-secret fields:

- `pool_id`: opaque, trimmed, 1..=128 UTF-8 bytes;
- `upstream_id`: an existing same-Version Upstream;
- `name`: trimmed, 1..=256 UTF-8 bytes and unique within the owning Upstream;
- `enabled`: boolean.

An enabled pool must contain at least one enabled node before an active runtime graph can compile.
The Store may retain an empty or disabled pool in a draft so an operator can build it atomically
across several revisioned writes; publication/runtime compilation still fails closed.

### Compatible proxy node

Each node has:

- `node_id`: opaque, trimmed, 1..=128 UTF-8 bytes;
- exact `upstream_id`;
- optional `pool_id` from the same Upstream; absence means a standalone fixed proxy;
- `name`: trimmed, 1..=256 UTF-8 bytes;
- `enabled`: boolean;
- `weight`: integer 1..=1024; a standalone node must use `1`;
- `maximum_concurrency`: integer 1..=100000;
- one non-empty AEAD ciphertext and positive key version for a validated local-DNS SOCKS5
  endpoint.

A node belongs to no more than one pool. The same node identity or encrypted endpoint row cannot
be reused to manufacture independent capacity across pools. Runtime aggregate pool/node and
schedule limits remain the bounds already enforced by P13-11B.

### Exact binding egress profile

Each profile is keyed by one existing `(config_version_id, endpoint_id, credential_id)` binding.
Its Endpoint and Credential already share an Upstream. It contains:

- `target`: `direct`, `fixed_proxy`, or `proxy_pool`;
- `target_id`: absent for `direct`; the same-Upstream standalone `node_id` for `fixed_proxy`; the
  same-Upstream `pool_id` for `proxy_pool`;
- `failure_scope`: `endpoint`, `credential`, or `egress_node`;
- `stickiness`: `none`, `credential`, or `credential_and_egress`;
- `pre_submit_max_attempts`: integer 1..=3, where `1` means no adapter replay.

`egress_node` failure scope and `credential_and_egress` stickiness require a proxy target. A
direct target cannot carry a target ID. A fixed or pool target must carry one. No record contains
a fallback Upstream, alternate Credential, or client-selectable target.

## Secret and AEAD contract

The only accepted endpoint form in P13-11D is `socks5://host:explicit-port`, validated by the
existing local-DNS SOCKS5 parser before sealing. `socks5h`, HTTP/HTTPS, user-info, query,
fragment, and non-root paths are rejected without retaining their raw values in an error.

The plaintext endpoint is sealed immediately with the externally supplied `SecretStore`. The AAD
domain is versioned and length-prefix binds:

1. domain `cpar-compatible-egress-node-v1`;
2. Config Version ID;
3. Upstream ID;
4. optional pool ID using an explicit absent/present marker;
5. node ID.

Opening a row under another Version, Upstream, pool, or node must fail. Ciphertext, key version,
proxy URL/host/port, and plaintext never appear in `Debug`, errors, audit events, response bodies,
runtime observations, or logs. Management reads expose only `proxy_configured: true` for a valid
stored node.

## Persistence and revision contract

- All resources are deleted with their owning Config Version.
- Pool/node/binding references never cross Config Versions.
- All writes require a draft Config Version and exact expected revision.
- Resource mutation, Config revision increment, and one value-free resource audit event share one
  immediate SQLite transaction.
- Stale revision, active/archived Version, missing owner, foreign Upstream, duplicate identity,
  target mismatch, invalid enum/bound, encryption failure, or audit failure leaves all rows and
  revision unchanged.
- Clone, publish, archive, rollback, backup, restore, reopen, and migration down/up preserve the
  graph or fail closed; there is no partial proxy graph publication.
- Deleting a pool that still owns nodes or a node/pool referenced by a binding profile is rejected
  until the dependent draft resources are changed explicitly. No implicit fallback to Direct is
  created by deletion.

## Protected management contract (P13-11D2)

All routes live under the existing protected management scope. Reads require a valid Management
Key, admitted peer/origin, and exactly one `X-Config-Version`. Writes additionally require the
existing CSRF rule and canonical `If-Match: rev-N` token. Responses use `Cache-Control: no-store`.

The authoritative OpenAPI must expose closed schemas for pool, node, and binding profile
resources. It may accept a plaintext `proxy_url` only on create/rotate operations; it must never
return that value. Update without a new proxy value preserves the existing encrypted endpoint.
Explicit rotation replaces it atomically. Unknown fields, overlong values, duplicate query/body
keys, and unbounded collections are rejected.

Every successful write returns the next Config revision and records a value-free action such as
`compatible_proxy_pool_created`, `compatible_proxy_node_updated`, or
`compatible_egress_binding_updated`. Audit resource IDs are opaque identities only and never
contain a URL, proxy address, Credential secret, request body, or ciphertext.
For the composite Endpoint-Credential binding key, the implementation uses a bounded,
domain-separated opaque projection rather than concatenating arbitrarily long IDs into the audit
row; the projection is not returned by the public data plane and does not contain either source ID.

If the authoritative OpenAPI changes, the same backend change must run
`npm --prefix web/prism run sync-contract` and append an action-required entry to
`docs/cross-boundary-log.md`. Frontend code may display safe fields and invoke generated methods;
it must not store or redisplay the submitted proxy URL.

## Runtime composition contract (P13-11D3)

- Only the active Config Version used by the data-plane snapshot may compile.
- The composition opens each enabled node once, validates its AAD and local-DNS SOCKS5 form, and
  builds the existing P13-11B fixed/pool inputs and exact binding settings.
- Pool and node identity, ownership, enabled state, weight, capacity, and schedule bounds are
  revalidated before publication.
- Missing or invalid encrypted material, an enabled empty pool, target drift, owner mismatch,
  unknown target, duplicate node, or runtime bound overflow rejects the new composition.
- No compatible resources preserves the current Direct-only default.
- The request hot path performs no Store read, decryption, implicit environment proxy lookup, or
  client-directed target selection.
- Serving remains under the existing exact Credential lease, egress lease, Health/Quota, failure
  feedback, first-semantic-event, and no-cross-Provider-fallback rules.

## Required verification

### D1 Store/AEAD

- migration up/down and exact schema table list;
- valid graph round-trip, deterministic load order, clone/rollback/reopen;
- draft-only revision atomicity and rollback on every invalid owner/target/audit case;
- pool/node/binding cascade/restrict semantics;
- encryption/open/rotation, missing key, corruption, wrong AAD, and row-swap rejection;
- no raw proxy or ciphertext in safe projections, Debug, errors, or audit.

### D2 HTTP/OpenAPI — locally implemented

- Management Key, peer/origin, CSRF, Config Version, `If-Match`, stale revision, active/archived
  rejection, unknown/foreign owner, limits, closed enum, and no-store behavior;
- create/read/update/rotate/delete round trips with responses and audit free of proxy material;
- authoritative OpenAPI reference/closed-schema tests, generated client freshness, Prism sync, and
  cross-boundary handoff.

The current local HTTP fixture covers protected create/list/update/delete round trips, stale
revision rejection, endpoint-value preservation when an update omits the write-only field, and
secret-free responses. Successful revisioned management responses carry `Cache-Control: no-store`.

### D3 runtime

- standalone fixed and weighted pool composition for the exact Upstream;
- disabled/empty/cross-Upstream/unknown/corrupt configurations fail before publication;
- default Direct behavior and existing JSON/SSE timeout/profile preservation;
- exact lease/capacity/drop, sticky-node fail-closed, Health/Quota and failure-scope isolation;
- restart/rollback reconstructs the selected Config Version and hot path performs no Store read;
- no Provider, real proxy, DNS, Autoreg, server, staging, or production traffic is required for
  local acceptance.

## Non-goals

- registering, logging in, refreshing, or replenishing accounts;
- Grok Web clearance, FlareSolverr, Console bootstrap, Kiro, or native Provider retry behavior;
- proxy authentication, HTTP/HTTPS proxy, `socks5h`, remote-DNS resolution, subscriptions, Clash
  conversion, or importing a local machine's proxy configuration;
- public client pool/node selection, cross-Provider fallback, production rollout, or a claim that
  a real proxy works.
