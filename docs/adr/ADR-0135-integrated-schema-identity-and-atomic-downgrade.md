# ADR-0135: Integrated schema identity and atomic downgrade

- Status: Accepted
- Date: 2026-10-25 (Asia/Shanghai)
- Scope: #70/#71 integration; limited successor to ADR-0133/0134 schema ordering and downgrade semantics.

The two independently tested branches both recorded schema41 with different physical layouts.
The common prefix1–40 remains unchanged. The integrated chain retains routing/affinity as41,
then adds endpoint request policy as42. A version number alone cannot identify those historical
databases: migration, schema inspection, downgrade and both backup readers now use the same
bounded structural check for the known40/41/42 distinction. It checks routing columns, endpoint
policy presence and the affinity break table's nullable loss/new reason. It is not a general
schema corruption detector or a migration checksum registry.

Common40 and #70 standalone41 can upgrade. #71 standalone41 and mixed/missing wave3 layouts
reject before migration or target publication. Backup preflight reports incompatible; restore
creates no target. Preserve the original #71 binary, artifact, database and Master Keys. Do not
rewrite its version number or drop its nonempty policy to force compatibility; cross-branch
data conversion requires a separate explicit migration procedure.

Endpoint policy may downgrade only when semantically empty (`{}` or empty rules/headers).
Any nonempty or unrecognized configuration blocks it. All requested down steps and bookkeeping
run in one transaction so a later guard failure rolls back earlier steps too. Existing explicit
routing, qualified affinity and unknown-loss guards remain. Successful downgrades retain each
older migration's existing destructive contract; this is not a blanket promise that arbitrary
historical down migrations preserve every table. Operational rollback still uses retained
configuration versions or a coherent artifact with its matching binary and keys.

This uses the existing Store boundary and SQLite transactions, with no new dependency or
configuration service. Regressions in `crates/gateway-store/tests/integrated_schema.rs` reproduce
the four original failures, then verify both historical upgrade paths, rejection without writes,
semantic empty-policy rollback, and complete rollback preservation. Actual nonempty schema42
backup adoption is separately exercised by the stock-process integration driver.
