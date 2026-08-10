# ADR-0076 Provider-aware management inventory

| Field | Value |
|---|---|
| Status | Accepted for P13-04A; phase gate pending |
| Date | `2026-08-11` |
| Task / Contract | `P13-04A` / [BC-MGMT-009](../contracts/BC-MGMT-009-provider-aware-operational-inventory.md) |
| Scope | Read-only, Config Version-scoped Provider/Channel/Account/Pool inventory |

## Context

P10 provides protected management CRUD and runtime status primitives, but it does not provide the
typed, provider-aware inventory needed by the CPAMP-like management surface. A frontend should be
able to list configured account bindings, filter them, and page through a stable result without
reading SQLite directly or inferring operational state from unrelated resources.

The existing control-plane graph already gives us the correct ownership boundaries:
`UpstreamConfiguration` is the Provider, `EndpointConfiguration` is a Channel, and an
`EndpointCredentialBindingConfiguration` joins a Credential to a Channel. Native Provider-owned
runtime pools (for example Grok Console/Build accounts) are not represented completely by this
graph and must not be fabricated as configuration rows.

## Decision

Add one read-only operational inventory projection with one row per configured
Endpoint-Credential binding in the caller-selected Config Version. The projection joins only
same-version records and contains the following non-secret fields:

- Provider: `provider_id`, `provider_name`, `provider_kind`, `provider_enabled`, `egress_policy_id`;
- Channel: `channel_id`, `adapter_id`, `api_format`, `transport`, `channel_enabled`;
- Account: `account_id`, `account_kind`, all four persisted `account_status` values
  (`active`, `cooling`, `unauthorized`, `disabled`), and `account_revision`;
- Binding: `binding_enabled`, `priority`, `weight`, `concurrency`;
- Relationships: sorted, de-duplicated `route_ids` derived from route candidates.

The endpoint is `GET /admin/operations/account-pools`. It uses the existing Management Key,
peer/CSRF admission, selected `X-Config-Version`, revision/ETag and audit/security plumbing. It
does not publish a Snapshot, contact a Provider, mutate a Config Version, lease an account, or
claim live Health, Quota, Circuit, usage, or billing state. The `enabled` filter means the static
configuration conjunction `provider_enabled && channel_enabled && binding_enabled`; Credential
lifecycle status remains a separate filter and field.

Results are sorted by the stable key `(provider_id, channel_id, account_id)`. The default page size
is 50 and the hard maximum is 100. A cursor carries the Config Version ID, its revision, and the
last stable key. A cursor from another version or revision is rejected with a safe `409` conflict,
preventing mixed-version pages.

Provider-specific native pools and live account observations are intentionally deferred to
P13-06, where an injected Provider facade can expose them without weakening Provider isolation.

## Consequences

- The management frontend receives a deterministic, typed source for configured pool topology.
- Secret-bearing fields, URLs, request bodies and client-key digests cannot cross this boundary.
- A binding row describes configured eligibility, not whether an account is currently healthy or
  has quota; consumers must use a separate runtime observation contract for those claims.
- The projection is implemented without a migration or Provider network dependency.
- Native runtime pools require a later Provider-specific adapter and cannot be counted here.

## Alternatives considered

- **Expose the SQLite graph directly:** rejected; it couples the frontend to storage and risks
  leaking ciphertext, URLs, or future private columns.
- **Treat every Credential as a pool account:** rejected; an unbound credential has no channel
  scheduling semantics and would misrepresent the configured pool.
- **Merge native Provider runtime pools into this response now:** rejected; it would fabricate
  configuration state, require Provider calls, and blur failure/credential ownership.
- **Use offset pagination:** rejected; mutations can reorder rows and cause duplicates or skipped
  entries. Revision-bound keyset pagination is deterministic and fail-closed.

## Validation and rollback

P13-04A validation must cover deterministic ordering, all filters, bounded page sizes, cursor
continuity and stale-cursor `409`, malformed cross-upstream graph rejection, management
authentication, and absence of secrets/ciphertext/URLs/digests/request bodies. The HTTP/OpenAPI
fixture proves that no Provider transport is invoked. The accompanying report records the
implementation and local review as passing; the formal P13 phase gate remains pending.

Rollback is limited to removing the new read method, route, OpenAPI operation, generated client
operation and projection module. Existing P10 management resources, active configuration,
Provider pools and production listeners remain unchanged.
