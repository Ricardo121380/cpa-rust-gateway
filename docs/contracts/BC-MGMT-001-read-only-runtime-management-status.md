# BC-MGMT-001: Read-only runtime management status and controlled Credential-account recovery

| Field | Value |
|---|---|
| Contract | `BC-MGMT-001` |
| Task | `P4-10` |
| ADR | [ADR-0032](../adr/ADR-0032-read-only-runtime-management-status.md) |
| Extends | [BC-HEALTH-002](BC-HEALTH-002-target-local-probe-ewma-and-circuit-recovery.md), [BC-CRED-002](BC-CRED-002-exact-target-runtime-quota-and-controlled-reset-recovery.md), [BC-ROUTE-003](BC-ROUTE-003-fixed-input-route-explain.md), and [BC-ROUTER-003](BC-ROUTER-003-request-scoped-attempt-orchestration.md) |
| Domain | Process-local, read-only runtime status projection and safe 403 account recovery |

## Entry and boundary

`RuntimeManagementStatusTarget::try_new` accepts an exact `EndpointId`, non-secret
`CredentialId`, and an optional non-empty upstream-model scope. Its `Debug` representation reports
only whether a model scope exists. `RuntimeManagementStatusQuery::binding_status` then accepts that
target and an explicit Unix-millisecond observation time.

The query reads existing `RuntimeHealthRegistry` and `RuntimeQuotaRegistry` state only. It does not
write state, reserve a Credential lease, advance a Route/Credential cursor, begin or complete a
recovery ticket, read a runtime clock, query SQLite, create an Event, export telemetry, open a
socket, call a Provider, parse an HTTP response, or start a task. It returns no URL, Header, Body,
Credential Secret, Client Key, raw response diagnostic, or upstream-model label.

This is an in-process Rust boundary for a controlled management caller. It has no HTTP route,
authentication, authorization, public JSON representation, CLI command, UI, persistence model, or
remote control semantics. P10 owns those concerns.

## Projection and time semantics

Every component is evaluated with the exact caller-supplied `observed_at_ms` value.

| Projection member | Required safe content |
|---|---|
| Account | `Available`, exact-binding `Forbidden`, or exact-binding `RecoveryInFlight { expires_at_ms }`. |
| Endpoint Health | Existing Endpoint Cooldown/Circuit availability at the supplied time. |
| Binding Health | Exact Endpoint/Credential availability, including `AccountForbidden` and account recovery. |
| Model Health | Optional exact Endpoint/Credential/model availability; present only for a model-scoped target. |
| Binding Quota | Existing binding availability plus optional source, confidence, classifier observation time, and current blocking Reset. |
| Model Quota | The corresponding optional model-scoped Quota projection. |

The result is coherent per registry key: each Quota availability/snapshot pair is copied under one
isolated shard read lock. It is not an atomic snapshot across Health keys, Quota keys, or the two
registries. A caller must treat it as a point-in-time diagnostic projection, not a later scheduling
guarantee. A caller that also needs Candidate/lease/exclusion reasoning uses P4-06 Route Explain at
the same explicit time.

The existing Circuit contract deliberately projects a half-open Circuit ticket as
`CircuitOpen { retry_after_ms }` to ordinary scheduling. This query preserves that behavior and
does not reveal a mutable Circuit ticket. Quota recovery remains visible through its existing
`RecoveryRequired` or `RecoveryProbeInFlight` availability; account recovery is visible through the
dedicated account status and binding Health values.

## 403 account-state and recovery timeline

| State / action | Required behavior |
|---|---|
| Safe provider classification | Only an existing `GatewayErrorCode::CredentialForbidden` from an `AttemptFailure::NonRetryable` may mark the exact Endpoint/Credential account forbidden. The attempt remains non-retryable. |
| Exact isolation | The forbidden state blocks every model using that exact Endpoint/Credential account. A sibling Credential or another Endpoint remains independently evaluated. |
| Generic runtime mutations | `mark_healthy`, cooldown, and Circuit operations cannot clear or overwrite an active forbidden/account-recovery state. |
| Begin recovery | Only a separate controller call may obtain a non-cloneable, exact-binding ticket with a strictly future expiry. Ordinary traffic remains blocked and the query reports `RecoveryInFlight`. |
| Complete recovery | A current unexpired `Allowed` result removes only the exact account block; `Forbidden` retains the block. Expired, superseded, or mismatched tickets fail closed and cannot reopen a newer state. |
| Query behavior | The query is observational only. It never obtains or completes a ticket and cannot cause recovery traffic. |

## Error semantics

| Condition | Required result |
|---|---|
| Empty model scope | `RuntimeManagementStatusTargetError::EmptyUpstreamModel`; no registry read or mutation. |
| Required Health shard unavailable | `RuntimeManagementStatusQueryError::HealthUnavailable`; no partially populated result or identifier-rich diagnostic. |
| Required Quota shard or target unavailable | `RuntimeManagementStatusQueryError::QuotaUnavailable`; no partially populated result or identifier-rich diagnostic. |
| Explicit-time read | The query does not call either registry clock. A clock failure therefore cannot turn this read into an implicit-time result. |
| Concurrent state change | A result may contain independently timed key observations; it still has the caller's one explicit timestamp and grants no scheduling/recovery permission. |

## Deferred behavior

P10 owns authenticated HTTP management endpoints, authorization, public model/alias input
resolution, response serialization, UI, audit records, durable status, and remote recovery control.
Provider/transport phases own raw 403 or Header/body interpretation and any explicitly authorized
recovery request. This contract creates no real Provider request.

## Corresponding tests

- `gateway-router::runtime_management_status::tests::exact_read_only_projection_shows_403_quota_circuit_and_controlled_recovery`
- `gateway-router::runtime_management_status::tests::target_scope_is_validated_and_explicit_query_never_reads_the_shared_clock`
- `gateway-router::attempt_orchestrator::tests::credential_forbidden_blocks_only_its_binding_until_controlled_recovery`
- `gateway-router::runtime_health::tests::forbidden_account_is_binding_scoped_and_needs_its_own_recovery_ticket`
- `gateway-router::runtime_health::tests::rejected_or_stale_account_recovery_ticket_cannot_reopen_a_forbidden_binding`
- `gateway-router::route_explain::tests::fixed_explain_reports_exact_health_and_quota_reasons_without_a_lease`
- `cargo test --locked -p gateway-router`
- `cargo clippy --locked -p gateway-router --all-targets --all-features -- -D warnings`
- `./scripts/check.sh full`
