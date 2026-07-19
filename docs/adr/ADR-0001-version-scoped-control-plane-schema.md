# ADR-0001: Version-scoped control-plane SQLite schema

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-19` |
| Task / Matrix / Contract references | `P2-01`; `L01-L05`, `E04`, `E08`, `E20`, `E23`, `E29`, `J03`, `J18-J20`; [BC-STORE-001](../contracts/BC-STORE-001-versioned-control-plane-schema.md) |

## Context

The aggregation control plane needs durable configuration versions before it can compile and
publish a `RouteSnapshot`. A configuration version must never be able to combine an Endpoint,
Credential, or Binding from another version. SQLite is the locked single-node control-plane store,
but it must not enter the inference request hot path.

## Decision

- `gateway-store` owns versioned SQLite migrations using `rusqlite` with its bundled SQLite build.
  This keeps the schema reproducible on the Mac and Linux CI without depending on a host SQLite
  development package.
- `config_versions` is the configuration root. `upstreams`, `upstream_endpoints`,
  `upstream_credentials`, and `endpoint_credential_bindings` use `(config_version_id, id)` keys.
  Composite foreign keys make every Endpoint and Credential belong to an Upstream in the same
  configuration version.
- A Binding stores its common `upstream_id` and references both Endpoint and Credential through
  `(config_version_id, id, upstream_id)`. This makes a cross-Upstream Binding an SQLite foreign
  key violation instead of a later application-level surprise.
- A Config Version is `draft`, `active`, or `archived`; a partial unique index permits at most one
  `active` version. Publishing, validation, cloning, rollback policy, and Snapshot construction
  remain P2-05 through P2-10 work.
- Credential ciphertext is an opaque non-empty BLOB with a positive key version. P2-01 does not
  encrypt, decrypt, log, or interpret it; AEAD and master-key loading are exclusively P2-03.
- In the version-1 migration, `upstreams.egress_policy_id` is a nullable opaque reference. P2-09
  later adds the version-scoped `egress_policies` table and same-version write enforcement in
  migration 3; its URL/DNS/CIDR behavior is defined by ADR-0009 rather than retroactively by this
  schema decision.
- Migrations are transactional, recorded by an internal `schema_migrations` table, and have an
  explicit down path. Foreign-key enforcement is enabled and verified on every Store connection.

## Consequences

Version cloning can retain logical IDs while storing independent rows in separate configuration
versions. The version-1 schema intentionally contains no Repository, management API, route
compiler, snapshot, secret cryptography, Client Key, Public Model, Candidate, or EgressPolicy
behavior. Those concerns add their own migrations and contracts in their assigned P2 Tasks; in
particular migration 3 supersedes the former opaque EgressPolicy reference.

## Alternatives considered

- One global row per Upstream/Endpoint/Credential with a Config Version join table was rejected:
  its revisions would be harder to keep atomically isolated and it would make version rollback
  ambiguous.
- Application-only checks for version and Upstream consistency were rejected: SQLite can enforce
  these relationships at write time with composite foreign keys.
- A host-linked SQLite library was rejected for this first migration because it weakens clean Mac
  and CI reproducibility.

## Validation and rollback

The P2-01 tests prove a valid tree can be inserted, each relationship is foreign-key enforced,
and `migrate` followed by `rollback_all` returns a new database to its original user-table set.
The down migration is only a development/schema rollback mechanism at this stage; production
backup and downgrade rehearsal remain P11-07 work.
