# ADR-0024: Catalog snapshot freshness and last-success fallback

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-21` |
| Task / Matrix / Contract references | `P4-02`; `E20`, `G28`, `L09`, `L10`, `L33`; [BC-CATALOG-002](../contracts/BC-CATALOG-002-catalog-snapshot-freshness-and-last-success-fallback.md) |

## Context

P4-01 proves that concurrent discovery is shared only by one exact `EndpointId + CredentialId` and
that its scheduler deliberately retains no result. The next control-plane boundary must preserve
the last successful discovery across source failure and attach explicit Fresh/Stale/Expired time.
It must not confuse a background refresh target with hard expiry, and must not prematurely add
P4-03 removal/diff behavior or persistent/runtime publication integration.

The frozen architecture defaults to `Fresh 6h / Stale 24h / Expired 72h`. The phrase needs an
unambiguous state boundary: the 24-hour point is a refresh-due target while a snapshot is already
Stale after six hours and remains retained until the 72-hour hard expiry.

## Decision

- `CatalogFreshnessPolicy` validates positive, ordered Unix-millisecond intervals. Defaults are
  Fresh through 6 hours, refresh due at 24 hours, and Expired at 72 hours.
- `CatalogSnapshot` is immutable and contains one exact `ModelCatalogTarget`, sorted/deduplicated
  discovered model names, a target-local success version, observed time, stale time, refresh-due
  time, and expiry time. It accepts a successful empty list; P4-03 later decides removal/diff.
- `CatalogSnapshotStore` is process-local and locks successful replacement atomically. It changes
  only the exact target entry, assigns versions from one upward, rejects a success older than that
  target's retained snapshot, and leaves its prior entry unchanged on any rejected update.
- `retain_last_success_on_failure` is intentionally non-mutating. It returns existing success for
  exactly the failed target and stores neither raw error nor failure payload. A later observability
  or persistence Task may record a failure Run without overwriting source success evidence.
- Fresh/Stale/Expired and refresh-due evaluation take explicit `i64` Unix milliseconds. Pre-epoch,
  overflow, and caller timestamps before a snapshot are rejected safely.

## Consequences

The in-memory snapshot boundary remains credential-isolated and deterministic for tests. Static
models continue to use the existing `CatalogModelState::Manual` boundary and are not relabeled as
discovered state. This Task does not read SQLite, call a Provider, publish RouteSnapshots, alter
health/quota, create `/v1/models`, or infer added/suspected-removed/removed records.

`Stale` remains hard-eligible in this storage-neutral P4-02 type, while `Expired` is not. A later
route/compiler integration must make any explicit exception visible rather than silently treating
an expired discovery as current.

## Alternatives considered

- Endpoint-only snapshots: rejected because Credentials sharing an Endpoint can expose different
  model entitlements.
- Clearing the list on a failed discovery: rejected because transient failure is not evidence that
  previously successful models disappeared.
- Treating 24 hours as Expired: rejected because it would collapse the documented Stale retention
  window and erase the stated 72-hour hard deadline.
- Implementing diff/removal now: rejected because P4-03 owns three successful absences, 24-hour
  isolation, Preview/Apply, and removal controls.

## Validation and rollback

Synthetic tests prove exact Fresh/Stale/Expired boundaries, the 24-hour refresh-due value, failure
retention, Credential isolation, target-local replacement/versioning, empty-success acceptance,
and invalid/overflow/non-monotonic timestamp rejection. They use no URL, Credential value, network
client, Provider request, SQLite database, or external time source.

Rollback removes the snapshot types/store/tests and reverts P4-02 documentation. It changes no
persisted Catalog data, public endpoint, deployed behavior, RouteSnapshot, health state, quota, or
P4-01 discovery scheduler semantics.
