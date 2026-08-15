# P13-07A Provider-scoped deterministic selector report

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

## Objective

Introduce the first bounded P13-07 routing policy slice without replacing the existing request
scheduler. The policy ranks caller-supplied Provider-scoped candidate observations for
cost-aware/fill-first/least-loaded behavior and emits safe exclusion reasons.

## Delivered

- `gateway-router::ProviderScopedSelector` and secret-free candidate/decision types;
- exact Provider scope with hard cross-Provider rejection;
- explicit Health, Quota, capability, expiry and concurrency rejection matrix;
- known-vs-unknown cost/quota ordering with no zero/unlimited substitution;
- overflow-safe load-ratio comparison and deterministic channel/candidate tie-breakers;
- finite candidate/identity bounds plus whitespace-only and duplicate-identity rejection;
- no OpenAPI, Prism, production or serving-path changes.

## Focused verification

| Check | Result |
|---|---|
| `cargo test --locked -p gateway-router provider_scoped_selector --lib` | PASS (9 tests) |
| `cargo test --locked -p gateway-router --all-targets` | PASS (110 tests) |
| `cargo clippy --locked -p gateway-router --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` and `git diff --check` | PASS |
| `scripts/check-source-policy.rb` and `scripts/check-crate-boundaries.rb` | PASS |
| `scripts/check.sh docs` | PASS (516 Markdown files; tracked-secret scan included) |

## Boundary and next slice

The selector is not yet wired into request-time candidate selection. The existing
`RouteCredentialScheduler` continues to own leases, Health/Quota reads, cursors, retry boundaries
and Provider execution. No Provider/network request, credential refresh, production deployment,
or frontend contract change occurred. The next P13-07 slice can add a composition adapter and
Route Explain projection only after the policy is reviewed against the existing scheduler matrix.
