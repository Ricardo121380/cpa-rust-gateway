# P13-05C Protected billing catalog management report

Date: `2026-08-11`
Branch: `codex/p13-05-billing-ledger`

## Scope

P13-05C exposes the P13-05A immutable price catalog through the existing P10 protected management
boundary.  It closes the operator-write gap identified by the P13-05A/B review without creating a
parallel control plane or coupling billing to Provider-owned account state.

## Delivered

- `GET /admin/billing/catalogs` lists a bounded, revisioned catalog projection.
- `POST /admin/billing/catalogs` imports one reviewed immutable catalog.
- `POST /admin/billing/catalogs/{catalog_version_id}/rollback` creates a new catalog from a retained
  predecessor; it never deletes or edits the predecessor.
- Catalog write, selected draft Config Version revision increment and resource audit commit in the
  same SQLite transaction.
- Management import is create-only: any reused catalog id returns 409 without revision/audit
  mutation.  The lower Store's exact-replay behavior remains reserved for crash recovery.
- Existing Management Key, network admission, same-origin browser CSRF, `X-Config-Version`,
  `If-Match`, ETag and safe error envelopes are reused.
- OpenAPI 3.1 and the generated TypeScript management client expose the three operations.
- Management JSON input caps rates and effective timestamps at `9_007_199_254_740_991`, the
  largest integer exactly representable by JavaScript/TypeScript; the lower management boundary
  prevents a UI or generated client from silently rounding a price while the Store retains its
  existing wider SQLite `i64` storage capability.

The mutation response is a value-free receipt.  Catalog reads expose pricing data but never
credential material, endpoint URLs, request content, client-key digests or billing source-event
fingerprints.  Existing ledger rows remain immutable and are not retroactively repriced.

## Local verification

| Check | Result |
|---|---|
| `cargo test --locked -p gateway-store --no-fail-fast` | PASS (43 unit + integration tests) |
| `cargo test --locked -p gateway-control --no-fail-fast` | PASS (50 tests) |
| `cargo test --locked -p gateway-http-actix --test p13_05c_billing_catalog` | PASS |
| `cargo test --locked -p gateway-http-actix --test p10_01_management_openapi_contract` | PASS (7 tests) |
| `cargo clippy --locked -p gateway-control -p gateway-http-actix -p gateway --all-targets --all-features -- -D warnings` | PASS |
| `node scripts/generate-management-client.mjs --check` | PASS |
| `node scripts/check-management-spa.mjs` | PASS (76 generated operations, reproducible static build) |
| `./scripts/check.sh docs` | PASS (links, contracts, plan state, secret scan, whitespace) |
| `cargo fmt --all -- --check` and `git diff --check` | PASS |

## Overall P13-05 review

P13-05 now has one coherent backend chain: immutable versioned catalog → effective-time quote →
restart-safe event materializer → idempotent ledger → protected filtered billing read model, plus
an authenticated and auditable operator catalog write.  Fixed-point arithmetic, Provider scoping,
unknown/unpriced confidence, checkpoint replay, retention, revision conflicts and response
redaction remain explicit and covered.

Automatic materializer cadence/retry ownership, catalog discovery, a formal frontend, Provider
calls and production catalog changes are not part of P13-05C.  They must not be inferred from this
local backend pass.

## Boundary

P13-05 is `DONE_WITH_BOUNDARY` after formal P13 Delivery Gate run
[31858904767](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31858904767) passed for
the exact pushed revision. No production, server, Provider, OAuth or account refresh activity
occurred, and the four existing untracked helper scripts remain outside this change. The formal
frontend and automatic materializer/Provider lifecycle work remain separate follow-up tasks.
