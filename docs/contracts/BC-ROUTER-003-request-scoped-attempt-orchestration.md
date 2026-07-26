# BC-ROUTER-003 Request-scoped Attempt orchestration

| Field | Value |
|---|---|
| Contract | `BC-ROUTER-003` |
| Task | `P3-06` |
| Status | IN_PROGRESS |
| Domain | Request-local Attempt selection, binding exclusion, bounded transparent retry, and first-semantic-event gating |

## Entry and boundary

`gateway-router::AttemptOrchestrator` receives a P3-04 `RouteCredentialScheduler`, a P3-05
`RuntimeHealthRegistry`, an injected non-secret clock, and one `AttemptDriver` invocation for each
selected binding. The scheduler and Route configuration originate from the same immutable
Snapshot. The orchestrator neither reads a Repository nor accepts a Secret Store, raw HTTP body,
URL, Authorization value, Provider implementation, or database handle.

The driver may start a P3-02 transport or a later Provider decoder, but returns only a typed
output or a safe `AttemptFailure`. A success is wrapped in `StartedAttempt<T>` so the selected
`CredentialLease` remains live until that wrapper is dropped. The caller supplies an
`gateway-core::TransparentRetryGate`; `StreamControl` implements it and therefore exposes actual
downstream cancellation and first-semantic-event state rather than an upstream event being decoded
or merely queued.

## Preconditions

- The Route exists in the immutable scheduler Snapshot and has positive `max_attempts` and
  `bootstrap_timeout_ms` values validated on the control path.
- Each selected Candidate/lease pair came from the same validated Snapshot/pool generation and
  contains only stable non-secret identities at the orchestration boundary.
- The driver does not retain borrowed Credential bytes after its future returns, does not log
  Secret material, and reports a status/body-derived failure as a safe `AttemptFailure`.
- The retry gate reports cancellation and first-semantic-event delivery monotonically. Once a
  bridge places its first semantic canonical event into downstream delivery, it waits for that
  event's actual delivery or cancellation before it consumes later upstream output.
- Retry-after and fallback cooldown durations are positive and represent bounded duration values;
  no caller requests a sleep, waiter, or global cooldown queue.

## Required behavior

| Concern | Required behavior |
|---|---|
| Retry budget | Start no more than the Route's `max_attempts`; never begin an attempt once the cumulative bootstrap deadline has expired. One in-flight `start` is bounded by the driver-declared `start_timeout(remaining_bootstrap)` (default: the remaining cumulative bootstrap budget); a driver may extend only its own in-flight attempt, never the window in which a subsequent attempt may begin. No retry waits for cooldown expiry. |
| Binding exclusion | After any retryable failure, exclude exactly the attempted Candidate/Credential binding before the next selection. The predicate runs before pool capacity reservation, so an excluded binding never leaks a lease. |
| Selection | Reuse P3-05 Endpoint and Endpoint/Credential availability checks plus P3-04 bounded two-stage scheduling. A healthy sibling Credential remains eligible after a 429. |
| Quota-recovery probe admission | Only after ordinary selection fails may the loop admit exactly one controlled quota-recovery probe: Health predicates run unchanged and first, the binding-wide quota must be due (`RecoveryRequired`), the Credential lease is acquired before the single non-cloneable CAS registry ticket is begun (a lost ticket race releases the lease, never leaks a ticket), and the ticket expiry is derived from the driver-declared start ceiling plus a bounded grace. A successful probe completes the ticket with an Estimated empty-window snapshot; a failed probe's fresh 429 snapshot supersedes the ticket; a cancelled or abandoned probe leaves the target blocked until the ticket deadline. |
| 429 | Record a Cooldown only for the failed `(EndpointId, CredentialId)` pair. Prefer the classified retry-after duration; otherwise use the orchestrator's configured finite fallback. |
| Connection, 5xx, pre-FSE truncation | Record an Endpoint Cooldown and make a next eligible Candidate available only if budget and retry gate both permit it. P3-06 does not open or automatically close a Circuit. |
| Non-retryable failure | Return the safe supplied `GatewayError` immediately. Do not exclude/retry a client/request/permanent error. |
| Cancellation | Return `Cancelled/Request`; do not start another attempt or mutate health for cancellation itself. |
| First semantic event | After any retryable failure, retry only when the gate says no cancellation and no actual client-visible semantic event. Once FSE is committed, return the safe failure and never transparently start another binding. |
| Lease lifetime | A failed attempt drops its selected lease before the next selection. A successful `StartedAttempt<T>` retains it through the output lifetime; dropping it releases capacity even when the request is cancelled. |
| Error privacy | Public errors and `Debug` summaries do not render Candidate, Endpoint, Credential, URL, retry-after value, response body, Authorization, or Secret text. |

