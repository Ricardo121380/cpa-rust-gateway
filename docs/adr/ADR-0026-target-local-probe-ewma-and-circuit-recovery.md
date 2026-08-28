# ADR-0026: Target-local probe EWMA and controlled Circuit recovery

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-21` |
| Task / Matrix / Contract references | `P4-04`; `E08`、`E09`、`E11`、`E12`、`D20`、`D24`、`G20`、`H19`、`L30`; [BC-HEALTH-002](../contracts/BC-HEALTH-002-target-local-probe-ewma-and-circuit-recovery.md) |

## Context

P3-05 established bounded, sharded Endpoint and Endpoint/Credential runtime availability but
intentionally deferred active health-probe interpretation, model-specific state, EWMA, and
half-open Circuit recovery. A model can fail only for one Credential at one protocol-specific
Endpoint; treating it as an Endpoint-wide or global model failure would remove healthy routing
capacity. Conversely, an expired Circuit must not silently admit ordinary client traffic.

P4 needs diagnostic health measurements and a controlled recovery path without putting HTTP,
Provider details, SQLite, URLs, request/response bodies, or Secret material into the router's
request-time state. No new real Provider probe is authorized for this Task.

## Decision

- `RuntimeHealthProbeTarget` represents exactly one non-secret Endpoint, Endpoint/Credential, or
  Endpoint/Credential/upstream-model scope. A model-scoped target requires all three identities and
  rejects an empty model label. `RuntimeHealthKey` receives the matching model-scoped variant.
- `RuntimeHealthProbeRegistry` is an in-memory 64-shard bounded registry. It accepts only a
  sanitized terminal success/failure plus explicit observation time and latency. It maintains
  deterministic fixed-point success and latency EWMA values; the default new-sample weight is 200
  per mille. It stores no URL, status text, Header, Credential, request body, response body, or
  transport diagnostic, and it does not execute a network probe itself.
- Runtime selection continues to fail closed. It checks Endpoint state before a pool, then exact
  Endpoint/Credential and Endpoint/Credential/model state before reserving a lease. A
  model-specific Circuit therefore skips only the affected binding and does not alter immutable
  RouteSnapshot/public-model visibility.
- An open Circuit may issue exactly one `RuntimeHealthCircuitProbe` ticket after its explicit
  retry instant. During that half-open interval ordinary scheduling remains unavailable. A valid,
  unexpired successful ticket closes the Circuit; a failed ticket reopens it at an explicit future
  retry instant. An expired, superseded, or mismatched ticket fails closed and cannot overwrite a
  newer Circuit. Probe EWMA insertion and ticket completion are target-local coordinated.
- An authorized external executor may later supply the sanitized outcomes. P4-04 does not decide
  HTTP request shape, periodic scheduling, quota, 403/429 classification, persistent health
  history, management HTTP API, Route Explain rendering, telemetry export, or log-body handling.

## Consequences

Health measurements are deterministic and directly testable without wall clock or network access.
Model-specific failures no longer have to be widened to an entire Credential or Endpoint, while the
existing Circuit safety property is preserved: a timer opens an opportunity for one controlled
probe, never for arbitrary client traffic.

P4-05 remains responsible for quota/source/reset semantics. P4-06 will render health and exclusion
evidence in Route Explain. P4-07/P4-08 own durable event/timeline and telemetry export. The
transport-neutral boundary prevents a Provider-specific status, body, or Secret from contaminating
router state.

## Alternatives considered

- Automatically close a Circuit once `retry_after_ms` passes: rejected because a timer is not
  recovery evidence and would admit arbitrary client traffic.
- Share a model health key by model name alone or by Endpoint/model only: rejected because a
  Credential can lack one model while a sibling Credential remains healthy.
- Store floating-point EWMA or wall-clock samples: rejected because deterministic tests and
  explainable integer snapshots are required.
- Build/send probes in `gateway-router`: rejected because the router is transport-neutral and
  adding HTTP/Provider/Secret access would violate its dependency and hot-path boundary.
- Persist probe state in this Task: rejected because P4-07 owns asynchronous durable event writing
  and restart recovery.

## Validation and rollback

Synthetic tests prove model-target EWMA isolation, explicit time-regression rejection, one
half-open ticket, stale-ticket rejection, successful close, failed reopen, and pre-lease selection
of a healthy sibling Credential. They execute no Provider request. Rollback removes the new probe
registry and model Circuit predicate; it changes no database schema, RouteSnapshot, public model,
Credential Secret, endpoint configuration, quota state, or external traffic.
