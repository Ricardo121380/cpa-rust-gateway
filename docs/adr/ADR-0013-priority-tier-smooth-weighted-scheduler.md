# ADR-0013: Priority-tier bounded smooth weighted scheduler

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-20` |
| Task / Matrix / Contract references | `P3-03`; `L24`, `L25`; [BC-SCHEDULER-001](../contracts/BC-SCHEDULER-001-priority-tier-smooth-weighted-scheduler.md) |

## Context

P2-07 publishes a secret-free immutable `RouteSnapshot`. Its Candidates already carry a
lower-is-better `priority`, a positive `weight`, and one of the three persisted policies
`round_robin`, `smooth_weighted_round_robin`, or `priority_failover`. It deliberately did not
select a Candidate at runtime. P3 needs selection that is deterministic, bounded, and safe under
concurrent requests without adding a Store query or a global scheduling mutex to the hot path.

Weights originate in persisted configuration, so expanding a schedule directly from an unbounded
weight would make Snapshot publication and request selection vulnerable to allocation or scan
amplification. P3-03 must reject unsafe plans before a new Snapshot becomes active. It must also
leave Credential leasing, live health, cooldown, circuits, attempts, retries, and transport to
their assigned later tasks.

## Decision

- `gateway-router::RouteSnapshot::try_new` precompiles a `SnapshotRouteSchedule` for every Route.
  Tiers are ordered by ascending priority; tie order is the stable Candidate ID order, not input
  insertion order. A plan is immutable with its Snapshot and is never constructed during a
  selection.
- `round_robin` and `priority_failover` build one ID-ordered slot per Candidate within a tier.
  `smooth_weighted_round_robin` uses the standard smooth weighted recurrence and emits one slot per
  unit of weight. All policies first prefer the lowest eligible priority tier; lower tiers are
  considered only after no Candidate in every higher tier slot is eligible.
- Every tier has a maximum of `1024` schedule slots. A negative priority, a non-positive weight,
  conversion overflow, or a tier above that bound rejects `RouteSnapshot` construction. The
  existing atomic Snapshot publication flow consequently retains the prior active version on a
  rejected plan.
- `RouteCandidateScheduler` owns a fresh `AtomicUsize` cursor for each `(Route, priority tier)`
  of one `Arc<RouteSnapshot>`. Each call uses `fetch_add(Relaxed)`, a bounded scan of the immutable
  slots, and a caller-provided eligibility predicate. The predicate boundary permits later health
  and Credential filters without making P3-03 own their mutable state.
- A selection returns a cloned non-secret `SnapshotRouteCandidate` or no Candidate. It exposes no
  Store, Credential, endpoint secret, HTTP, response, retry, or observability behavior.

## Consequences

Every request that shares one scheduler sees an independent atomic cursor position, so a complete
cycle preserves configured weights even when callers race. A newly published Snapshot receives a
new scheduler and cursor set, keeping configuration versions and their schedule plans isolated.
The route plan is finite, and a failed new plan cannot partially replace the active Snapshot.

P3-04 will turn the chosen Candidate into a Credential lease. P3-05 will supply real runtime
eligibility for cooldown/circuit state. P3-06 will add exclusion sets, attempts, and retry/failover
semantics around this primitive. P3-03 itself does not claim that an eligible Candidate can execute
a request.

## Alternatives considered

- Recomputing smooth weighted scores on each request was rejected because it adds mutable score
  state and per-request work proportional to every Candidate.
- A global `Mutex` around one Route or one scheduler was rejected because concurrent unrelated
  requests would serialize on the hot path.
- Repeating a Candidate `weight` times without a hard bound was rejected because configuration can
  otherwise cause unbounded memory and scan cost at publication or selection.
- Choosing from all priorities at once was rejected because it violates the fixed rule that lower
  priority values must be exhausted before a lower-preference tier participates.
- Adding Credential or health ownership here was rejected because it would collapse P3-04/P3-05
  boundaries and let Key-count or transient-state logic distort the route-level weight plan.

## Validation and rollback

Unit tests prove stable smooth-weighted distribution, equal round-robin despite unequal stored
weights, priority fallback only after an eligibility predicate rejects every higher-tier Candidate,
and exact aggregate weighted distribution under concurrent callers. Snapshot tests reject negative
priority, non-positive weight, and over-limit tier plans. `gateway-control` publication tests still
prove that malformed candidate data leaves the active Snapshot unchanged.

Rolling back removes the immutable schedule plans and the process-local atomic cursors only. It
does not migrate SQLite, alter Client Keys or Credentials, contact an upstream, change transport
behavior, or persist cursor/health state.
