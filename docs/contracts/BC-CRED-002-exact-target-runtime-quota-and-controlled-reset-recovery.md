# BC-CRED-002: Exact-target runtime Quota and controlled Reset recovery

| Field | Value |
|---|---|
| Contract | `BC-CRED-002` |
| Task | `P4-05` |
| ADR | [ADR-0028](../adr/ADR-0028-exact-target-runtime-quota-and-controlled-reset-recovery.md) |
| Extends | [BC-CRED-001](BC-CRED-001-endpoint-credential-pool-leases.md), [BC-HEALTH-001](BC-HEALTH-001-sharded-runtime-health.md), and [BC-ROUTER-003](BC-ROUTER-003-request-scoped-attempt-orchestration.md) |
| Domain | Sanitized runtime quota evidence, target isolation, pre-lease filtering, and controlled recovery |

## Entry and boundary

`QuotaSnapshot::try_new` and `RuntimeQuotaRegistry` accept only an exact non-secret target,
bounded structural quota windows, a source, a confidence, and explicit observation time.
`AttemptOrchestrator` converts an already-classified 429 into one binding-wide snapshot; it does
not parse a raw response. `RouteCredentialScheduler` reads quota state before reserving a
Credential lease.

This contract does not issue HTTP requests, parse raw headers, call a billing endpoint, access a
Credential Secret, persist a quota record, expose management HTTP, render Route Explain, export
telemetry, retain a URL/body/header/status text, or authorize a real recovery probe.

## Target identity and retained evidence

| Target | Exact identity | Scheduling effect |
|---|---|---|
| Binding-wide | `(EndpointId, CredentialId)` | An exhausted window blocks every Candidate model using that binding. |
| Model-scoped | `(EndpointId, CredentialId, non-empty upstream_model)` | An exhausted window blocks only that exact model on that binding. |

`QuotaSnapshot` stores no more than eight `QuotaWindow` values. Each has a bounded non-empty
structural label, optional limit, optional remaining value, and optional Reset instant. If both
limit and remaining exist, remaining cannot exceed limit. Duplicate labels are rejected. An
exhausted window (`remaining == 0`) requires a Reset strictly after `observed_at_ms`; it cannot
create a permanent or ambiguous block.

| Source | Confidence rule |
|---|---|
| `Header`, `Billing`, `Rest`, `Grpc` | The external classifier chooses `Observed` or `Authoritative` without retaining raw source content. |
| `Estimated` | Must pair with `Estimated`; it cannot claim direct observation or authority. |

## 429 and transient classification

| Classified outcome | Runtime mutation |
|---|---|
| 429 with positive `Retry-After` | Exact binding-wide exhausted `rate_limit` window, `Header/Observed`, Reset = observation time + retry duration. |
| 429 with missing or zero retry metadata | Exact binding-wide exhausted `rate_limit` window, `Estimated/Estimated`, Reset = observation time + existing finite 30-second fallback. |
| Connection failure, 5xx, or pre-semantic truncation | Existing Endpoint runtime-health cooldown only; no quota snapshot. |
| Cancellation or non-retryable failure | No quota or transient-health mutation. |

A rate-limited binding is added to the request's existing local exclusion set before the next
transparent attempt. Later requests also see the registry's pre-lease quota predicate. A healthy
sibling binding or unrelated model remains eligible subject to the normal route, health, and
concurrency rules.

## Reset and controlled-recovery timeline

| State / action | Required behavior |
|---|---|
| No retained blocking snapshot | `Available`; ordinary scheduling may proceed. |
| Exhausted window, current time before latest Reset | `Exhausted`; ordinary scheduling is rejected. |
| Current time reaches latest Reset | `RecoveryRequired`; ordinary scheduling remains rejected. |
| One caller begins a recovery probe with a future expiry | Receives the sole non-cloneable ticket; state is `RecoveryProbeInFlight`; ordinary scheduling remains rejected. |
| Another caller during the ticket lifetime | Receives no ticket and cannot send an ordinary recovery request. |
| Current ticket completes before expiry with exact non-exhausted snapshot | Snapshot replaces the old state; ordinary scheduling becomes `Available`. |
| Current ticket completes with still-exhausted snapshot | New reset evidence is retained; ordinary scheduling remains blocked. |
| Ticket expiry, target mismatch, supersession, or an observation-time regression | Completion fails closed; it cannot reopen or overwrite current state. A later caller may obtain one replacement ticket only when due. |

