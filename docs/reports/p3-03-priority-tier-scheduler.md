# P3-03 Priority-tier smooth weighted scheduler report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-03` |
| Matrix / behavior | `L24`, `L25`; Behavior 3/20 |
| Date | `2026-07-20` |
| Branch | `codex/p3-03-priority-scheduler` |
| Rust | `1.97.1` |
| Result | PASS locally; GitHub Fast/Full acceptance pending |

## Delivered scope

- Added immutable per-Route `SnapshotRouteSchedule` plans built only during
  `RouteSnapshot::try_new`. Plans group Candidates by lower-is-better priority and apply stable
  Candidate-ID tie ordering.
- Added finite precompilation with at most `1024` slots per tier. Round-robin/priority-failover
  plans use one slot per Candidate; smooth weighted plans use a standard smooth recurrence and one
  slot per configured weight unit.
- Added fail-closed Snapshot errors for negative priority, non-positive weight, conversion/total
  overflow, and oversized tier plans. Existing atomic publication retains the prior Snapshot if a
  new plan fails to build.
- Added `RouteCandidateScheduler`, which holds a Snapshot and one `AtomicUsize` cursor per Route
  tier. `select_eligible` lets a later task inject availability filtering without giving P3-03
  ownership of Credentials or health state.
- Added stable weighted, equal round-robin, priority fallback, concurrent `5:1:1` fairness, and
  invalid-plan tests, along with [ADR-0013](../adr/ADR-0013-priority-tier-smooth-weighted-scheduler.md)
  and [BC-SCHEDULER-001](../contracts/BC-SCHEDULER-001-priority-tier-smooth-weighted-scheduler.md).

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-router` | PASS; 14 tests including plan bounds, weighted distribution, priority fallback, and concurrent cursor fairness |
| `cargo test --locked -p gateway-router -p gateway-control` | PASS; Snapshot publication path and route scheduler integration compile together |
| `cargo clippy --locked -p gateway-router -p gateway-control --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `ruby scripts/check-crate-boundaries.rb`, `ruby scripts/check-source-policy.rb`, and `ruby scripts/check-doc-links.rb` | PASS; 21 package boundaries, 49 Rust files, and 82 Markdown documents |
| `scripts/secret-scan.sh --all` and `git diff --check` | PASS |
| `cargo deny check` and `cargo audit` | PASS; duplicate-version notices are non-blocking and covered by the existing dependency-policy rule |
| `./scripts/check.sh fast` | PASS |
| `./scripts/check.sh full` | PASS |

## Review

Review passed. It verified that every Route's tier plan is constructed before its Snapshot is
visible, that candidate-ID sorting makes equal-score behavior independent of input order, and that
the smooth recurrence produces a stable exact `5:1:1` complete-cycle distribution. It verified
that each tier gets an independent atomic cursor, so concurrent callers occupy distinct logical
positions without a global scheduling lock, while a predicate cannot advance to a lower tier when
any higher-tier Candidate is eligible.

The review also verified fail-closed negative/zero/oversized schedule rejection, bounded scans of
at most 1024 slots, safe wrapping cursor arithmetic, and absence of SQLite, Credential lease,
health/circuit mutation, transport, response decoding, retry/failover, observability, deployed
Endpoint, Client Key, or Secret behavior. The extraction of `build_routes` was required by the
project's Clippy complexity rules; it keeps Snapshot validation and immutable schedule construction
explicit without weakening the publication boundary.

## Scope and deferred work

P3-03 does not select a Credential, acquire/release a lease, implement dynamic Endpoint/Credential
health or circuit state, decide runtime availability itself, construct an outbound request, contact
an upstream, retry/fail over, parse HTTP/SSE, publish `/v1/models`, or emit observability events.
P3-04 owns Credential pooling, P3-05 owns dynamic health/cooldown/circuit state, and P3-06 owns
attempt exclusions, retry budget, and FirstSemanticEvent failover. All tests use synthetic
Candidates and Snapshot data; no deployed endpoint, Client Key, Credential, Authorization value,
or production traffic was read, logged, or committed.

## GitHub CI

The implementation commit's GitHub Fast and Full gates must both pass before the P3-03 acceptance
record is finalized. Its separate verification-record commit must also pass the same workflow before
P3-04 can begin.
