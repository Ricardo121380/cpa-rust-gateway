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
| Begin recovery | The P12 facade may begin and complete one controlled local recovery transition in its injected registries: an operator-confirmed forbidden account (403) is the account-level evidence required by BL-16; a due (post-Reset) quota target may be operator-overridden. Pre-Reset exhausted windows are refused. No Provider request is sent. |
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

## Optional closed Attempt-stage projection

P12-05 may populate an optional `stage` member on an already-authorized value-free Attempt row.
It is not an arbitrary diagnostic string: its only permitted values are `request_conversion`,
`egress_admission`, `http_transport`, `http_status`, `content_type`, `body_read`, `decoder`, and
`sse_bootstrap`. The existing terminal `succeeded|failed` outcome remains mandatory, while an
embedding without stage instrumentation omits the member for wire compatibility.

The stage projection is process-local and bounded. It must contain no URL, Header, Body, status
number, error string, timestamp, token, digest, or Provider result. In the P12 serve composition
the durable append-only event log (BC-OBS-002) is the authoritative Attempt listing: each listed
row carries the persisted terminal outcome plus the exact non-secret `endpoint_id` and
`credential_id` identities that the durable Attempt event already contains. The in-memory
bounded stage ledger is an enrichment only; it retains the newest requests up to twice the
admitted total Credential concurrency and evicts the oldest rather than latching itself off.
Ledger contention, a missing record, an unpaired terminal, or a multi-Attempt timeline degrades to
a stage-free listing — durable evidence is never hidden behind the bounded stage store. A ledger
that holds more terminals than the durable log returns is the one case that fails the read closed:
it proves this process observed an Attempt whose durable evidence is missing, which must never be
served as a shorter successful listing. This refinement adds no Provider
request, data-plane route, or recovery authority.

## Corresponding tests

- `gateway-router::runtime_management_status::tests::exact_read_only_projection_shows_403_quota_circuit_and_controlled_recovery`
- `gateway-router::runtime_management_status::tests::target_scope_is_validated_and_explicit_query_never_reads_the_shared_clock`
- `gateway-router::attempt_orchestrator::tests::credential_forbidden_blocks_only_its_binding_until_controlled_recovery`
- `gateway-router::runtime_health::tests::forbidden_account_is_binding_scoped_and_needs_its_own_recovery_ticket`
- `gateway-router::runtime_health::tests::rejected_or_stale_account_recovery_ticket_cannot_reopen_a_forbidden_binding`
- `gateway-router::route_explain::tests::fixed_explain_reports_exact_health_and_quota_reasons_without_a_lease`
- `gateway::runtime::tests::p12_attempt_stage_projection_is_terminal_and_value_free`
- `gateway::runtime::tests::p12_attempt_stage_contention_withholds_the_stage_projection`
- `gateway::runtime::tests::p12_attempt_stage_capacity_withholds_new_stage_projections`
- `gateway::runtime::tests::p12_management_listing_survives_stage_ledger_exhaustion`
- `gateway::runtime::tests::p12_serve_composition_persists_request_attempt_usage_for_management_reads`
- `gateway::runtime::tests::operator_quota_reset_recovers_a_due_binding_through_the_real_handle`
- `gateway::runtime::tests::operator_endpoint_recovers_a_forbidden_account_with_explicit_evidence`
- `cargo test --locked -p gateway-router`
- `cargo clippy --locked -p gateway-router --all-targets --all-features -- -D warnings`
- `./scripts/check.sh full`