## Invariants

- The exclusion set and budget are per external request, not shared across requests or persisted.
- Runtime Cooldown state is mutable but does not mutate `RouteSnapshot` or `/v1/models` visibility.
- A retry cannot reacquire the same Candidate/Credential binding in one request, even if a
  cooldown expires between selections.
- The router does not create an unbounded queue, wait for a cooldown, query SQLite, take a global
  scheduler lock, automatically recover a Circuit, emit P3-08 records, parse an OpenAI body, or
  contact a real endpoint by itself.
- A failure after first semantic output must be rendered by the relevant downstream protocol path;
  P3-06's only responsibility is that it does not replay it to another upstream.

## Error semantics

| Condition | Result |
|---|---|
| No selectable Route binding before any Attempt | Existing `GatewayError(CredentialUnavailable, Credential)`; no identity text. |
| Connection failure or exhausted bootstrap budget with no prior classified failure | `GatewayError(EgressUnavailable, Egress)`. |
| 429 | `GatewayError(ProviderRateLimited, Provider)` if it cannot be retried. |
| 5xx | `GatewayError(ProviderTransient, Provider)` if it cannot be retried. |
| Pre-semantic truncated stream | `GatewayError(StreamTruncated, Stream)` if it cannot be retried. |
| Safe non-retryable driver failure | The exact supplied safe `GatewayError`; no additional diagnostic is retained. |
| Cancellation | `GatewayError(Cancelled, Request)`. |
| Clock/deadline/health mutation failure | Safe `GatewayError(InternalError, Internal)`; fail closed rather than retrying an unrecorded failed binding. |

## Corresponding tests

- `gateway-router::attempt_orchestrator::tests` proves connection fallback, 429 Credential-only
  cooldown with a healthy sibling, 5xx Endpoint fallback, pre-semantic truncation fallback,
  budget exhaustion, exclusion before lease acquisition, cancellation, FSE closure, lease release,
  and redacted diagnostics using synthetic identities only. It additionally proves a
  driver-declared `start_timeout` lets one healthy attempt outlive the bootstrap deadline, that
  the default ceiling still cuts an attempt at the remaining budget, and that an extended ceiling
  preserves pre-first-byte transparent retry and safe truncation errors. Its controlled
  quota-recovery tests prove one due binding self-recovers through exactly one probe attempt,
  concurrent selections admit at most one probe, a failed probe returns to cooldown instead of
  flapping, and a forbidden account is never admitted as a quota probe
  (`a_due_quota_reset_self_recovers_through_one_controlled_probe_attempt`,
  `concurrent_selection_admits_at_most_one_quota_recovery_probe`,
  `a_failed_quota_probe_returns_to_cooldown_instead_of_flapping`,
  `a_forbidden_account_is_never_admitted_as_a_quota_probe`).
- `gateway-router::credential_scheduler::tests` proves Candidate-plus-Credential predicates run
  before a CAS lease reservation while a sibling binding remains selectable, and that
  `quota_recovery_selection_admits_only_a_due_binding` admits only a due `RecoveryRequired`
  binding whose exact model target is quota-available.
- `gateway-stream::tests` proves the downstream control can wait for first semantic delivery or
  cancellation without treating queued/encoded events or SSE keepalives as FSE.
