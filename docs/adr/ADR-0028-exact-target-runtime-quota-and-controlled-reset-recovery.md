# ADR-0028: Exact-target runtime Quota snapshots and controlled Reset recovery

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-21` |
| Task / Matrix / Contract references | `P4-05`; `E19`, `G20`, `G26`, `BL-17`; [BC-CRED-002](../contracts/BC-CRED-002-exact-target-runtime-quota-and-controlled-reset-recovery.md) |

## Context

P3-06 classified a retryable 429 as an Endpoint/Credential cooldown. That was deliberately safe
for the MVP, but it loses the distinction between a quota observation and a transient transport
failure, cannot retain a source/confidence/reset window, and makes it too easy to treat a passed
timer as recovery evidence. A quota may apply to every model on one Endpoint/Credential binding or
only to one exact upstream model. It must not suppress a healthy sibling Credential, Endpoint, or
model.

The router remains transport-neutral: it must not parse raw headers, execute a billing request,
read a Secret, persist a row, or retain a URL, response body, header value, or provider diagnostic.
P4-05 therefore needs a bounded sanitized runtime-state boundary, not a Provider implementation or
management API.

## Decision

- `RuntimeQuotaTarget` is either exact `(EndpointId, CredentialId)` or exact
  `(EndpointId, CredentialId, non-empty upstream_model)`. `QuotaSnapshot` retains only that target,
  at most eight structural windows, `QuotaSource`, `QuotaConfidence`, and explicit observation
  time. Window labels are bounded; duplicate labels, impossible remaining counts, and exhausted
  windows without a strictly later Reset are rejected.
- `Header`, `Billing`, `Rest`, `Grpc`, and `Estimated` are distinct sources. An estimated source
  and estimated confidence must occur together, so an inferred value cannot be presented as a
  direct observation or vice versa. Raw source material is not retained.
- `RuntimeQuotaRegistry` is an in-memory 64-shard bounded registry with at most 1,024 targets per
  shard and no global lock. Under capacity pressure it reclaims only entries already available;
  it never drops an exhausted or recovery-required target, because absence would otherwise reopen
  ordinary scheduling. The registry is queryable but does not add SQLite persistence in this Task.
- A 429 from `AttemptOrchestrator` writes a binding-wide quota snapshot. A positive `Retry-After`
  becomes `Header/Observed`; missing or zero retry metadata becomes the existing bounded 30-second
  fallback as `Estimated/Estimated`. Connection, 5xx, and pre-semantic truncation remain Endpoint
  runtime-health cooldowns rather than quota observations.
- Scheduling checks Endpoint health, then exact binding and model health/quota, before a
  Credential-pool lease is acquired. This keeps a healthy sibling eligible and prevents a blocked
  binding from consuming lease capacity.
- Reaching the latest exhausted-window Reset changes availability only to `RecoveryRequired`.
  Exactly one non-cloneable recovery ticket may be issued with a strictly future expiry. Ordinary
  traffic remains blocked while it is outstanding and after it expires. Only completion with the
  current ticket and a new exact sanitized snapshot can reopen scheduling. A newer snapshot
  invalidates an older ticket, so a stale probe cannot overwrite fresh quota evidence.

## Consequences

429 handling now has explicit source/confidence/reset state and no longer conflates quota with
Endpoint health. A scheduler can preserve capacity by selecting an unaffected Credential or Route
Candidate, while an elapsed Reset never causes an arbitrary customer request to be the first probe.
The state is deterministic with an injected clock and bounded under high target churn.

P4-06 will turn exact quota availability into operator-facing Route Explain exclusions. P4-08 and
P4-09 own telemetry/export and logging/redaction respectively. Provider-specific Header/Billing/
REST/gRPC classifiers, durable restart restoration, management APIs, and real recovery probes are
deferred; no real Provider request is authorized or issued by this decision.

## Alternatives considered

- Treat 429 as an Endpoint or Endpoint/Credential transient cooldown: rejected because it cannot
  describe source/reset evidence and over- or under-scopes model-specific quota.
- Reopen ordinary traffic when Reset time passes: rejected because a timer is not evidence that the
  next request will succeed and would make a customer request an uncontrolled recovery probe.
- Use a global quota map or unbounded retained history: rejected because request-time contention or
  unbounded memory violates the router's hot-path boundary.
- Evict all due snapshots at capacity: rejected because a due exhausted snapshot still requires a
  controlled recovery ticket; removing it would silently admit normal traffic.
- Parse/provider-persist raw quota metadata in `gateway-router`: rejected because it would pull
  HTTP, Provider, Secret, and Store responsibilities into this transport-neutral boundary.

## Validation and rollback

Synthetic tests prove source/confidence separation, exact model isolation, binding-wide 429
recording, healthy-sibling fallback, Reset-not-auto-open, one recovery ticket, stale-ticket
rejection, and safe bounded-shard reclamation. They send no Provider traffic.

Rollback removes the registry, pre-lease quota predicate, and 429 snapshot handoff, reverting 429
to the prior bounded cooldown behavior. It changes no SQLite schema, RouteSnapshot, public model
view, Endpoint configuration, Credential Secret, Provider request, or external API.

## Amendment (2026-07-26, P12)

Explicitly authorized real probe execution is now delivered for the live selection path
(BC-ROUTER-003) and, as an operator override, through the P12 management facade (BC-MGMT-001).
After ordinary selection fails, the orchestrator may admit exactly one due (`RecoveryRequired`)
binding as a controlled probe attempt: Health predicates run unchanged and first, the Credential
lease is acquired before the single non-cloneable registry ticket is begun (a lost race releases
capacity instead of leaking a ticket), and the ticket expiry derives from the driver-declared
start ceiling plus a bounded grace. A successful probe completes its ticket with an
`Estimated/Estimated` empty-window snapshot; a probe that hits another 429 is superseded by the
fresh exhausted snapshot. The Reset-never-auto-opens rule and the registry's fail-closed ticket
semantics are unchanged.
