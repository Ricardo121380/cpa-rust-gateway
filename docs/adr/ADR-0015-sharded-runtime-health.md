# ADR-0015: Sharded runtime health state

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-20` |
| Task / Matrix / Contract references | `P3-05`; `E08`, `E11`, `E12`, `K06`, `L30`; [BC-HEALTH-001](../contracts/BC-HEALTH-001-sharded-runtime-health.md) |

## Context

P3-04 provides immutable route Candidate schedules and Endpoint-local Credential leases, but it
deliberately has no mutable availability state. A transient block must be visible to the next
selection without querying SQLite, recompiling `RouteSnapshot`, taking one global scheduler lock,
or letting a failure at one protocol-specific Endpoint disable another Endpoint or a healthy sibling
Credential.

The next task owns HTTP dispatch, status-code/error classification, retry budgets, attempt
exclusions, and first-semantic-event behavior. P4 owns active health probes, EWMA, model health,
and half-open Circuit recovery. P3-05 therefore needs a narrow, reusable in-memory state primitive
and a way for the existing two-stage scheduler to consult it before leasing a Credential.

## Decision

- `gateway-router::RuntimeHealthRegistry` owns process-local state keyed either by `EndpointId` or
  by an `(EndpointId, CredentialId)` pair. Endpoint state is protocol-specific; Credential state is
  Endpoint-local, so a shared Credential's transient failure at one Endpoint does not contaminate a
  different Endpoint.
- The registry uses a fixed default of `64` independently locked `RwLock` shards. Shard counts must
  be positive powers of two and no greater than `1024`; every shard retains at most `1024` entries.
  A request-time availability lookup reads exactly one deterministic shard. A clock or lock failure
  fails closed for scheduling.
- P3-05 records only `CoolingDown { until_ms }` and `CircuitOpen { retry_after_ms }`. A Cooldown
  becomes eligible when its deadline passes. A Circuit remains unavailable after its earliest
  recovery instant until an explicit `mark_healthy` result; P4 will add controlled half-open probes
  and richer recovery policy. Longer deadlines never shorten an existing block, and Circuit state
  outranks a Cooldown.
- `RuntimeHealthClock` is injectable for deterministic tests. Runtime state is non-secret, has no
  persistence, and is removed on explicit successful recovery. Expired Cooldowns are reclaimed only
  on a bounded full-shard insertion; no global cleanup scan occurs on the request path.
- `EndpointCredentialPool::try_lease_eligible` accepts only a stable Credential ID predicate before
  a CAS capacity reservation. `RouteCredentialScheduler` first filters Endpoint state, then uses
  that predicate inside the selected Endpoint pool. Thus one cooled Credential is skipped while its
  healthy sibling can retain the Candidate's configured route share.

## Consequences

Short 429/transient cooling can be excluded at runtime without hiding a hard-eligible Public Model
from `/v1/models` or mutating the immutable `RouteSnapshot`. Endpoint and Credential filters are
bounded by the precompiled Candidate/Credential schedules and cannot expose IDs or Secrets through
the existing `CredentialUnavailable/Credential` selection error.

The registry purposefully does not decide what failure should cause a Cooldown or Circuit transition.
P3-06 will classify Attempt outcomes and invoke these mutation APIs; P4 will add active probes,
EWMA, model-level state, and half-open Circuit recovery. Nothing in this task writes SQLite, sends
HTTP, queues retries, changes durable Credential status, or contacts an Endpoint.

## Alternatives considered

- One global `Mutex<HashMap<...>>` was rejected because all Endpoint health reads would serialize
  and violate the required no-global-lock hot-path boundary.
- Storing transient state in `RouteSnapshot` was rejected because Cooldown/Circuit changes would
  require recompilation/publication and make model visibility flap.
- Storing only a Credential-global key was rejected because the same Credential may bind to multiple
  protocol-specific Endpoints whose failures must remain isolated.
- Automatically closing a Circuit after `retry_after_ms` was rejected because P4 owns controlled
  probe/half-open recovery; P3-05 requires explicit successful recovery instead.
- Adding response status classification, retry/failover, quota, probes, or persistence was rejected
  because it would cross the P3-06/P4 task boundary.

## Validation and rollback

Focused tests prove Endpoint and Credential isolation, Cooldown expiry, explicit Circuit recovery,
deadline monotonicity, Circuit precedence, finite shard construction/capacity reclamation,
fail-closed clock handling, and Candidate-plus-Credential filtering without a healthy sibling being
skipped. Clippy, source-policy, crate-boundary, Secret, Fast, and Full gates provide the remaining
evidence.

Rollback removes the runtime-health registry and the non-secret pool predicate only. It does not
change RouteSnapshot data, database schema, encrypted Credential material, HTTP transport,
Provider behavior, retry semantics, deployed Endpoint state, or production Secret.
