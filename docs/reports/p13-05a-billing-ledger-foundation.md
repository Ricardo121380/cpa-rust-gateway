# P13-05A Billing ledger foundation report

Date: `2026-08-11`
Branch: `codex/p13-05-billing-ledger`

## Scope

This slice starts P13-05 after the public-release redaction.  It adds a versioned price catalog,
fixed-point pricing semantics and an idempotent, retention-bounded ledger.  It does not yet expose
the management HTTP time-series/reporting surface and does not contact Providers or production
databases.

## Implementation

- Schema migration 0014 adds immutable catalog versions/entries and `billing_ledger_entries`.
- Rates are integer micro-units per million tokens; checked `u128` arithmetic is used before the
  persisted `u64` conversion.
- Source Usage events are fingerprinted with SHA-256. Identical replays return the existing row;
  conflicting replays fail closed.
- Pricing returns `exact`, `partial`, `unknown`, or `unpriced`; absent token values are never
  guessed as zero. Retention deletion is bounded by caller-provided limit.
- File reopen and SQLite migration tests prove restart recovery of surviving rows.

## Local verification

| Check | Result |
|---|---|
| `cargo fmt --all` | PASS |
| `cargo test --locked -p gateway-control billing_service` | PASS (3 tests) |
| `cargo test --locked -p gateway-store --lib` | PASS (42 tests) |

## Boundaries / next slice

The next P13-05 slice should connect this foundation to a protected management read model with
bounded time-window filters, catalog selection metadata, aggregate cost/status and an OpenAPI/
generated-client contract.  It must preserve P13-04's `unpriced` facts and remain read-only with
respect to Providers and active Config Versions.
