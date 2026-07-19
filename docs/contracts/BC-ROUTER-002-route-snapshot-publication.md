# BC-ROUTER-002 Immutable RouteSnapshot publication

| Field | Value |
|---|---|
| Contract | `BC-ROUTER-002` |
| Task | `P2-07` |
| Status | DONE |
| Domain | Version-pinned RouteSnapshot publication and one-step rollback |

## Boundary

`gateway-router::RouteSnapshot` is an immutable, runtime-safe view of P2-06-approved public
models, aliases, routes, candidates, and Access Group grants. It stores no `SQLite` connection,
`ControlPlaneConfiguration`, encrypted Credential, plaintext Credential, complete Client Key,
Provider instance, or HTTP type. The P2-07 Snapshot shape initially contained no Client Key digest;
P2-08's separately specified extension permits only the redacted, zeroizing HMAC record enclosed
by `SnapshotClientKeyView` for Snapshot authentication.

`RouteSnapshotRegistry` owns `ArcSwap<RouteSnapshot>`. The data plane obtains one `Arc` through
`load()` and keeps it for the entire request or stream. Only a management publication path obtains
the separate publication mutex; ordinary reads never contend on it.

```text
persisted Config Version
  -> P2-06 RouteCompiler
  -> router-safe RouteSnapshot input
  -> prepare replacement Snapshot
  -> SQLite: target Active / old Active Archived
  -> ArcSwap commit
  -> requests observe either whole old Snapshot or whole new Snapshot
```

## Publication

- A target Version must be present and have status `draft` or `archived`; an already `active`
  Version cannot be re-published as a no-op.
- P2-06 compilation and Snapshot construction happen before the Config Version transition.
- The SQLite transaction archives any existing active Version before activating the target, so the
  unique active-Version invariant holds throughout the commit.
- The registry reserves the replacement while holding the publication mutex. If compilation,
  construction, reservation, or the database transition fails, it drops the reservation and leaves
  the current `ArcSwap` value unchanged.
- Once SQLite activation succeeds, committing the prepared replacement only stores an already
  allocated `Arc` and updates the previous-snapshot slot; it has no fallible operation.

## Version pinning and rollback

- Every `load()` returns an owned `Arc<RouteSnapshot>`. A later publish cannot mutate that Snapshot
  or alter its Config Version.
- The registry retains exactly one predecessor. A successful publish replaces it with the former
  current Snapshot.
- `rollback()` prepares that predecessor, atomically activates its archived Config Version in
  SQLite, and swaps it into `ArcSwap`; the former current Snapshot becomes the new predecessor.
- A rollback with no predecessor, a missing target Version, an invalid target status, or a database
  failure returns an error without swapping the current Snapshot.

## Deferred behavior

P2-07 does not authenticate Client Keys, expose `/v1/models`, choose Candidates or Credentials,
create weighted schedules, execute Providers, query Catalogs, provide management HTTP/CLI, or
bootstrap a process from SQLite. P2-08, P3, P4, and P2-10 own those operations.

## Corresponding tests

- A P2-06 multi-Candidate compiled graph converts to a deterministic Snapshot containing no
  Credential material; P2-08 separately covers its redacted Client Key HMAC view.
- Database publication transitions `draft -> active` and `active -> archived` atomically.
- A compiler/snapshot error leaves the currently loaded Snapshot intact.
- 100 concurrent readers retain their originally loaded Snapshot across a publication.
- Rollback toggles the current and immediately previous Versions without exposing a partial view.
