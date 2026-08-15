# P13-07C Provider-scoped serving lease revalidation report

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

## Objective

Connect the reviewed P13-07A Provider-scoped selector to the real request-serving lease path. The
selector supplies deterministic candidate order, while the existing `RouteCredentialScheduler`
performs fresh exact Health/Quota, expiry, binding and capacity checks immediately before a driver
starts. This slice must preserve the existing AttemptOrchestrator retry, first-semantic-event,
timeout, cancellation and Config Version boundaries.

## Frozen scope

- only a route whose admitted candidates belong to one Provider may use selector-driven serving;
- multiple Providers without an explicit internal scope fail closed before lease/driver/Provider;
- selector output is advisory and never a lease or reservation;
- lease-time checks use the same scheduler, credential pools and Health/Quota registries as serving;
- a lease race may advance to the next ranked candidate in the same Provider without consuming an
  attempt or widening `max_attempts`;
- no cross-Provider fallback, implicit credential conversion, refresh/reauth or proxy-pool action;
- public Chat/Responses/Messages, management OpenAPI, Prism and frontend shapes are unchanged;
- no Provider request, production/server mutation or formal P13 Delivery Gate.

## Delivered implementation

- `RouteCredentialScheduler::select_provider_scoped_and_lease_at` resolves every advisory ranked
  candidate again from the pinned RouteSnapshot, verifies Provider/channel/candidate ownership and
  performs the existing exact binding, expiry, Health, Quota and atomic-capacity checks;
- `AttemptOrchestrator::start_with_event_sink_provider_scoped_matching` derives one Provider only
  from admitted hard-eligible candidates, rebuilds the bounded selector observation on every
  pre-lease iteration and keeps the existing retry/max-attempts/FSE/cancellation/lease owner;
- a multi-Provider admitted route without an explicit scope returns the existing value-free
  `CredentialUnavailable/Credential` before a lease, Attempt record or driver invocation;
- `P12RoutedResponsesExecutor` uses the Provider-scoped entrypoint for Chat/Responses/Messages and
  rechecks the pinned Config Version after the async future starts, immediately before routing and
  lease work;
- selector observations remain read-only and secret-free. They do not advance the legacy route or
  credential cursor, read SQLite, contact a Provider, refresh a credential or create a second pool;
- versioned cost remains `Unknown` in the serving composition because P13-07C does not inject the
  P13-05 catalog. This slice therefore preserves correct unknown-cost ordering but does not claim
  that configured catalog prices influence serving yet.

## Required evidence

| Area | Result |
|---|---|
| Provider scope | PASS — same-Provider retry stays scoped; ambiguous multi-Provider routes fail before lease/driver/Attempt record; ranked Provider/channel identity is rechecked |
| Lease race | PASS — post-selection saturation advances to the same-Provider sibling; post-selection expiry, endpoint cooldown and exact binding quota all fail closed with zero leaked leases |
| Retry semantics | PASS — the existing AttemptOrchestrator state machine remains the only owner of attempt budget, exact exclusions, pre-semantic retry and post-semantic no-retry |
| Snapshot safety | PASS — executor rechecks the pinned Config Version inside the async serving future at the request-start boundary; later publication retains the existing pinned in-flight semantics; selector reads only immutable route/pool diagnostics and shared runtime registries |
| Ownership | PASS — no selector lease or second pool exists; existing cancellation, timeout and driver-drop tests remain green |
| Secrecy | PASS — no endpoint URL, credential material, client-key digest, headers, body, raw Provider response, quota window or new public response field was added |

## Verification

| Check | Result |
|---|---|
| `cargo test --locked -p gateway-router --lib -- --test-threads=1` | PASS — 121 passed |
| `cargo test --locked -p gateway --bin gateway -- --test-threads=1` | PASS — 100 passed |
| `cargo test --locked -p gateway-http-actix --tests -- --test-threads=1` | PASS — 116 passed, 4 explicitly gated tests ignored |
| `cargo clippy --locked -p gateway-router -p gateway --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `./scripts/check-source-policy.rb` and `./scripts/check-crate-boundaries.rb` | PASS — 219 Rust files / 21 crate roots / 21 workspace packages |
| `git diff --check` | PASS |

The first combined race test exposed two test-only issues during development: an absolute cooldown
deadline was paired with the system clock, and the combined test exceeded strict Clippy's line
limit. The final evidence uses an injected fixed clock and three focused expiry/Health/Quota tests;
the failed exploratory runs are not counted as passing evidence.

Independent review initially found that the legacy quota-recovery fallback did not receive the
inferred Provider scope. The final patch passes that scope into recovery admission and requires
hard eligibility plus exact Candidate `upstream_id` → `ProviderId` ownership before any recovery
lease. A foreign hard-ineligible recovery-due fixture now proves zero lease/zero driver, while a
same-Provider fixture proves controlled recovery still succeeds. The review was rerun and reported
no remaining blocker.

## Boundary and next slice

P13-07C is locally complete and remains `LOCAL_PASS_PENDING_PHASE_GATE`; no formal P13 Delivery Gate
or staging canary was run. P13-07 remains `IN_PROGRESS`: before phase closeout, a narrow follow-up
must decide and implement the versioned P13-05 catalog-to-runtime cost projection so configured
prices are not permanently represented as `Unknown`. A bounded staging canary may be evaluated only
after that boundary and the P13 phase rules are reviewed. Do not start P13-08/11/12 from this report.
