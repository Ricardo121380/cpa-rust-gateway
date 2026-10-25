# Issue 72: unified Usage query and snapshot export

Date: 2026-10-25 (Asia/Shanghai). Branch: `codex/72-usage-query-export`.
Common baseline: `444558a8de930f5f1febd5a40ec5bdbbc30a15fb`.

**Historical status at `d3dda4ba`: local implementation and controlled functional acceptance complete; Full gate blocked by an inherited supply-chain advisory.**

Follow-up on 2026-10-25: the separately authorized
[RUSTSEC-2026-0387 fix](crypto-bigint-security-fix-20261025.md) upgrades the P-256 dependency chain;
the subsequent local Full passes all 44 steps (1677 Rust tests passed, 0 failed, 21 ignored), with
no advisory exemption. The original #72 results and failed receipts below remain unchanged.
This follow-up does not change the Usage/ledger/query/export implementation or schema.

Implementation: [contract](../contracts/BC-MGMT-020-frozen-usage-query-export.md),
[decision](../adr/ADR-0128-frozen-usage-query-export.md).

## Local implementation

The new protected management snapshot API freezes existing Request/Attempt/Usage facts and their
original versioned billing ledger entries in a single SQLite read transaction. It provides a
full-filter summary, bounded detail/bucket pages, and CSV/JSONL pages from that same immutable result.
There is no schema migration, alternate ledger, re-pricing, price refresh or quota-history change.
Old Usage/billing/history endpoints retain their contracts.

## Acceptance results

All results below are **local controlled evidence**, not real-Provider or production qualification.

| #72 criterion | Result and evidence |
|---|---|
| Existing facts and unified time/model/account/Key/Group filters | PASS: one read transaction over the original event log and ledger; actual HTTP positive, negative and combined dimension cases |
| Bounded buckets/pagination and distinct requests/attempts | PASS: independent whole-range and per-bucket oracle; a retry is one Request and two Attempts |
| Verified costs, missing prices and evidence admission | PASS: original ledger amounts/catalog versions retained; exact/partial/unknown/unpriced/pending/unavailable separated; synthetic estimated Usage remains unknown-cost |
| Explicit consistency and nonzero meaning for unknown states | PASS: snapshot ID plus event/ledger watermarks; null, observed zero, incomplete and unavailable states remain distinct |
| Backend permission and caller scope | PASS: existing management guard, actor-owned cache, independent Key/Group filters; no secret Key copy or alternate ledger |
| Actual management HTTP whole-filter oracle | PASS: retry, stream failure, socket cancellation, missing prices, cross-Key and concurrent-write cases; every page compared |
| OpenAPI, metrics and frontend example | PASS: additive OpenAPI schemas/paths, BC-MGMT-020 and examples; large-scale performance and production data remain unqualified |
| Distinct detail and aggregate export grains | PASS: Attempt/Request-only details and separately paged time buckets |
| CSV/JSONL bounded export and field semantics | PASS: version v1, UTF-8 CSV/JSONL, UTC ms, token counts, integer microunits, null/zero/partial retained |
| Immutable batch and explicit invalidation | PASS: concurrent append and ledger purge do not alter retained results; expired/cancelled/restarted handles fail, cursors bind snapshot/grain |
| Protected safe export | PASS: DTO selects permitted fields, no raw metadata/private payloads/credentials; CSV formula and quoting regressions |
| Actual HTTP pages, cancellation, limits and totals | PASS: source overlimit 413, invalid pagination 400, cursor conflict 409, source unavailable 503, cancel/restart 410; full CSV/JSONL data and bucket totals compared |
| Export is not backup | PASS: lifecycle/rollback/backup boundaries documented explicitly |

## Validation record

