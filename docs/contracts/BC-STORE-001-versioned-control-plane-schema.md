# BC-STORE-001 Versioned control-plane schema

| Field | Value |
|---|---|
| Contract | `BC-STORE-001` |
| Task | `P2-01` |
| Status | DONE |
| Domain | Versioned aggregation control-plane persistence |

## Entry and boundary

`gateway-store` opens a SQLite control-plane connection, enables foreign keys, and applies the
ordered schema migrations before a future management service reads or writes configuration.
The inference request hot path never receives a Store connection or queries these tables; later
P2 work compiles persisted configuration into an immutable `RouteSnapshot`.

## Data shape

| Table | Identity and required relationship | P2-01 responsibility |
|---|---|---|
| `config_versions` | global `id`; optional parent version | draft/active/archived configuration root |
| `upstreams` | `(config_version_id, id)`; belongs to one Config Version | name, kind, enabled, tags, nullable EgressPolicy reference in migration 1 |
| `upstream_endpoints` | `(config_version_id, id)`; belongs to one same-version Upstream | adapter, API format, base URL, paths, transport, enabled |
| `upstream_credentials` | `(config_version_id, id)`; belongs to one same-version Upstream | opaque ciphertext, key version, status, revision |
| `endpoint_credential_bindings` | `(config_version_id, endpoint_id, credential_id)` | Endpoint and Credential must have the same Upstream; enabled, priority, weight, concurrency |

Rows deliberately use version-scoped composite keys, so a Config Version cannot accidentally
reference a row from another version. Config Version activation, cloning, and publication are
not implemented by this contract.

Migration 3 later adds `egress_policies` and turns a non-null Upstream reference into a
same-version checked relation; its semantic URL, DNS, CIDR, and redirect rules are owned by
[BC-SEC-002](BC-SEC-002-egress-policy-ssrf-admission.md), not this historical P2-01 contract.

## Preconditions

- The caller uses the Store migration entry point before application repositories are introduced.
- SQLite foreign keys are enabled on the connection. The Store verifies this rather than relying
  on SQLite's per-connection default.
- IDs and configured text fields are non-empty bounded values. `tags_json` is a JSON array.
  P2-01 itself deferred URL syntax and EgressPolicy admission; migration 3 and BC-SEC-002 now
  define them for current schema versions.

## Invariants

- A Version has at most one `active` row across the database. It may have an optional distinct
  parent Version.
- Every Endpoint and Credential references an existing Upstream with the same
  `config_version_id`.
- A Binding cannot cross Config Versions or Upstreams. The database rejects it through composite
  foreign keys, even if both referenced IDs individually exist.
- `enabled` values are booleans; credential status, positive key version, non-negative revision,
  positive Binding weight/concurrency, and non-negative Binding priority are schema constraints.
- The ciphertext column is opaque binary data. P2-01 neither accepts plaintext Secret semantics
  nor exposes a read/write Repository API; P2-03 defines AEAD and key management.
- In version 1, `egress_policy_id` is a nullable opaque reference. Migration 3 adds the
  EgressPolicy entity and enforces non-null same-version references; no P2-01 code itself
  performed URL, DNS, CIDR, proxy, redirect, or SSRF work.

## Migration and error semantics

```text
empty database
  -> enable and verify SQLite foreign keys
  -> transactional migration up
  -> schema_migrations records version 1
  -> valid same-version tree is writable

version 1 database
  -> transactional rollback_all
  -> remove P2-01 tables and migration bookkeeping
  -> original user-table set restored
```

A foreign-key violation is a write failure; it never silently creates an orphan or cross-Upstream
Binding. A database with an unsupported migration sequence is rejected rather than guessed at.
The migration runner does not activate a Config Version, compile a route, or publish a Snapshot.

## Corresponding tests

- Migration `0001` records schema version 1 and creates exactly the five P2-01 control-plane
  tables; later migrations preserve that historical version number.
- A valid Config Version → Upstream → Endpoint/Credential → Binding tree succeeds.
- Missing Version/Upstream relationships and a cross-Upstream Binding each fail through SQLite
  foreign-key enforcement.
- Migration up followed by rollback restores the initial user-table inventory and no migration
  bookkeeping remains.
