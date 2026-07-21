# P4-03 Catalog diff Preview/Apply and removal isolation

| Field | Value |
|---|---|
| Plan version | `v1.4` |
| Task | `P4-03` |
| Status | `LOCAL_PASS_PENDING_CI`; local implementation, review, and complete Gate passed; GitHub Code Gate pending |
| Scope level / execution budget | `M`; `<=25min` from Task Card to code commit |
| Task Card | Exact target-local successful-snapshot diff; no network, persistence, routes, health, quota, or P4-04 work |
| References | `E20`, `G28`, `L09`, `L10`, `L33`; [ADR-0025](../adr/ADR-0025-catalog-diff-preview-apply-removal-isolation.md); [BC-CATALOG-003](../contracts/BC-CATALOG-003-catalog-diff-preview-apply-removal-isolation.md) |

## Scope

P4-03 adds deterministic `Added` / `SuspectedRemoved` / `Removed` evidence from successful,
exact-Endpoint/Credential `CatalogSnapshot` inputs. A preview is non-mutating and apply is guarded
by a target-local generation, so stale preview data cannot overwrite a newer applied success.

Removal requires three consecutive successful absences plus 24 hours from the first absence.
Static/manual models and all control-plane configuration remain outside this registry.

## Local verification

| Command / review | Result |
|---|---|
| `cargo test --locked -p gateway-catalog` | PASS; 15 unit tests, including non-mutating Preview, stale-plan rejection, three successful absences plus the 24-hour boundary, reappearance reset/add behavior, and same-Endpoint Credential isolation. |
| `cargo clippy --locked -p gateway-catalog --all-targets --all-features -- -D warnings` | PASS. |
| `CHECK_REPORT_PATH=tmp/p4-03-full-check.md ./scripts/check.sh full` | PASS in 54 seconds (started `2026-07-21T15:21:59+08:00`, completed `2026-07-21T15:22:53+08:00`); it covers Fast, formatting, workspace Clippy/tests, document links, tracked Secret scan, whitespace, pinned-tool validation, `cargo deny`, and RustSec audit. |

`cargo deny` emitted existing duplicate-version notices. `cargo audit` printed a transient
crates.io yanked-metadata lookup timeout but completed its advisory scan and the checker exited
zero; the report records the final `PASS` result rather than treating that non-fatal network notice
as a security acceptance claim.

## Review and execution measurement

The scope review confirmed all state is keyed by the exact `ModelCatalogTarget`, source failure has
no diff entrypoint, Preview does not mutate, Apply is target-local CAS, and a model remains
suspected unless both `misses >= 3` and `observed_at >= removal_eligible_at` hold. Static/manual
Catalog state, routes, health/quota, HTTP, SQLite, and public-model publication are untouched.

| Measurement | Evidence / value |
|---|---|
| Scope / budget | `M`; target `<=25min` from Task Card to code commit. |
| Task Card | The first P4-03 code artifact was modified at `2026-07-21T15:18:55+08:00`; the prior visible Task Card did not leave a durable timestamp, so this is the conservative local timing anchor. |
| Local complete Gate | `2026-07-21T15:21:59+08:00` to `2026-07-21T15:22:53+08:00` (54s). |
| Repeated complete Gates | `0`; the one required complete Gate was not mechanically replayed. |
| Rework | `0`; focused review required no implementation correction after the successful targeted test and Clippy runs. |
| Code commit / Code Gate / docs closeout / docs Gate | Pending immutable evidence after this code delivery and its normal GitHub workflow. |

## Remote Code Gate

The normal Code Gate on the cache-visible delivery ref will provide the only workflow measurement.
No manual rerun will be issued.

## Closeout boundary

After Code Gate success, one docs-only closeout will record immutable evidence and mark P4-03
`DONE`. P4-04 remains `PENDING` until its own Task Card is started.