- `cargo check --workspace --all-targets --all-features`: PASS.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`: PASS.
- `cargo test --locked -p gateway-http-actix --test issue72_usage_snapshots`: PASS (1 test).
- Production gateway with controlled loopback TLS upstream: **517 checks PASS**, seven synthetic
  upstream calls, source `881f8e9b0f3d8144ba5f639182a1f89922cfb3f5`.
  Binary SHA-256: `deba5a2398e6c19d989b20ed100688b4cc7af1ca76777922640fc1ea482d6f6b`.
  The oracle independently recomputes all six token metrics, distinct request/attempt and outcome
  counts, all cost-status counts, known costs and each bucket from selected full-detail facts;
  CSV checks all columns and JSONL checks complete rows for both grains.
- Python syntax, document links, contract references, tracked secret scan and diff whitespace: PASS.
- First Full run: Rust tests PASS (1676 passed / 0 failed / 21 ignored), then the crate-boundary
  checker rejected the unlisted direct `serde` edge. The exact dependency list and ADR were
  synchronized without disabling or weakening the equality check.
- Final `scripts/check.sh full` rerun at code commit `daf4718b14ba58be00ede963b957e8fdfaaf14ac`:
  **FAIL at Dependency policy** after 42 passing steps. Rust tests again passed 1676 / failed 0 /
  ignored 21; serve, Agent stream regression, source/boundaries, docs/contracts, tracked secrets,
  whitespace and tool versions passed. The final RustSec-audit step was not reached by the script.
- Separately ran `cargo audit`: **FAIL**, one vulnerability, the same inherited
  [RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387.html) in `crypto-bigint 0.5.5`.
  The advisory describes possible secret-dependent timing in intended constant-time arithmetic;
  applicability/exploitability in this deployment has not been established by this ticket.
- Baseline comparison: all 366 locked package name/version/source/checksum identities equal
  `444558a8de930f5f1febd5a40ec5bdbbc30a15fb`. The lockfile's only change is the direct `serde` edge
  in gateway-control. The reported path includes existing `provider-grok -> p256 0.13.2 ->
  elliptic-curve 0.13.8 -> crypto-bigint 0.5.5`. No advisory ignore or dependency upgrade was added.
  Resolve this dependency chain in a separately scoped change and rerun Full; this result is not
  a green merge/release gate.

Original failed iterations remain under `output/issue-72/logs`: enum typo, lint findings, missing
crate-boundary declaration and a synthetic fixture that incorrectly combined inclusive/exclusive
cache fields. The fixture was corrected; existing billing admission rules and expected monetary
assertions were kept. Both Full failures and the separate audit failure remain inspectable.

All databases, generated local test credentials and logs belong to this worktree's ignored
`output/issue-72`. Compilation uses its own `target`; loopback listeners use dynamic ports.
Only sanitized receipts are intended for reporting. No real Provider, production database,
actual external client, deployment or remote CI qualification is claimed.

## Code review

Fixed comparison: `444558a8de930f5f1febd5a40ec5bdbbc30a15fb...cd01f923`.
Standards: zero documented-rule violations and zero actionable heuristic findings.
Spec: one P2 acceptance-coverage finding, corrected in `152155f9` and `881f8e9b`.
Independent follow-up confirmed that dimension coverage and independent bucket/CSV oracles now
close the identified gaps. The expanded process-level test passed 517 checks. No known actionable
review finding remains; static review and runtime evidence are recorded separately. The subsequent
exact dependency-list synchronization and documentation-only clarifications were reviewed by the
primary agent; the dependency checker passed without changing its equality assertion.

## Shared integration surface

- Additive `/admin/operations/usage-snapshots` and ID-scoped metadata/rows/export/delete operations.
- `ManagementUsageFacade::create_usage_snapshot` has a default unavailable implementation; the
  gateway composition supplies the real read-only implementation.
- New store capture and control read-model modules; the existing billing lineage resolver is reused.
- OpenAPI schemas/paths, management route registration and HTTP state, `apps/gateway/src/deployment.rs`,
  ADR index, traceability, roadmap, and the existing synthetic acceptance harness are touched.
- ADR-0128 and BC-MGMT-020 are chosen against the common baseline. Parallel #68/#74 integration must
  check numbering and shared files; it has not been performed here. Migration/schema version unchanged.
- `gateway-control` directly declares the already-workspace-owned `serde`; its exact dependency
  declaration in `scripts/check-crate-boundaries.rb` is synchronized. No dependency version changes.

## Operational limits

Capture is limited to 20000 source events /16 MiB source JSON and an 8 MiB serialized result.
Each process retains at most eight snapshots with a 32 MiB serialized-result budget for ten minutes;
allocator overhead and process RSS are not measured by this budget. Pages have 1–100 rows;
limits return explicit errors. Snapshot expiry, cancellation or process restart returns 410.
These are implementation bounds, not measured large-scale capacity. Exports are readable products,
not encrypted backups or a complete control-plane recovery format.