A new equal-or-newer snapshot invalidates any older ticket. The registry uses 64 independent
shards with a finite 1,024-entry limit each. On capacity pressure it may reclaim an already
available snapshot, but it must retain exhausted, recovery-required, and probe-in-flight state.

A completed live-selection probe records its evidence as an `Estimated/Estimated` snapshot with
an empty window list: the upstream accepted one real request, which proves availability without
claiming an observed quota window. A probe attempt that fails with another 429 needs no explicit
ticket handling; the fresh exhausted snapshot supersedes the outstanding ticket.

## Error semantics

| Condition | Required result |
|---|---|
| Empty model label, invalid window, duplicate label, too many windows, source/confidence mismatch, or exhausted window without future Reset | Typed constructor error; no registry mutation. |
| Zero fallback, clock/shard unavailable, or full shard with only blocking state | Typed runtime-quota error; scheduler fails closed before a lease. |
| Older snapshot for the same target | `ObservationTimeRegressed`; current state and ticket remain unchanged. |
| No due exhausted quota or another unexpired recovery ticket | `Ok(None)` from `begin_recovery_probe`; no ordinary traffic is admitted. |
| Stale, expired, superseded, or target-mismatched ticket | Typed recovery error; current quota state remains unavailable or unchanged. |

## Deferred behavior

Provider adapters own raw Header/Billing/REST/gRPC interpretation. P4-06 owns Route Explain,
P4-07 does not gain a quota persistence path in this Task, P4-08 owns telemetry, P4-09 owns log
redaction/body policy, and later Provider phases own durable provider-specific quota restoration.
Explicitly authorized real probe execution is delivered for the live selection path (P12):
`RouteCredentialScheduler::select_eligible_and_lease_for_quota_recovery` admits one due
`RecoveryRequired` binding after ordinary selection fails, and `AttemptOrchestrator` runs it as
one ticketed controlled probe attempt per BC-ROUTER-003.

## Corresponding tests

- `gateway-router::runtime_quota::tests::source_confidence_reset_and_model_scope_are_exact_target_isolated`
- `gateway-router::runtime_quota::tests::rate_limit_records_header_or_explicit_estimate_without_conflation`
- `gateway-router::runtime_quota::tests::reset_requires_one_controlled_recovery_probe_before_ordinary_scheduling`
- `gateway-router::runtime_quota::tests::stale_probe_cannot_overwrite_newer_quota_observation`
- `gateway-router::runtime_quota::tests::a_full_shard_reclaims_available_snapshots_but_never_blocking_quota`
- `gateway-router::credential_scheduler::tests::model_quota_filters_before_lease_and_reset_needs_controlled_recovery`
- `gateway-router::credential_scheduler::tests::quota_recovery_selection_admits_only_a_due_binding`
- `gateway-router::attempt_orchestrator::tests::rate_limit_records_exact_quota_and_preserves_a_healthy_sibling`
- `gateway-router::attempt_orchestrator::tests::a_due_quota_reset_self_recovers_through_one_controlled_probe_attempt`
- `gateway-router::attempt_orchestrator::tests::concurrent_selection_admits_at_most_one_quota_recovery_probe`
- `gateway-router::attempt_orchestrator::tests::a_failed_quota_probe_returns_to_cooldown_instead_of_flapping`
- `cargo test --locked -p gateway-router`
- `cargo clippy --locked -p gateway-router --all-targets --all-features -- -D warnings`
- `./scripts/check.sh full`
