# ADR-0032: Read-only runtime management status projection and Credential-account recovery

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-21` |
| Task / Matrix / Contract references | `P4-10`; `CR-P4-G4-001`; `G20`, `G21`, `G26`, `H19`, `H20`; [BC-MGMT-001](../contracts/BC-MGMT-001-read-only-runtime-management-status.md) |

## Context

P4 already has bounded runtime Health/Circuit state, exact Quota/Reset state, controlled recovery
tickets, and fixed-input Route Explain. They are intentionally separate runtime primitives. Before
this decision, however, no transport-neutral caller could obtain one safe projection of the exact
Endpoint/Credential account state together with the existing Health and Quota evidence.
`CredentialForbidden` was also returned safely but did not become a controlled-recovery state.

G4 requires a management-visible representation of 403 account status, 429/Quota, Circuit, and
recovery. Building an HTTP management endpoint now would prematurely introduce authentication,
authorization, input resolution, audit policy, and a public response contract that belong to P10.
The runtime registries are process-local and sharded, so a query must not imply a global transaction
or a durable management read model.

## Decision

- `gateway-router` exposes `RuntimeManagementStatusQuery`, constructed only from externally owned
  process-local `RuntimeHealthRegistry` and `RuntimeQuotaRegistry` instances. Its
  `binding_status` method accepts an exact Endpoint/Credential and optional non-empty upstream-model
  scope plus an explicit observation time. It is a read-only in-process Rust boundary, not an HTTP
  route, CLI, background task, or request-path dependency.
- The projection reports exact account status; Endpoint, binding, and optional model Health; and
  binding/model Quota availability with safe source, confidence, observation-time, and blocking
  Reset metadata. The model scope is never returned or rendered by `Debug`; the projection retains
  no URL, Header, Body, Credential bytes, Client Key, Provider diagnostic, or raw status material.
- Every component lookup receives the same caller-supplied timestamp, allowing correlation with
  P4-06 Route Explain. Each lookup remains independently locked: the result is not a cross-shard or
  cross-registry atomic snapshot and is never a lease, scheduling promise, or recovery authority.
- A driver may record an exact account block only when it already emits the safe
  `GatewayErrorCode::CredentialForbidden` classification. The Attempt remains non-retryable. The
  exact binding becomes `AccountForbidden`, so every model using that Endpoint/Credential account
  is blocked while sibling Credentials and Endpoints remain isolated. Generic cooldown, Circuit,
  and healthy transitions cannot silently clear the block.
- A separate `RuntimeHealthRegistry` account-recovery ticket is non-cloneable and exact-binding
  scoped. An authorized controller may begin one ticket and complete it with a sanitized `Allowed`
  or `Forbidden` result. Normal scheduling remains closed until a current, unexpired `Allowed`
  completion removes the block. The management query only observes this state; it cannot issue,
  complete, or otherwise control a ticket.

## Consequences

G4 can be evaluated against a safe, deterministic in-process projection without broadening P4 into
a remote control plane. The query is useful alongside Route Explain: callers provide the same
observation time to both, while accepting that concurrent writers can interleave between individual
shard reads.

The new 403 state is process-local and intentionally non-persistent. Restart behavior, raw
provider-status interpretation, a recovery executor, HTTP authentication/authorization, audit
records, public JSON shape, UI, and durable management history remain deferred. P10 owns the
authenticated HTTP/UI adapter and access policy; provider phases own raw response classification.

## Alternatives considered

- Add a P4 HTTP management endpoint: rejected because it would invent P10's authentication,
  authorization, audit, response, and network-surface policy early.
- Make Route Explain itself the management query: rejected because Explain is Route/schedule input
  based and does not provide a direct account status or safe Quota source/confidence projection.
- Automatically reopen a 403 account after a timer or generic successful Health operation: rejected
  because either action would turn absence of recovery evidence into ordinary scheduling permission.
- Persist or export the runtime projection now: rejected because P4-07/P4-08 boundaries are
  intentionally narrow and P10 owns durable management read-model design.

## Validation and rollback

Synthetic tests prove exact 403 binding isolation, no transparent retry, controlled account
recovery, fixed-time 429/Header and model Estimated Quota projection, Circuit visibility, safe
model redaction, no registry-write side effect, and Route Explain's account-block reason. They make
no Provider request.

Rollback removes the management projection and account-recovery additions. It leaves existing
Health/Circuit, Quota/Reset, Route Explain, SQLite/Event, transport, configuration, and public HTTP
behavior unchanged.

## Amendment (2026-07-26, P12)

The P12 management facade now receives the same runtime Health/Quota registries the request path
consults and may begin and complete one controlled local recovery transition per authorized reset
request (BC-MGMT-001): an operator-confirmed forbidden account (403) is completed with `Allowed`
account-level evidence, and a due (post-Reset) quota target may be operator-overridden with an
`Estimated` empty-window snapshot. Pre-Reset exhausted windows are refused, no Provider request
is sent, and the read-only status query itself remains observational.

The facade additionally owns a read connection to the append-only `SQLite` event log (ADR-0027).
Its Attempt listing is now backed by the durable Request-correlated timeline instead of the
in-memory stage ledger: each row reports the persisted terminal outcome and the exact non-secret
Endpoint/Credential identities the durable Attempt event already carries. The bounded stage
ledger only enriches an unambiguous single-Attempt timeline with the closed stage enum; ordinary
ledger loss degrades to a stage-free listing rather than failing the durable read closed, while a
ledger holding more terminals than the durable log does fail closed, because that divergence is
evidence of an Attempt whose durable record never landed.
