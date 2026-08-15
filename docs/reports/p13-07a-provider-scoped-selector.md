# P13-07A Provider-scoped deterministic selector report

Status: `DONE_WITH_BOUNDARY`

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

## Boundary and phase closeout

P13-07A itself did not wire the selector into request-time candidate selection. The existing
`RouteCredentialScheduler` continued to own leases, Health/Quota reads, cursors, retry boundaries
and Provider execution; the later P13-07B through P13-07D slices composed and integrated this
policy without weakening that ownership. No Provider/network request, credential refresh,
production deployment or frontend contract change is attributed to P13-07A.

The complete P13-07 phase passed the local Full preflight (`43/43`) and the immutable
`phase-p13-routing-complete` Delivery Gate at commit
`0c338ee8eef76e470c55515a24728324684365c5`: [run 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495)
completed Authorize, Fast, Full supply-chain and Required successfully in `3s`, `5m57s`, `1m16s`
and `2s`. This closes P13-07A as `DONE_WITH_BOUNDARY`; it does not claim Provider traffic,
staging/production mutation or the start of P13-08.
