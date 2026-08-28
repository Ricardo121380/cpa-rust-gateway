# ADR-0029: Fixed-input Route Explain without scheduling side effects

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-21` |
| Task / Matrix / Contract references | `P4-06`; `E15`, `E16`, `E23`, `G20`, `G21`, `L20-L26`; [BC-ROUTE-003](../contracts/BC-ROUTE-003-fixed-input-route-explain.md) |

## Context

P3-06 deliberately returns the same secret-free `CredentialUnavailable` error whenever no
Candidate can make a safe lease. P4-04 and P4-05 add exact Health/Circuit and Quota/Reset state,
but neither tells an operator which Candidate or binding was filtered. A useful Route Explain must
show compiler-time eligibility, request-local retry exclusions, Endpoint/Binding/Model Health,
Binding/Model Quota, and lease saturation without changing the request path it is trying to
diagnose.

Reading a live selector by calling it is unsafe: it advances atomic cursors and may reserve a
Credential. Reading an arbitrary wall-clock time also makes the result difficult to reproduce.
The current phase has no management HTTP API, durable Explain record, affinity implementation, or
Provider transport boundary.

## Decision

- `RouteExplainInput` supplies the exact `RouteId`, explicit observation time, and deterministic
  Candidate/Credential schedule starts. `RouteCredentialScheduler::explain` reads the same
  immutable Snapshot and Endpoint pools as real scheduling but never acquires a lease, advances a
  cursor, mutates Health/Quota, or starts a recovery probe.
- `RouteExplainSnapshot` emits every immutable Candidate in stable Snapshot order. Each record
  carries only stable IDs, upstream model label, priority/weight, compiler Catalog admission,
  binding count, secret-free Credential pool observations, and typed exclusion reasons. It carries
  no Credential secret, URL, Header, body, response diagnostic, Client Key, or Provider result.
- Candidate reasons are `NotHardEligible`, exact Endpoint Health, unavailable Health read, missing
  Endpoint pool, and no eligible Credential. Binding reasons distinguish request-local exclusion,
  saturation, Endpoint/Credential Health, Endpoint/Credential/model Health, binding Quota,
  model Quota, and each corresponding fail-closed unavailable read.
- A projected selection simulates the precompiled Route and Credential policy from the input's
  fixed starts. It uses no live cursor. If concurrent capacity changes after the captured binding
  observation, the projection fail-closes that binding and continues to a later Candidate; it is a
  diagnostic point-in-time projection, never a lease or a promise of a later real request.
- `gateway-upstream` exposes a bounded secret-free `CredentialPoolEntrySnapshot` and explicit
  non-mutating pool peek. These APIs reveal concurrency metadata only; they cannot reveal or use
  the zeroizing Credential Secret.

## Consequences

An operator or a future management adapter can reproduce a pure Route decision snapshot with a
fixed time and fixed schedule positions. It can distinguish "Endpoint cooling", "model quota",
"this request already tried the binding", and "concurrency saturated" instead of treating all as a
generic unavailable Credential. Explain remains outside the response latency path and does not
alter actual weighted fairness or capacity.

P4-08 may export safe counters/traces and P4-09 owns logging/body redaction. P10 owns management
HTTP, Client Key/public-model/protocol input resolution, authentication, and any access policy for
internal Endpoint/Credential/model identifiers. Affinity/continuity causes remain deferred until
their own state exists. No real Provider request is authorized or issued here.

## Alternatives considered

- Call the ordinary scheduler and immediately drop its lease: rejected because it advances atomic
  cursor state, races real requests, briefly consumes capacity, and can change weighted fairness.
- Return only a final `CredentialUnavailable` error: rejected because it hides the exact
  Health/Quota/saturation/request-exclusion cause needed for recovery.
- Read a wall clock and live cursor implicitly: rejected because a management query could not be
  deterministically reproduced or fixture-tested.
- Put Explain behind HTTP in this Task: rejected because P10 owns management API auth, auditing,
  access controls, and public request shape.
- Include raw response diagnostics or a Credential Secret: rejected because Explain must be safe
  to retain and future access-control policy must not clean up leaked source material later.

## Validation and rollback

Synthetic tests prove exact Endpoint Health and model Quota reasons, request-local binding
exclusion, saturation with sibling projection, fixed schedule starts, no lease/cursor side effect,
and safe unknown-Route handling. They perform no network request.

Rollback removes the Explain module and the secret-free pool diagnostic view. It changes no
RouteSnapshot publication, Credential Secret, Health/Quota state, scheduler cursor, lease,
persistence schema, Provider request, or public HTTP API.
