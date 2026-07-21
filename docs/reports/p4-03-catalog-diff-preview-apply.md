# P4-03 Catalog diff Preview/Apply and removal isolation

| Field | Value |
|---|---|
| Plan version | `v1.4` |
| Task | `P4-03` |
| Status | `DONE` after this docs-only closeout Gate; Code Gate passed |
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
| Code commit | `ecaee4f`, `2026-07-21T15:28:42+08:00`; about 10 minutes after the conservative local timing anchor, within the `M` budget. |
| Code Gate passed | `2026-07-21T16:00:00+08:00`; [run 29812271739](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29812271739). |
| Docs closeout / docs Gate | This one docs-only closeout records the immutable Code Gate evidence. Its required docs-only Gate is external evidence and will not cause a second status commit. |

## Accepted GitHub Code Gate and delivery-flow measurement

GitHub Actions [run 29812271739](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29812271739)
passed for implementation commit `ecaee4f` on the cache-visible
`codex/p4-01-catalog-singleflight` delivery ref. It was the normal push Gate, not a manual rerun.

| Job / step | Result and duration |
|---|---|
| Classify delivery gate | PASS; selected `code`; Docs-only Gate correctly skipped. |
| Fast gate | PASS; job about 189 seconds, complete `Run fast gate` about 162 seconds. |
| Full supply-chain gate | PASS; job about 56 seconds after Fast. |
| Cache | PASS; primary key hit; restore took about 7 seconds. |
| Install pinned quality tools | PASS; `cargo-deny 0.20.2` and `cargo-audit 0.22.2` were version-verified in about 1 second, within the `<=10s` operational target and `<=90s` hard ceiling. |
| Supplemental supply-chain | PASS; version verification, `cargo deny check`, and RustSec audit completed in about 9 seconds without replaying Workspace Fast checks. |
| Required delivery gate | PASS; fail-closed verification of the code path's Fast + Full results. |

The workflow was created at `2026-07-21T15:55:31+08:00`, first started about 3 seconds later, and
completed at `2026-07-21T16:00:00+08:00` (about 4 minutes 29 seconds end-to-end). The warm
`<=4min` operational target missed by about 29 seconds, following P4-02's one-second miss. This is
a delivery-performance observation, not a correctness or supply-chain failure: the cache hit and
all required Gates passed. No manual rerun is issued. Any further CI acceleration needs an explicit
approved Change Request; this closeout does not alter the locked delivery workflow.

## Closeout boundary

This is the single docs-only closeout that records the immutable Code Gate evidence and marks P4-03
`DONE`. Its own GitHub status is external evidence and will not cause another status-only commit.
