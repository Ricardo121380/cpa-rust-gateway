# P13-07D Config-bound routing price evidence report

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

## Objective

Connect P13-05 immutable billing rates to P13-07 serving and Route Explain without guessing request
token usage, reading SQLite on the hot path or allowing a global catalog import to change an
already published Config Version.

## Frozen scope

- exact immutable catalog binding belongs to a draft Config Version;
- comparison is closed to `rate_dominance_v1`;
- routing consumes six rate dimensions, never a fabricated request `cost_microunits`;
- exact identity is Provider/upstream + Endpoint/channel + canonical public model;
- serving and Explain share one immutable scheduler price map;
- missing catalog tuples remain unpriced and eligible; missing/future bound catalogs fail closed;
- no public inference API change, Provider request, token-count request, production mutation,
  staging traffic, automatic refresh, P13-08, P13-11 or P13-12;
- protected management/OpenAPI changes are synchronized to Prism and logged for Claude Code, while
  formal frontend page work stays outside this backend slice.

## Implementation evidence

- `gateway-store` migration `0016_routing_price_policy` persists one Config-Version-owned catalog
  binding with foreign-key, draft-only, revision and atomic audit boundaries; round-trip, rollback,
  malformed-record and secret-free tests pass.
- `gateway-control` compiles exact public-model price maps from one effective immutable catalog and
  exposes revisioned set/get/clear management operations. Missing tuples remain unpriced; no token
  or request-cost estimate is produced.
- `gateway-router` carries six-dimensional `ProviderScopedPriceRates` and closed evidence
  (`dominant`, `equal`, `dominated`, `incomparable`, `unpriced`, `not_evaluated`) through the same
  scheduler-owned map used by serving and Route Explain. A single known vector is `equal` evidence;
  crossed vectors are incomparable.
- Runtime composition compiles the map once, shares it with serving and management Explain, rejects
  future/malformed/mismatched policy snapshots before serving construction, and does not add Store,
  token-count or Provider work to the request hot path.
- Protected HTTP/OpenAPI adds GET/PUT/DELETE routing-price-policy operations with Management Key,
  CSRF, Config-Version and If-Match controls. Route Explain now always returns nullable
  `price_policy` (`null` means disabled) and one required closed `price_evidence` per candidate.
- Prism's vendored contract and generated client are synchronized; generator and root compatibility
  checks explicitly retain PUT support. Formal UI integration remains a Claude Code follow-up.

## Verification

- `cargo test --locked -p gateway-store`: 50 unit tests plus repository/backup/upgrade integration
  tests passed.
- `cargo test --locked -p gateway-control`: 65 passed; `cargo test --locked -p gateway-router --lib`:
  128 passed; `cargo test --locked -p gateway --bin gateway`: 102 passed.
- `cargo test --locked -p gateway-http-actix --tests`: all executed tests passed (only explicitly
  authorized/live harnesses remain ignored); P13-05C policy tests cover CSRF, draft set/get/clear,
  stale PUT/DELETE and future-catalog rejection.
- `cargo check --locked --workspace --all-targets` and strict Clippy for touched crates passed.
- `npm --prefix web/prism run check`, `node scripts/check-management-spa.mjs`,
  `./scripts/check.sh docs`, `./scripts/check.sh fast`, `cargo fmt --all -- --check`,
  `git diff --check`, source-policy, crate-boundary and tracked-secret checks passed. The authoritative
  OpenAPI and Prism contract contain 82 operations and are byte-aligned.
- No Provider request, token-count request, production/server mutation, staging traffic, or formal
  Delivery Gate was run.

## Independent review

Independent review completed with no blocking findings after fixes for Prism PUT generation,
Route Explain lineage/evidence bounds, explicit nullable disabled policy state, and runtime/HTTP
future-catalog coverage. The remaining non-blocking evidence gaps are a dedicated malformed/missing
catalog-row composition fixture and active/archived/unknown-catalog HTTP permutations; Store/compiler
and service-level fail-closed tests already cover those invariants, and SQLite foreign keys prevent a
normal persisted policy from referencing a missing catalog.

## Boundary and next step

P13-07D is locally complete but remains `LOCAL_PASS_PENDING_PHASE_GATE`. P13-07 and the P13 umbrella
remain `IN_PROGRESS` until one separately authorized phase Gate decision. No staging canary, Provider
probe, production mutation, P13-08, P13-11 or P13-12 is started by this receipt.
