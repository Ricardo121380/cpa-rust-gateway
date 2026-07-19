# ADR-0016: Request-scoped Attempt orchestration and transparent-retry gate

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-20` |
| Task / Matrix / Contract references | `P3-06`; `A22`, `E11`, `E12`, `E15`, `E16`, `G21`, `K03-K06`, `L20-L26`, `L30`; [BC-ROUTER-003](../contracts/BC-ROUTER-003-request-scoped-attempt-orchestration.md) |

## Context

P3-02 can make exactly one DNS-pinned HTTP attempt, P3-04 can acquire an Endpoint-local
Credential lease, and P3-05 can exclude transiently unavailable Endpoint or
Endpoint/Credential keys. None of those components owns the lifetime of a request across more
than one attempt. In particular, retrying a failed binding without remembering it can consume the
same Candidate/Credential pair repeatedly; treating a 429 as Endpoint-wide can unnecessarily
disable healthy sibling Credentials; and retrying after any client-visible semantic output can
duplicate a response.

The transport and Provider decoder are deliberately not complete in P3-06. The missing boundary
must therefore be a narrow, testable router primitive that accepts a non-secret attempt driver,
holds a lease through a successful output's lifetime, and makes its retry decision only from
classified safe failures plus downstream cancellation/first-semantic-event state.

## Decision

- `gateway-router::AttemptOrchestrator` owns one request-scoped loop. It reads the selected
  immutable `SnapshotRoute` through its matching `RouteCredentialScheduler`, applies the Route's
  positive `max_attempts` and cumulative `bootstrap_timeout_ms`, and never reads SQLite, rebuilds
  a Snapshot, creates an unbounded queue, or takes a global request-path lock.
- `AttemptExclusionSet` retains only non-secret `(RouteCandidateId, CredentialId)` bindings
  attempted by this external request. `RouteCredentialScheduler` gains a Candidate-plus-Credential
  predicate that runs before a pool capacity CAS, so a retry cannot reacquire the same binding.
  It does not globally exclude an Endpoint or a Credential: P3-05 runtime health still provides
  the correct Endpoint and Endpoint/Credential scope.
- `AttemptDriver` is an async, non-secret port. It borrows a selected Candidate and live
  Credential lease only while starting one attempt; on success the orchestrator returns a
  `StartedAttempt<T>` that owns the selection and thus retains the lease until the caller drops the
  output wrapper. The driver reports only `Connection`, `RateLimited`, `ServerError`,
  `BootstrapTruncated`, `Cancelled`, or a safe non-retryable `GatewayError`; it does not expose
  raw status bodies, URLs, Authorization values, or Secrets.
- Retry budget is checked before every selection and after each failed driver call. It limits both
  total attempts and the cumulative pre-first-semantic-event bootstrap interval. An exhausted
  budget returns the last safe attempt failure; no wait queue or sleep-until-cooldown behavior is
  introduced.
- A 429 records an Endpoint/Credential Cooldown, using a driver-supplied retry-after duration when
  present and a finite fallback otherwise. Connection, 5xx, and pre-semantic truncation record an
  Endpoint Cooldown. P3-06 does not automatically open or recover a Circuit, persist health,
  classify account/quota/403 state, or run a probe; P4 owns those policy transitions.
- `gateway-core::TransparentRetryGate` is a transport-neutral port rather than a dependency from
  `gateway-router` to the stream crate. `StreamControl` implements it: transparent retry is allowed
  only while the request is not cancelled and no semantic event has been delivered to the client;
  its cancellation Future lets the orchestrator drop an in-flight driver Future promptly. After a
  bridge hands its first canonical semantic event to the bounded output, it must wait for
  first-semantic-event delivery or cancellation before pulling later upstream output; this prevents
  an undispatched queued start from being followed by a retried duplicate start.

### Implementation sequence

1. Extend two-stage selection with a non-secret binding predicate and retain the selected Route
   configuration from the same scheduler Snapshot.
2. Add the bounded Attempt loop, exclusion set, safe failure classifier, cooldown mapping, lease
   ownership wrapper, and deterministic injected clock.
3. Extend stream control with a first-semantic-event-or-cancellation wait primitive, then verify
   that a committed semantic event and cancellation both close the transparent-retry gate.
4. Cover connection, 429 with a healthy sibling Credential, 5xx fallback, pre-semantic
   truncation, budget exhaustion, cancellation, and post-FSE failure without contacting a real
   Endpoint.

## Consequences

The request path can fall through to a healthy binding before any client-visible semantic output,
while retaining per-Endpoint concurrency through the exact successful attempt. A 429 does not
make `/v1/models` flap or disable a healthy sibling Credential. A successful traffic attempt does
not silently close a Circuit; P4's controlled recovery remains authoritative.

The generic driver deliberately stops before OpenAI Responses response decoding, direct
`UpstreamClientPool` construction, response-model rewrite, structured Attempt records, mock HTTP
E2E, or real-endpoint verification. Those belong respectively to the remaining P3 tasks and must
adapt this port rather than reimplement retry policy.

## Alternatives considered

- Retrying inside `UpstreamClientPool` was rejected because it cannot see Route policy,
  Credential leases, runtime health, output delivery, or cross-Endpoint exclusions.
- Recording only a Candidate exclusion was rejected because one Endpoint can have several healthy
  Credentials and a retry must be able to select a sibling without reacquiring its failed binding.
- Recording only a Credential exclusion was rejected because a Credential can be valid at another
  Endpoint and Candidate policy is the route-level identity of an attempt.
- Letting each Provider own retries was rejected because it would duplicate FSE, budget, and
  health semantics across protocol adapters and permit inconsistent failover behavior.
- Coupling `gateway-router` directly to `gateway-stream` was rejected to preserve the current
  dependency direction. A small core port implemented by StreamControl plus an explicit stream-side
  wait retains the same behavior without a reverse dependency.

## Validation and rollback

Focused tests prove binding exclusion before a new lease, total/cumulative budget handling, exact
429 Credential isolation, Endpoint failure fallback, lease release on failed/cancelled attempts,
no retry after FSE, and no retry after cancellation. Fast/Full gates, crate-boundary checking,
document links, whitespace checks, and secret scans provide the remaining evidence.

Rollback removes the request-scoped orchestrator, scheduler predicate, and stream wait helper. It
does not change RouteSnapshot data, SQLite schema, encrypted Credentials, egress admission, HTTP
client pooling, durable health, Provider protocol decoding, or deployed Endpoint state.
