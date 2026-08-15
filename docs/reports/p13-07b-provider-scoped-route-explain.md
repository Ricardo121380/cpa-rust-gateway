# P13-07B Provider-scoped Route Explain composition report

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

## Objective

Connect the reviewed P13-07A selector to the existing runtime/scheduler observations without
creating a second lease owner or changing the serving request path.

## Delivered

- shared `RouteCredentialScheduler` read-only composition method;
- expiry-aware base Route Explain credentials (`expiry <= observed_at` is excluded);
- bounded Provider-scoped Route Explain input/snapshot and deterministic selection;
- exact Provider identity from Candidate `upstream_id`;
- known-vs-unknown quota projection and optional cost map with no inferred prices;
- optional `provider_id` Route Explain query scope, single-Provider inference, and fail-closed
  multi-Provider `provider_scope_required` behavior;
- safe `provider_mismatch` projection without changing the response object shape;
- authoritative OpenAPI → Prism contract synchronization and boundary log for Claude Code.

## Explicit non-goals

The serving `AttemptOrchestrator` still owns request-time cursor advancement, lease acquisition,
retry/max-attempts, first-semantic-event handling and final Health/Quota revalidation. This slice
does not call Providers, refresh credentials, use a proxy pool, modify production, or run the
formal Delivery Gate.

## Verification

| Check | Result |
|---|---|
| `cargo test --locked -p gateway-router --all-targets` | PASS — 113 passed |
| `cargo test --locked -p gateway --all-targets` | PASS — 100 passed (99 unit + 1 component smoke) |
| `cargo test --locked -p gateway-http-actix --tests -- --test-threads=1` | PASS — 117 passed, 4 ignored (explicitly gated live/soak tests) |
| `cargo check --locked -p gateway-http-actix --benches` | PASS — benchmark targets compile without running a live workload |
| `cargo clippy --locked -p gateway-router -p gateway-http-actix -p gateway --all-targets --all-features -- -D warnings` | PASS |
| `npm --prefix web/prism run sync-contract` followed by `npm --prefix web/prism run check` | PASS — authoritative OpenAPI and Prism contract/client synchronized |
| `npm --prefix web/prism run build` | PASS |
| `./scripts/check-source-policy.rb` and `./scripts/check-crate-boundaries.rb` | PASS |
| `CHECK_REPORT_PATH=/tmp/cpar-p13-07b-docs-20260815.md ./scripts/check.sh docs` | PASS — links 519, contract refs 107, plan state 129 tasks / 1 IN_PROGRESS, tracked secret scan and whitespace clean |
| `git diff --check` | PASS |

The first exploratory `cargo test ... --all-targets -- --test-threads=1` invocation was not used as
evidence because Cargo forwarded the test-only flag to the Criterion benchmark binary. The formal
test result above uses `--tests` with serial threads and separately compiles benchmarks without
executing them.

No Provider request, production/server change, credential refresh, lease acquisition, staging
canary, or formal P13 Delivery Gate was performed.
