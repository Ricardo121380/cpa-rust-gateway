# ADR-0010: Local management lifecycle and durable publication audit

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-19` |
| Task / Matrix / Contract references | `P2-10`; `H05`, `J03`, `J18-J20`; Behavior 20; [BC-CONTROL-002](../contracts/BC-CONTROL-002-local-management-lifecycle.md) |

## Context

P2-05 through P2-09 provide typed version-scoped persistence, semantic compilation, immutable
`RouteSnapshot` publication, Client Key views, and EgressPolicy admission. They intentionally do
not yet provide a single operational entrypoint that creates a draft, proves it is publishable,
activates it, rolls it back, and leaves auditable evidence. P2-07 also retains rollback state only
in memory, so a process restart would otherwise lose the predecessor needed by a local CLI.

The locked plan requires only a minimal management API/CLI in P2. P10 still owns remote HTTP or
OpenAPI, management authentication/authorization, CORS/CSRF, full entity CRUD, query UI, and a
Web audit-log surface. P3 must not begin while this control-plane boundary is implemented.

## Decision

- `gateway-control::ManagementService` is a transport-neutral, management-only facade. It owns a
  `SqliteControlPlaneRepository`, a `SnapshotPublicationService`, bounded non-secret actor label,
  and an injectable management clock. Its typed API creates a complete draft configuration,
  validates a Version, publishes it, rolls it back, and lists durable audit events.
- Migration 4 adds append-only `management_audit_events`. Each row carries only its monotonic ID,
  action (`config_created`, `config_published`, or `config_rolled_back`), bounded actor label,
  timestamp, target Config Version ID, and optionally replaced active Version ID. It contains no
  plaintext Secret, encrypted envelope, Client Key, HMAC digest, URL, request body, or error text.
  SQLite triggers reject ordinary update and delete operations, so the durable predecessor evidence
  cannot be silently rewritten through the Repository connection.
- Draft creation writes the complete graph and its `config_created` event in one SQLite transaction.
  Publishing or rolling back prepares the complete immutable Snapshot first, then atomically
  activates the target Version and records the corresponding audit event in one SQLite transaction,
  and only then commits the infallible `ArcSwap` transition. Audit insertion failure therefore
  cannot yield an unlogged active configuration or a partially changed runtime Snapshot.
- Startup recompiles the persisted active Version and, when the latest matching audit event names a
  valid archived predecessor, recompiles that predecessor into the registry's one-step rollback
  slot. With no active Version, startup uses an empty synthetic Snapshot that exposes no models,
  Routes, Access Groups, or Client Keys. The first publish cannot roll back to that non-persisted
  sentinel; a later persisted publish restores ordinary one-step rollback.
- `apps/gateway` exposes only a local `gateway admin` CLI for `create`, `validate`, `publish`,
  `rollback`, and `audit`. Its default injected Catalog/capability views are empty, so it safely
  supports draft scaffolding and does not invent Route eligibility evidence. Embedders with real
  immutable Catalog/capability views use the typed API; P4 owns their discovery and persistence.
- The P2 CLI accepts explicit structured command fields only. It does not introduce a whole YAML
  or JSON overwrite path, and it does not send a request, open an upstream socket, or add a
  management HTTP listener.

## Consequences

An operator can now establish a Config Version lifecycle and reconstruct its most recent rollback
predecessor after restart without giving the inference path a Repository or database handle. The
data plane continues to load only its immutable `Arc<RouteSnapshot>`; it does not observe audit
writes, call the management service, or query SQLite.

The minimal CLI intentionally cannot make a populated Route graph publishable from fabricated
Catalog/capability data. It fails closed until an embedding supplies real injected evidence. This
is narrower than P10's eventual structured entity CRUD, but prevents P2 from smuggling a broad
configuration-import or remote administration surface across the security boundary.

## Alternatives considered

- Writing the audit row after activation or after `ArcSwap` was rejected because a storage failure
  could leave an active configuration without a matching audit record.
- Keeping audit events only in memory was rejected because restart would lose both evidence and
  the durable predecessor needed by local rollback.
- Reconstructing a predecessor by choosing an arbitrary archived Version was rejected because
  archival order is not an explicit rollback relationship. The successful transition audit record
  carries the exact replaced Version.
- Adding a management HTTP endpoint now was rejected because P10 owns its OpenAPI contract,
  management authentication, localhost/private-network policy, CSRF/CORS boundary, and UI.
- Accepting a full configuration-file overwrite was rejected because the matrix explicitly replaces
  that high-risk pattern with structured versioned management transactions.

## Validation and rollback

Tests cover migration up/down, append ordering, transaction-bound `config_created` and publication
events, active/predessor reconstruction after restart, version rollback, immutable reader behavior,
and safe rejection of synthetic first-publish rollback. The CLI exercise creates two drafts,
validates and publishes both in separate invocations, rolls back after a fresh bootstrap, and
prints five safe audit records. Rolling back this task removes migration 4, the local management
facade, and CLI subcommands; it neither starts P3 nor changes persisted encrypted Secret or Client
Key material.
