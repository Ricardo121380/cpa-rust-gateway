# BC-HEALTH-002: Target-local probe EWMA and controlled Circuit recovery

| Field | Value |
|---|---|
| Contract | `BC-HEALTH-002` |
| Task | `P4-04` |
| ADR | [ADR-0026](../adr/ADR-0026-target-local-probe-ewma-and-circuit-recovery.md) |
| Extends | [BC-HEALTH-001](BC-HEALTH-001-sharded-runtime-health.md) P3-05 Endpoint/Credential runtime state |
| Domain | Sanitized probe outcomes, target-local EWMA, model isolation, and half-open Circuit recovery |

## Entry and boundary

`RuntimeHealthProbeRegistry` receives an exact `RuntimeHealthProbeTarget`, a sanitized terminal
outcome `{ succeeded | failed, latency_ms }`, and explicit Unix-millisecond observation time. It
does not create a request, call a Provider, reach an Endpoint, parse a response, read a Header,
know a URL, retain a Secret, inspect quota, or persist a row. An authorized executor is responsible
for supplying an already-classified terminal outcome.

`RuntimeHealthProbeRegistry::complete_circuit_probe` combines an exact target, current
`RuntimeHealthCircuitProbe` ticket, sanitized success/failure, explicit time, and (on failure) an
explicit future retry instant. It updates only the matching target's EWMA and matching Circuit.

## Target identity and measurements

| Target | Exact identity | Isolation rule |
|---|---|---|
| Endpoint | `EndpointId` | A protocol-specific Endpoint result never affects a sibling Endpoint that shares an Upstream or URL. |
| Endpoint/Credential | `(EndpointId, CredentialId)` | One Credential's result does not affect a healthy sibling Credential or the same Credential at another Endpoint. |
| Endpoint/Credential/model | `(EndpointId, CredentialId, non-empty exact upstream_model)` | A model failure affects neither another model nor the same model through another Credential/Endpoint. |

The first observation initializes `success_ewma_per_mille` to `1000` for success or `0` for failure
and `latency_ewma_ms` to `latency_ms`. Later observations use
`round((previous * (1000 - alpha) + sample * alpha) / 1000)`, where the default `alpha` is `200`.
Success samples are `1000`/`0`; latency samples are whole milliseconds. An observation time may
equal the previous time but may not regress for the same exact target.

## Circuit recovery timeline

| State / action | Required behavior |
|---|---|
| `CircuitOpen { retry_after_ms }`, now before retry instant | No recovery ticket; ordinary scheduling remains unavailable. |
| Due open Circuit | Exactly one caller can acquire a ticket with a strictly future probe-expiry instant. |
| Half-open ticket outstanding | Ordinary scheduling still observes `CircuitOpen`; another caller receives no ticket. |
| Successful current ticket before expiry | The exact Circuit is removed and the success EWMA is recorded. |
| Failed current ticket before expiry | The exact Circuit becomes `CircuitOpen { retry_after_ms }` with the supplied strictly future instant; failure EWMA is recorded. |
| Ticket expiry, replacement, manual reopen, or target mismatch | Completion fails closed; it cannot close or alter the newer Circuit and it inserts no metric snapshot. An expired ticket may be replaced by one new due probe. |

`RuntimeHealthKey::EndpointCredentialModel` integrates with `RouteCredentialScheduler` after its
Endpoint and Endpoint/Credential checks, before a Credential pool reserves capacity. The scheduler
uses the immutable Candidate's exact `upstream_model`; a blocked model binding leaves a healthy
sibling Credential/model eligible. Runtime availability remains outside `RouteSnapshot` and never
changes `/v1/models` by itself.

## Error semantics

| Condition | Safe result |
|---|---|
| Empty model target | `RuntimeHealthProbeTargetError::EmptyUpstreamModel`; no target retained. |
| Regressed time / counter overflow / full or poisoned probe shard | Named `RuntimeHealthProbeError`; no target mutation. |
| No due Circuit or current unexpired ticket | `Ok(None)` from begin; no traffic is admitted. |
| Stale, expired, or superseded ticket | `RuntimeHealthError::StaleCircuitProbe`; Circuit remains unavailable. |
| Target/ticket mismatch | `RuntimeHealthProbeCompletionError::TargetDoesNotMatchCircuitProbe`; no Circuit or metric mutation. |
| Invalid future deadline or runtime lock/clock failure | Existing safe `RuntimeHealthError`; scheduler remains fail-closed. |

## Deferred behavior

P4-05 owns quota/429/reset classification and controlled quota probes. P4-06 owns operator-facing
Route Explain. P4-07 owns SQLite event persistence and restart behavior. P4-08 owns telemetry
export. P4-09 owns logging/body sampling/redaction. This contract creates no HTTP probe executor,
periodic task, management endpoint, durable status, automatic route publication, or real Provider
request.

## Corresponding tests

- `gateway-router::runtime_probe::tests::model_scoped_probe_ewma_is_exact_target_isolated`
- `gateway-router::runtime_probe::tests::controlled_circuit_probe_updates_exact_ewma_and_recovery_state`
- `gateway-router::runtime_health::tests::model_circuit_isolated_and_half_open_recovery_is_single_ticket`
- `gateway-router::runtime_health::tests::expired_half_open_ticket_cannot_overwrite_a_reopened_circuit`
- `gateway-router::credential_scheduler::tests::model_scoped_circuit_skips_only_the_failed_endpoint_credential_binding`
- `cargo test --locked -p gateway-router`
- `cargo clippy --locked -p gateway-router --all-targets --all-features -- -D warnings`
- `./scripts/check.sh full`
