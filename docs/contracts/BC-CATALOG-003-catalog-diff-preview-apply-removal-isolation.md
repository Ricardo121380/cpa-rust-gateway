# BC-CATALOG-003: Catalog diff Preview/Apply and removal isolation

| Field | Value |
|---|---|
| Contract | `BC-CATALOG-003` |
| Task | `P4-03` |
| ADR | [ADR-0025](../adr/ADR-0025-catalog-diff-preview-apply-removal-isolation.md) |
| Domain | Target-local discovery diff evidence before control-plane action |

## Entry and boundary

`CatalogDiffRegistry::preview` accepts one successful immutable `CatalogSnapshot`. The snapshot
already owns exact Endpoint/Credential identity, normalized discovered models, version, and explicit
observation time. `apply` consumes only a matching current `CatalogDiffPreview` and returns the
events actually recorded for that target.

This boundary does not call a source, retry a source, treat a failure as a miss, persist data,
publish RouteSnapshots, mutate Public Models, delete routes, or alter static/manual Catalog entries.

## State sequence and invariants

| Successful snapshot sequence for one target/model | Preview event and Apply result |
|---|---|
| First presence | `Added`; apply begins target generation `1`. |
| Present in a later success | No event; any earlier suspicion is cleared when applied. |
| First successful absence | `SuspectedRemoved(misses=1)` with first-missing and 24-hour eligibility timestamps. |
| Second consecutive successful absence | `SuspectedRemoved(misses=2)`. |
| Third consecutive successful absence before 24 hours | `SuspectedRemoved(misses=3)`; model remains retained. |
| Third consecutive successful absence at or after 24 hours | `Removed(misses=3)`; discovery diff state no longer retains the model. |
| Reappearance before removal | No event and suspicion resets; the next absence begins at `misses=1`. |
| Reappearance after removal | `Added`; it is new discovery evidence. |

## Preview/Apply and isolation rules

1. Preview is non-mutating: equal previews may be created from unchanged state.
2. Apply compares the preview's target-local generation. After one apply, any sibling preview from
   the old generation fails with `StalePreview` and makes no partial change.
3. Successful snapshot versions must strictly increase and observation times must not regress for
   that target. A malformed/stale input fails before state changes.
4. Same Endpoint plus different Credential identities have independent generations, missing counts,
   and model sets. A preview for one cannot alter the other.
5. Only successful snapshots enter this state machine. P4-02 failure retention leaves the prior
   snapshot/diff state intact by not issuing a preview.

## Error semantics

| Condition | Safe result |
|---|---|
| Already applied or older snapshot version | `SnapshotVersionNotNewer`; no mutation. |
| Observation time regresses | `SnapshotObservedAtNotMonotonic`; no mutation. |
| Another preview applied first | `StalePreview`; no partial mutation. |
| Generation, miss count, or removal deadline overflows | Named finite error; no mutation. |
| Registry lock unavailable | `RegistryLockPoisoned`; fail closed. |

## Deferred behavior

P4-04/P4-05 own health and quota. P4-06 owns operator-facing Route Explain. P4-07 owns durable
event persistence. A later control-plane task may consume a `Removed` event explicitly, but P4-03
does not auto-remove static/manual records, aliases, mappings, Public Models, or routes.

## Corresponding tests

- `gateway-catalog::tests::catalog_diff_preview_is_non_mutating_and_apply_rejects_a_stale_plan`
- `gateway-catalog::tests::catalog_diff_removes_only_after_three_successful_misses_and_24h`
- `gateway-catalog::tests::catalog_diff_reappearance_resets_a_suspected_removal_sequence`
- `gateway-catalog::tests::catalog_diff_never_mixes_same_endpoint_credential_targets`
- `cargo test --locked -p gateway-catalog`
- `./scripts/check.sh full`
