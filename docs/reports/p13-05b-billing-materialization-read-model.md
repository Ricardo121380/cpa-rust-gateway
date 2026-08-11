# P13-05B Billing materialization and protected read-model report

Date: `2026-08-11`
Branch: `codex/p13-05-billing-ledger`

## Scope

P13-05B connects the P13-05A versioned catalog/ledger to the durable gateway event log and adds a
protected management billing page.  It remains backend-first: no Provider calls, production
configuration changes, active Config Version mutation or formal frontend UI work occurred.

## Delivered

- Migration 0015 adds a monotonic `billing_materializer_checkpoints` table.
- `SqliteEventStore::list_events_after_ordinal_bounded` provides a finite ordinal window.
- `billing_materializer` reloads complete Request/Attempt/Usage lineage, chooses the highest
  successful Attempt, selects the effective matching catalog and writes idempotent rows.
- `GET /admin/operations/billing` provides bounded time/provider/channel/account/model/status
  filters, snapshot keyset pagination, row-level confidence and status/cost summary.
- A `SqliteBillingManagementFacade` is available for protected management-listener composition;
  it is read-only and fail-closed on ledger errors.
- OpenAPI 3.1 and generated client now expose `listOperationalBilling`.

The production composition exposes the ledger through a read-only facade, but this task does not
start a background materializer scheduler or mutate the production database on startup.  The
bounded executor is intentionally an explicit operational primitive; wiring its trigger and
observability is a separate follow-up and must not be inferred from a successful billing read.

## Local verification

| Check | Result |
|---|---|
| `cargo fmt --all` | PASS |
| `cargo test --locked -p gateway-control` | PASS (49 tests) |
| `cargo test --locked -p gateway-store` | PASS (43 unit + integration tests) |
| `cargo test --locked -p gateway-http-actix --test p13_04_management_inventory` | PASS |
| `cargo test --locked -p gateway-http-actix --test p10_01_management_openapi_contract` | PASS (7 tests) |
| `cargo clippy --locked -p gateway-control -p gateway-http-actix --all-targets --all-features -- -D warnings` | PASS |
| `node scripts/generate-management-client.mjs --check` | PASS |
| `node scripts/check-management-spa.mjs` | PASS (73 operations) |

## Overall P13-05 review

P13-05A and P13-05B now have a coherent data path: final Usage event → checkpointed materializer →
versioned fixed-point quote → idempotent ledger → protected billing page.  Replay, retention,
unknown/unpriced confidence, Provider scoping and no-secret boundaries are covered by unit and
HTTP regressions.  The remaining explicit gap is the operator HTTP mutation/import contract for
price catalogs; it is not silently treated as complete and should be a follow-up P13-05C task
with CSRF, audit and revision semantics before production price updates are enabled.
The materializer trigger/scheduler is also intentionally not enabled by this read-model task; a
future operational task must define bounded cadence, retry ownership, metrics and rollback before
claiming automatic production billing ingestion.

## Boundary

P13-05 is `LOCAL_PASS_PENDING_PHASE_GATE`.  The phase Delivery Gate remains intentionally deferred
until P13 phase close under the repository rule of one expensive Gate per P.  No production,
server, Provider, OAuth, GitHub Actions or untracked helper files were changed.
