# BC-MGMT-009 Provider-aware operational inventory

| Field | Value |
|---|---|
| Contract | `BC-MGMT-009` |
| Task | `P13-04A` |
| Status | Accepted; local implementation and review passed; phase gate pending |
| Domain | Secret-free Config Version management inventory |

## Boundary

`GET /admin/operations/account-pools` is a protected management read. The caller must satisfy the
existing Management Key, peer/origin and CSRF admission rules, and select exactly one Config
Version using the existing management context. The operation reads that version's persisted
control-plane graph only. It never reads plaintext credential material, decrypts an envelope,
publishes a route snapshot, contacts a Provider, leases a credential, or changes configuration.

The first slice emits one item for every valid Endpoint-Credential binding. The source mapping is:

| Projection area | Source | Allowed fields |
|---|---|---|
| Provider | `UpstreamConfiguration` | id, name, kind, enabled, egress policy id |
| Channel | `EndpointConfiguration` | id, adapter id, API format, transport, enabled |
| Account | `CredentialConfiguration` | id, kind, status, revision |
| Pool binding | `EndpointCredentialBindingConfiguration` | enabled, priority, weight, concurrency |
| Routes | `RouteCandidateConfiguration` | candidate route ids for the endpoint, sorted/deduplicated |

Endpoint base URL, inference/catalog paths, tags, capability JSON, encrypted secret, key version,
client-key prefix/digest, request body and any runtime observation are excluded. The four stored
Credential statuses are preserved exactly: `active`, `cooling`, `unauthorized`, and `disabled`.
Transport is projected without normalization (`http`, `sse`, or `websocket`).

## Query and pagination

The query object is closed and supports only:

| Parameter | Meaning |
|---|---|
| `provider_id` | Exact Provider/Upstream ID filter |
| `channel_id` | Exact Channel/Endpoint ID filter |
| `account_status` | Exact persisted Credential status filter |
| `enabled` | Static conjunction of Provider, Channel and Binding enabled bits |
| `limit` | Optional positive page size; default 50, maximum 100 |
| `cursor` | Opaque URL-safe keyset cursor |

Unknown, repeated or malformed parameters are rejected with the existing safe management input
error. Rows are sorted by `(provider_id, channel_id, account_id)`. The server emits at most
`limit` rows and derives `next_cursor` from the last emitted row; it never uses offset pagination
or fabricates a cursor from an unreturned row.

The cursor is bound to `(config_version_id, config_revision, provider_id, channel_id, account_id)`.
It is opaque to clients, has a bounded decoded size, and must be URL-safe. A cursor whose version
or revision differs from the selected Config Version returns `409` and no page. This prevents a
caller from combining pages across a graph mutation or switching Config Versions mid-list.

## Response shape

The response is a closed JSON object with the selected Config Version ID, its revision, an array of
items, and an optional next cursor. Item keys are exactly the fields listed above. The response
uses the normal management revision/ETag projection. Empty results are a successful page with an
empty array, not a Provider or account-pool failure.

`enabled` is a configuration projection only:

```text
provider_enabled && channel_enabled && binding_enabled
```

It does not include Credential status and does not imply runtime availability. Health, Quota,
Circuit, usage, cost, refresh and native Provider-owned account pools require later contracts.

## Invariants and errors

1. Every returned binding resolves its endpoint, credential and upstream in the same Config
   Version; a broken or cross-upstream relationship fails closed rather than being skipped.
2. Ordering and cursor continuation are deterministic for identical `(version, revision, query)`.
3. `limit=0`, negative/overflow limits, malformed status/IDs, unknown fields and overlong cursors
   return a safe `400`-class management error.
4. A stale or cross-version cursor returns a safe `409` conflict.
5. Missing management credentials continue to use the existing concealment response; the endpoint
   does not create a new authentication path.
6. Serialized JSON contains no URL, path, ciphertext, plaintext, digest, header or request body.
7. The implementation has no Provider transport dependency and emits no live Health/Quota claim.

## Required verification

- Unit tests for projection, stable ordering, filters, limit bounds, route-id deduplication,
  malformed graph rejection, cursor encoding/decoding and revision conflicts.
- Protected HTTP tests for authentication, selected Config Version, ETag, filters, pagination,
  stale cursor and value-free response assertions.
- OpenAPI contract and generated-client checks with a closed schema and `x-delivery-phase:
  P13-04`.
- Local review proving no production/server/Provider/GitHub CI activity was required.

## Deferred capabilities

Native Grok Build/Console pools, Provider-specific account health/quota, automatic refresh/reauth,
proxy pools, usage/cost/billing and operator mutations are explicitly outside this contract and
are tracked by P13-05, P13-06, P13-11 and P13-12.
