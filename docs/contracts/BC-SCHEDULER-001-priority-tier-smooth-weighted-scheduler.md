# BC-SCHEDULER-001 Priority-tier bounded smooth weighted scheduler

| Field | Value |
|---|---|
| Contract | `BC-SCHEDULER-001` |
| Task | `P3-03` |
| Status | DONE |
| Domain | Immutable Route scheduling plans and lock-free Candidate cursor selection |

## Entry and boundary

`RouteSnapshot::try_new` accepts only a complete compiler-approved Snapshot input. For every Route
it creates an immutable `SnapshotRouteSchedule`; `RouteCandidateScheduler::new` then receives one
`Arc<RouteSnapshot>` and creates a process-local atomic cursor per `(Route, priority tier)`.
`select` accepts a Route ID; `select_eligible` additionally accepts a predicate over a non-secret
`SnapshotRouteCandidate`.

This contract does not authenticate a Client Key, resolve a Public Model, choose or lease a
Credential, query SQLite, mutate health/cooldown/circuit state, start an HTTP request, classify a
response, retry, fail over, emit events, or publish `/v1/models`. Those behaviors remain P2/P3-04
through P3-10 work.

## Preconditions

- The scheduler is constructed from one successfully built immutable `RouteSnapshot`; callers do
  not reuse it for a different Snapshot version.
- Every Candidate priority is non-negative, and every weight is positive.
- Each priority tier has at most `1024` compiled slots. `round_robin` and `priority_failover` use
  one slot per Candidate; `smooth_weighted_round_robin` uses the sum of the tier weights.
- Candidate IDs are unique in the Snapshot. Tie ordering is the exact stable Candidate ID order.
- The eligibility predicate is pure with respect to this scheduler. It may reject Candidates, but
  P3-03 neither observes nor mutates Credential, health, circuit, quota, cooldown, or concurrency
  state.

## Required scheduling behavior

| Concern | Required behavior |
|---|---|
| Compilation | Build all tier slot sequences during Snapshot construction, never during `select`. |
| Priority | Inspect tiers from lowest numeric priority upward. Do not enter a lower-preference tier while any Candidate in a higher tier passes the predicate. |
| `round_robin` | Cycle all Candidates in the selected tier equally, ignoring relative weights after validating they are positive. |
| `smooth_weighted_round_robin` | Use a deterministic smooth weighted sequence; in one complete cycle, every Candidate appears exactly its positive weight count. |
| `priority_failover` | Use stable ID order inside the highest available tier; only an ineligible higher tier permits lower-tier selection. |
| Concurrency | Advance the one tier cursor with `AtomicUsize::fetch_add(Ordering::Relaxed)`; do not use a global scheduling lock. |
| Failure | Return no Candidate if the Route is unknown, the precompiled plan/cursor is inconsistent, or no Candidate satisfies the predicate. |

## Invariants

- A tier plan is finite: it cannot exceed `1024` slots, and no request can scan more slots than that
  tier's precompiled length.
- An invalid tier/weight or an oversized smooth-weighted plan rejects the entire candidate Snapshot
  before publication; it cannot become a partial active Route.
- Cursor state is local to the scheduler and Snapshot version. It is not persisted, shared across
  published Snapshot versions, or exposed through `Debug` as a Candidate selection history.
- When all Candidates are eligible, a full smooth-weighted cycle has the configured exact weight
  distribution. Atomic cursor increments make concurrent calls occupy distinct logical positions in
  that cycle.
- Route-level scheduling is independent of the number of Credentials bound to an Endpoint. P3-04
  owns the separate within-endpoint Credential scheduler.
- No selector call reads SQLite/YAML, creates a network connection, leaks a Secret, or calls a
  global application `Mutex`.

## Error semantics

| Condition | Result |
|---|---|
| Negative priority | `RouteSnapshotBuildError::InvalidCandidatePriority` |
| Zero or negative weight | `RouteSnapshotBuildError::InvalidCandidateWeight` |
| Weight conversion/accumulation overflow or more than 1024 tier slots | `RouteSnapshotBuildError::RouteScheduleTooLarge` |
| Unknown Route, inconsistent plan/cursor, or no eligible Candidate | `None`; no diagnostic containing Candidate configuration values |

## Corresponding tests

- `gateway-router::route_scheduler::tests` proves smooth-weighted distribution, equal round-robin,
  strict priority fallback, and 8 concurrent callers retaining the exact `5:1:1` aggregate ratio.
- `gateway-router::route_snapshot::tests::rejects_invalid_or_unbounded_candidate_schedules` proves
  negative priority, non-positive weight, and `1025` smooth slots fail closed before publication.
- `gateway-control` tests continue to cover publication atomicity and failure retention around
  `RouteSnapshot::try_new`.
