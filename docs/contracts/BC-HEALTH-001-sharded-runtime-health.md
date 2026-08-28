# BC-HEALTH-001 Sharded runtime health state

| Field | Value |
|---|---|
| Contract | `BC-HEALTH-001` |
| Task | `P3-05` |
| Status | IN_PROGRESS |
| Domain | Endpoint/Credential transient availability, bounded sharded storage, and two-stage scheduling eligibility |

## Entry and boundary

`gateway-router::RuntimeHealthRegistry` is a process-local state primitive. Its only identities are
an `EndpointId` or an `(EndpointId, CredentialId)` binding pair. It does not receive a Repository,
`SecretStore`, `RouteSnapshot` mutation handle, HTTP client, Provider, response body, or retry
budget.

`gateway-router::RouteCredentialScheduler` consults Endpoint state before a Candidate's pool is
read. The selected `gateway-upstream::EndpointCredentialPool` then applies Endpoint/Credential
state to each bounded scheduling slot before its CAS lease acquisition. Existing caller predicates
remain available for P3-06 Attempt exclusions.

## Preconditions

- Runtime keys originate only from a validated Candidate Endpoint or an acquired Candidate/lease
  identity; they contain no Secret material.
- Cooldown and Circuit mutation deadlines are strictly later than the supplied runtime clock.
- Shard count is a positive power of two no greater than `1024`; every shard retains at most `1024`
  keys.
- Callers decide what observed outcome merits a mutation. P3-05 neither inspects HTTP status codes
  nor classifies 401/403/408/429/5xx, transport errors, Quota, or Provider results.

## Required behavior

| Concern | Required behavior |
|---|---|
| Endpoint isolation | An Endpoint state affects only Candidates for that exact Endpoint; one API Format must not affect a sibling Endpoint merely because it shares an Upstream. |
| Credential isolation | Endpoint/Credential state affects only that pair. A shared Credential's transient state at one Endpoint does not block another Endpoint or a healthy sibling Credential. |
| Cooldown | `CoolingDown { until_ms }` rejects selection strictly before the deadline and becomes eligible at/after it without Snapshot publication. A newer shorter deadline cannot shorten an existing Cooldown. |
| Circuit | `CircuitOpen { retry_after_ms }` rejects selection until explicit `mark_healthy`; its timestamp is only an earliest later recovery/probe instant. Circuit overrides a Cooldown and cannot be silently shortened. |
| Recovery | Explicit `mark_healthy` removes the key's transient state. P4 will implement half-open probes and automatic Circuit recovery. |
| Sharding | An availability lookup reads exactly one fixed deterministic `RwLock` shard. A mutation writes only that shard; no global lock, SQLite query, network operation, or unbounded queue/scan is permitted on selection. |
| Bounded storage | A new key is rejected when a live full shard is at capacity. On insertion, expired Cooldowns in that same full shard may be reclaimed; live Cooldowns and Circuit state are never evicted. |
| Scheduler integration | A Candidate with unavailable Endpoint state is skipped before pool access. Within a healthy Endpoint, the Credential predicate skips unavailable slots before CAS so another healthy Credential can lease normally. |
| Failure safety | Clock/shard lookup failures fail closed for scheduling. If no Candidate can satisfy health and lease availability, the existing `CredentialUnavailable/Credential` error exposes no Candidate, Endpoint, Credential, or Secret text. |

## Invariants

- Runtime state is mutable but neither durable nor part of an immutable `RouteSnapshot`; transient
  429/Cooldown/Circuit state cannot make `/v1/models` flap.
- `RuntimeHealthRegistry` has a redacted/non-secret `Debug` surface and no raw Credential,
  Authorization, ciphertext, request body, response body, or Provider diagnostic.
- The pool eligibility predicate sees only `CredentialId`; it runs before reservation and cannot
  leak a lease when it rejects a slot.
- P3-05 does not enqueue waiters, persist rows, mutate durable `CredentialStatus`, automatically
  retry/fail over, send a probe, construct/send HTTP, parse a response, or emit events.

## Error semantics

| Condition | Result |
|---|---|
| Unsafe shard count | Safe `RuntimeHealthRegistryBuildError`; no registry is constructed. |
| Clock unavailable, poisoned shard, non-future deadline, or full live shard | Safe `RuntimeHealthError`; scheduler treats availability lookup failure as unavailable. |
| Endpoint blocked, all Credentials blocked/saturated, unknown Route, or caller predicate rejection | `GatewayError(CredentialUnavailable, Credential)` with no runtime identity or Secret diagnostic. |
| Expired Cooldown | `RuntimeHealthAvailability::Available`; no error and no Snapshot mutation. |
| Expired Circuit timestamp without explicit recovery | `RuntimeHealthAvailability::CircuitOpen`; P4 owns later half-open/probe policy. |

## Corresponding tests

- `gateway-router::runtime_health::tests` proves Endpoint and Endpoint/Credential isolation,
  Cooldown expiry, explicit Circuit recovery, monotonic state, bounded shard validation, and
  expired-Cooldown-only capacity reclamation.
- `gateway-router::credential_scheduler::tests` proves a cooled Credential is skipped inside its
  Endpoint pool while a healthy sibling is leased, and an Endpoint Cooldown falls through to the
  next configured Candidate tier; it also proves an unavailable clock fails closed before a pool
  lease is acquired.
- `gateway-upstream::credential_pool::tests` proves the non-secret eligibility predicate skips a
  Credential before capacity reservation while retaining the existing pool scope and without
  incrementing the rejected Credential's lease count.
