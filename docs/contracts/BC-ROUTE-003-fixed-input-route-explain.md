# BC-ROUTE-003: Fixed-input Route Explain and Candidate exclusion reasons

| Field | Value |
|---|---|
| Contract | `BC-ROUTE-003` |
| Task | `P4-06` |
| ADR | [ADR-0029](../adr/ADR-0029-fixed-input-route-explain.md) |
| Extends | [BC-SCHEDULER-001](BC-SCHEDULER-001-priority-tier-smooth-weighted-scheduler.md), [BC-CRED-001](BC-CRED-001-endpoint-credential-pool-leases.md), [BC-HEALTH-002](BC-HEALTH-002-target-local-probe-ewma-and-circuit-recovery.md), [BC-CRED-002](BC-CRED-002-exact-target-runtime-quota-and-controlled-reset-recovery.md), and [BC-ROUTER-003](BC-ROUTER-003-request-scoped-attempt-orchestration.md) |
| Domain | Side-effect-free Route/Credential eligibility diagnostics |

## Entry and boundary

`RouteCredentialScheduler::explain` accepts a `RouteExplainInput`, the exact runtime Health and
Quota registries, and the current request's `AttemptExclusionSet`. The input contains a Route,
explicit Unix-millisecond observation time, and explicit Candidate/Credential schedule starts.
The result is a bounded `RouteExplainSnapshot` for that fixed input.

Explain reads the exact immutable `RouteSnapshot` and Endpoint Credential pools used by the
scheduler. It does not acquire a `CredentialLease`, advance a Candidate or Credential cursor,
write runtime state, begin/complete a recovery probe, query SQLite, execute an HTTP/Provider call,
or create a background task. It retains no Secret, presented Client Key, URL, Header, body, status
text, raw extension, or Provider diagnostic.

## Candidate and binding evidence

Each Candidate is returned in stable immutable Snapshot order with Candidate/Endpoint/Upstream IDs,
exact upstream model label, priority, weight, Catalog admission, active binding count, candidate
reasons, and secret-free Credential observations. Credential observations are stable-ID-order
snapshots of priority, weight, maximum concurrency, active lease count, and reasons.

| Level | Reason | Required interpretation |
|---|---|---|
| Candidate | `NotHardEligible` | Compiler-time Catalog/binding predicate is no longer hard eligible. |
| Candidate | `EndpointHealth(state)` | Exact Endpoint is cooling or Circuit-open at `observed_at_ms`. |
| Candidate | `EndpointHealthUnavailable` | Health read failed; mirror real scheduling's fail-closed behavior. |
| Candidate | `MissingCredentialPool` | The exact runtime assembly lacks this Endpoint's pool. |
| Candidate | `NoEligibleCredential` | Every observed binding has at least one binding-level exclusion. |
| Binding | `RequestExcluded` | This exact Candidate/Credential pair already failed in the current transparent-attempt sequence. |
| Binding | `Saturated` | Point-in-time active leases reached immutable maximum concurrency. |
| Binding | `CredentialHealth` / `ModelHealth` | Exact Endpoint/Credential or Endpoint/Credential/model Health blocks the binding. |
| Binding | `BindingQuota` / `ModelQuota` | Exact binding-wide or model-scoped Quota blocks ordinary scheduling. |
| Binding | `*Unavailable` | Matching Health/Quota state could not be formed/read; the reason is fail-closed and does not expose internal failure text. |

`RouteExplainCandidate::is_eligible` is true only when it has no Candidate reason and at least one
Credential with no binding reason. A model quota never suppresses another model or Credential; a
request-local exclusion never suppresses a sibling binding.

## Fixed policy projection

`RouteExplainProjectedSelection` is optional. It scans the immutable priority-tier schedules from
the input's explicit starts and uses a non-mutating Credential-pool peek. It chooses only an
explained eligible Candidate/binding and preserves priority-tier fallback semantics. It never
changes real cursor state or reserves capacity.

The projection is not a delivery guarantee. Another request can acquire a lease after Explain
observes capacity. In that case Explain treats the now-unavailable binding as unusable and keeps
scanning later Candidates, rather than returning a stale lease claim. A real request always runs
the normal atomic lease path independently.

## Error semantics

| Condition | Required result |
|---|---|
| Route absent from immutable Snapshot | `RouteExplainError::UnknownRoute`; no state mutation or identifier-rich scheduler error. |
| Route has no compiled schedule | `RouteExplainError::MissingRouteSchedule`; no fallback schedule is invented. |
| Health/Quota shard or target-build failure | Typed secret-free `*Unavailable` reason at the affected level; the binding remains ineligible. Explicit input time means Explain does not call a runtime clock. |
| Pool absent or all bindings excluded/saturated | Candidate carries `MissingCredentialPool` or `NoEligibleCredential`; global result may have no projected selection. |
| Concurrent capacity change during projection | No lease is claimed; Explain continues to another Candidate or returns no projection. |

## Deferred behavior

P4-06 creates no management HTTP endpoint, Client Key verification, public-model/Alias/protocol
resolution, durable Explain timeline, request-event write, tracing export, log write, body policy,
affinity/continuity explanation, Provider classification, or real probe. P10 owns the authenticated
management API; P4-08/P4-09 own telemetry and logging/redaction.

## Corresponding tests

- `gateway-router::route_explain::tests::fixed_explain_reports_exact_health_and_quota_reasons_without_a_lease`
- `gateway-router::route_explain::tests::fixed_schedule_starts_never_advance_live_candidate_or_credential_cursors`
- `gateway-router::route_explain::tests::saturated_binding_is_explained_and_a_sibling_is_projected_without_a_new_lease`
- `gateway-router::route_explain::tests::request_local_exclusion_is_exact_and_unknown_route_stays_safe`
- `cargo test --locked -p gateway-upstream -p gateway-router`
- `cargo clippy --locked -p gateway-upstream -p gateway-router --all-targets --all-features -- -D warnings`
- `./scripts/check.sh full`
