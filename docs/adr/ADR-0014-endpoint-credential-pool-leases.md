# ADR-0014: Endpoint Credential pool leases

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-20` |
| Task / Matrix / Contract references | `P3-04`; `D12`, `D14`, `D17`, `E16`, `K06`, `L26`; [BC-CRED-001](../contracts/BC-CRED-001-endpoint-credential-pool-leases.md) |

## Context

P3-03 supplies a secret-free, immutable Route Candidate plan with an atomic Candidate cursor. A
Candidate fixes an Endpoint and upstream model but deliberately carries only the count of active
Credential bindings. The next scheduling stage must choose one Endpoint-bound Credential without
letting a site with many keys receive more Route-level traffic than its Candidate weight permits.

Persisted Credential material is AEAD-encrypted and bound to its Config Version, Credential ID,
and Upstream ID. It cannot be read on the inference hot path. At the same time, runtime concurrent
request capacity is mutable: every successful acquisition must be released on normal completion,
error, or cancellation, without a global request-path lock.

## Decision

- `gateway-control::CredentialPoolCompiler` is a management/control-path bridge. It validates
  Endpoint/Credential/Upstream ownership, ignores administratively inactive records, reconstructs
  the existing length-delimited Credential AAD, decrypts only active bindings, and returns a
  complete `gateway-upstream::EndpointCredentialPools` value. It retains no plaintext after pool
  construction and never gives a Repository or Secret Store to request code.
- Each `EndpointCredentialPool` owns an independent immutable Credential entry set. Entries sort
  by Credential ID, group by lower-is-better binding priority, and precompile a smooth-weighted
  schedule inside each tier. A tier is limited to `1024` slots; empty input, duplicate IDs,
  malformed revision/priority/weight/concurrency, and unbounded schedules fail closed.
- Every pool tier owns its own `AtomicUsize` cursor. Each Credential slot owns its own atomic
  active-lease counter and uses compare-and-exchange to reserve capacity up to its configured
  concurrency limit. A saturated slot is skipped; a lower priority tier is considered only after
  its higher tier cannot acquire any bounded schedule slot.
- `CredentialLease` is non-cloneable and owns precisely one successful reservation. Its `Drop`
  releases the counter, so cancelling a request Future releases capacity. `release(self)` consumes
  the lease for an explicit early boundary. Decrypted bytes remain zeroizing and redacted; only a
  live lease may borrow them for a later Provider request builder.
- `gateway-router::RouteCredentialScheduler` composes P3-03 Candidate selection with pool lease
  acquisition. It first chooses a Candidate according to Route priority/weight, then uses that
  Candidate's Endpoint pool. Candidate and Credential cursors remain separate, preserving route
  weights regardless of the number or weights of Credentials at any Endpoint.

## Consequences

The request path gains a bounded two-stage selector with no SQLite query, global scheduler mutex,
or Secret logging. A matching pool set is built on the control path alongside the validated
configuration and handed to the route scheduler. An Endpoint without a matching available pool is
treated as unavailable; the result is the safe `CredentialUnavailable/Credential` classification.

The pool does not persist cursor or active-lease state. A process restart begins with empty active
counts and fresh cursors, while durable Credential revision and long-lived state remain in the
control plane. P3-05 will add dynamic health/cooldown/circuit eligibility; P3-06 will own Attempt
exclusions, retry budget, first-semantic-event failover, Provider dispatch, and transport lifetime.

## Alternatives considered

- Selecting directly from all Credentials across all Endpoint Candidates was rejected because it
  makes an Endpoint's key count distort its Route-level traffic share.
- Querying and decrypting SQLite rows during every request was rejected because it violates the
  immutable Snapshot/no-database hot-path baseline and expands Secret exposure.
- A global mutex or semaphore around every Credential was rejected because unrelated Endpoints
  would serialize. Per-slot atomics provide the required capacity boundary without cross-Endpoint
  contention.
- A manually released boolean lease was rejected because cancellation can bypass explicit cleanup.
  RAII ownership makes release unconditional while retaining an explicit consuming `release` API.
- Adding health, quota, cooldown, circuit, retry, HTTP, or response classification here was
  rejected because those states belong to P3-05/P3-06 and would conflate a capacity lease with an
  Attempt lifecycle.

## Validation and rollback

Focused tests prove exact within-Endpoint weighted cycles, priority fallback on saturation,
concurrent limit enforcement, capacity restoration after drop/explicit release, AEAD/AAD control
path compilation, malformed inactive graph and duplicate-binding rejection, inactive Credential
exclusion, and concurrent `3:1` Route plus `1:1` Endpoint weight preservation. Debug checks prove
synthetic secret text is not rendered.

Rollback removes the pool compiler, runtime pool, and two-stage scheduler only. It changes no
SQLite migration, encrypted record schema, Client Key, live Endpoint, HTTP request, health state,
retry policy, or production secret.
