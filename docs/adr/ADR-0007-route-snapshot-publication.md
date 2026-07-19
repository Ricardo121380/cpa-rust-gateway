# ADR-0007: Immutable RouteSnapshot publication

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-19` |
| Task / Matrix / Contract references | `P2-07`; `D01/D04/D10/D11/D20/D21/D31`, `H05/H06`, `J20`, `L17-L31`; [BC-ROUTER-002](../contracts/BC-ROUTER-002-route-snapshot-publication.md) |

## Context

P2-06 validates a complete persisted graph but deliberately leaves request-time routing and Config
Version activation untouched. The data plane must not query SQLite or observe a half-published
configuration. A streaming request also cannot change Config Version midway through its lifetime.

The P2-01 schema allows one active Config Version, but P2-05 intentionally permits only draft
mutations. P2-07 must connect a validated graph to an immutable runtime view and make publication
and one-step rollback explicit, without exposing encrypted Credentials or Client Key digests to the
runtime snapshot.

## Decision

- `gateway-router` owns immutable `RouteSnapshot` runtime values and an
  `ArcSwap<RouteSnapshot>` registry. Readers call `load_full` once and retain that `Arc` for an
  entire request; they never take a publication lock or access the Repository.
- `gateway-control` owns the conversion from P2-06 compiler output to the router-safe snapshot
  input and the publication orchestration. This conversion maps persisted scheduling and transform
  enums to runtime equivalents rather than making `gateway-router` depend on `gateway-store`.
- `gateway-store` owns an atomic Config Version transition: a selected `draft` or `archived`
  Version becomes `active`, and the former active Version becomes `archived` in the same SQLite
  transaction. P2-07 continues to forbid ordinary graph mutation outside `draft` Versions.
- The registry has a control-plane-only publication mutex and retains exactly the immediately
  preceding immutable snapshot. Publishing prepares and validates the replacement before the
  database transaction; after a successful transition its `ArcSwap` commit has no fallible work.
  Rollback uses that retained predecessor and atomically swaps the two in-memory positions after
  the matching database transition succeeds.
- A compile, snapshot-construction, preparation, or database-transition error leaves the active
  in-memory Snapshot unchanged. P2-07 does not add runtime scheduling, Client Key admission,
  Provider execution, Catalog discovery, management HTTP/CLI, or startup wiring; later tasks own
  those concerns.

## Consequences

The request hot path gets a stable, lock-free `Arc<RouteSnapshot>` and can never see a mix of
Alias, Route, Access Group, or Candidate versions. A prior stream retains its older `Arc` even
after a newer Snapshot has been published. The control path can roll back once without reloading
SQLite or retaining sensitive configuration values in the Snapshot.

The first publish and the rollback target must have already been validated by P2-06. Rebuilding a
registry from the active database Version during process startup and exposing these operations via
management interfaces remain P2-10 work; P2-07 supplies the typed primitives for that later
bootstrap.

## Alternatives considered

- A `RwLock` around a mutable router table was rejected because read-side locking makes the
  request hot path contend with management publication and cannot naturally pin a stream to its
  starting Version.
- Letting `gateway-router` read `ControlPlaneConfiguration` or SQLite was rejected because it
  reverses the crate boundary and risks a database query per request.
- Swapping memory before the SQLite state transition was rejected because a database failure could
  expose a Snapshot for a Version that was never activated.
- Retaining every historic Snapshot was rejected because it creates an unbounded control-plane
  memory store. The locked plan only requires the immediately previous Version for one-click
  rollback.

## Validation and rollback

Tests cover compiler-to-Snapshot conversion without sensitive fields, database activation and
archival, compile failure preserving the active Snapshot, concurrent readers pinning their first
loaded Version across publication, and a one-step rollback that swaps current/previous Versions.
No schema migration is required. Rolling back P2-07 removes the runtime publisher and restores the
P2-05 draft-only state; it does not mutate encrypted Credential or Client Key material.
