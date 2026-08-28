# BC-CATALOG-002: CatalogSnapshot freshness and last-success fallback

| Field | Value |
|---|---|
| Contract | `BC-CATALOG-002` |
| Task | `P4-02` |
| ADR | [ADR-0024](../adr/ADR-0024-catalog-snapshot-freshness-and-last-success-fallback.md) |
| Domain | Storage-neutral discovery success retention before P4-03 diff/persistence integration |

## Entry and boundary

`CatalogSnapshotStore` accepts a `ModelCatalogTarget`, successful `DiscoveredModel` list, and
explicit Unix-millisecond observation time. The target contains exactly one stable Endpoint and
Credential identity. `CatalogSnapshot` exposes immutable normalized models and explicit deadlines;
`status_at` evaluates a retained target at an explicit timestamp.

The boundary has no URL, credential material, provider client, network operation, SQLite handle,
RouteSnapshot publication, health/quota mutation, or public model endpoint.

## Preconditions

1. P4-01 has already produced an exact-target discovery result or failure.
2. Discovery success observation time is non-negative Unix milliseconds and does not precede the
   retained success for that exact target.
3. Timing policy is positive and ordered: Fresh period <= refresh due < hard expiry.

## State sequence and invariants

| Event / instant | Required behavior |
|---|---|
| First successful discovery | Stores an immutable target-local snapshot at version `1`; names are sorted/deduplicated without changing case. Empty success is valid. |
| Later successful discovery | Replaces only the same target atomically and increments its version; another Credential's snapshot remains untouched. |
| Discovery failure | `retain_last_success_on_failure` returns only that target's retained success and performs no mutation. No raw error is retained here. |
| `observed <= now < observed + 6h` | `Fresh`; refresh is not due. |
| `observed + 6h <= now < observed + 72h` | `Stale`; it remains retained and hard-eligible. |
| `now >= observed + 24h` | Background refresh is due, but this does not create a separate state or change the 72-hour expiry. |
| `now >= observed + 72h` | `Expired` and no longer hard-eligible. |

## Error semantics

| Condition | Safe result |
|---|---|
| Pre-epoch observation | `TimestampBeforeUnixEpoch`; prior success is unchanged. |
| Deadline arithmetic overflow | `TimestampOverflow`; prior success is unchanged. |
| Later success timestamp before retained one | `TimestampNotMonotonic`; prior success is unchanged. |
| Freshness evaluation before observation | `ClockBeforeSnapshot`; snapshot is not relabeled. |
| Invalid policy ordering | Specific policy construction error before a store exists. |
| Registry lock unavailable | `StoreLockPoisoned`; caller fails closed. |

## Deferred behavior

P4-03 owns added/suspected_removed/removed, consecutive-missing counters, isolation, and
Preview/Apply. P4-04 owns dynamic health/circuit. P4-07 owns durable writes. No P4-02 API may use
a successful empty list to imply automatic removal or mix static/manual models into discovery data.

## Corresponding tests

- `gateway-catalog::tests::catalog_snapshot_uses_explicit_fresh_stale_refresh_and_expiry_boundaries`
- `gateway-catalog::tests::discovery_failure_retains_only_its_target_last_success`
- `gateway-catalog::tests::successful_empty_catalog_is_retained_without_inventing_removal_semantics`
- `gateway-catalog::tests::catalog_snapshot_rejects_invalid_or_non_monotonic_times_without_replacing_success`
- `cargo test --locked -p gateway-catalog`
- `./scripts/check.sh fast` and `./scripts/check.sh full`
