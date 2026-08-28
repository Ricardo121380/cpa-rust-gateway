# ADR-0022: Endpoint-Credential Model Catalog discovery singleflight

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-21` |
| Task / Matrix / Contract references | `P4-01`; `E20`, `G28`, `L09`, `L10`, `L33`; [BC-CATALOG-001](../contracts/BC-CATALOG-001-endpoint-credential-catalog-singleflight.md) |

## Context

P2-06 deliberately keeps `gateway-catalog` as a storage-neutral, explicitly injected view for
Route compilation. It does not discover Models, retain source freshness, or decide whether a
Provider-wide model list is an entitlement for a particular Credential. P4 must add that discovery
boundary without turning an Endpoint-level response into a Credential-level claim.

Concurrent management or refresh callers can otherwise issue duplicate discovery work for one
Credential. That increases rate-limit and quota pressure while providing no new evidence. Sharing
by Endpoint alone is unsafe: two Credentials at the same URL can have different permissions,
account state, and visible Models. P4-01 needs a bounded in-process coordination primitive, but
not a result cache; P4-02 owns durable snapshots, freshness, and the last-success fallback.

## Decision

- `ModelCatalogTarget` is the sole singleflight identity and contains exactly one stable
  `EndpointId` and one stable `CredentialId`. It carries neither an Endpoint URL nor Credential
  material. `DiscoveredModel` admits only a non-empty upstream Model string.
- `ModelCatalogSource` extends the existing `ProviderAdapter` identity boundary. A Provider-owned
  source receives the stable target and owns the private mapping to actual Endpoint and Credential
  material. The scheduler itself constructs no HTTP request and exposes no Secret-bearing value.
- `ModelCatalogScheduler` owns one source and a Tokio-protected `BTreeMap` of in-flight targets.
  The first caller starts a detached discovery task; concurrent callers with the exact same target
  subscribe to one `watch` result. A same-Endpoint, different-Credential target always receives a
  separate source call.
- The detached task continues after its initiating caller is cancelled, so a later follower cannot
  be stranded. It removes the in-flight entry before publishing the result. Existing subscribers
  receive that result; a later caller begins new discovery rather than reading a scheduler cache.
- Successful source output is sorted and deduplicated before delivery. Safe `GatewayError` values
  are shared only with callers joined to that exact flight. Successes and failures are both absent
  from the scheduler after completion; P4-02 is the first task allowed to retain snapshot state.

## Consequences

Discovery concurrency is narrowed to one Provider source and one exact non-secret identity pair.
An upstream account cannot gain Models merely because another Credential shares its Endpoint, and
concurrent refreshes cannot multiply one source operation. The source must still enforce any
Provider-specific authorization semantics; singleflight is coordination, not authorization.

The map mutex is on the control-plane discovery path only. P4-01 does not wire discovery into
request routing, `/v1/models`, SQLite, health/circuit mutation, quota accounting, retries,
failover, Event emission, or transport. It does not persist source output and cannot claim a
Fresh/Stale/Expired state or last-success fallback.

## Alternatives considered

- Keying by `EndpointId` only was rejected because it conflates potentially different Credential
  entitlements and violates the per-Endpoint-plus-Credential Catalog requirement.
- Caching a successful vector in this scheduler was rejected because it would silently invent
  freshness, expiry, and failure-retention policy before P4-02 defines those semantics.
- Letting the first caller own and cancel the only source future was rejected because cancellation
  could strand a follower that had legitimately joined the same discovery operation.
- Sharing errors beyond the active flight was rejected because transient discovery errors must not
  become a Credential-wide persistent state or substitute for P4-02's last-success rules.

## Validation and rollback

Synthetic Tokio tests prove one source call for two concurrent equal targets, two independent calls
and Credential-specific results for the same Endpoint with different Credentials, cancellation of
the initiating caller without stranding a follower, and sharing-but-not-retaining a failed result.
They use no URL, network client, Credential material, or real Provider request. Local Fast and
Full gates plus the corresponding GitHub code Gate are required before this Task is `DONE`.

Rollback removes the scheduler, source contract, Tokio dependency, and tests. It changes no
database schema, persisted Catalog record, public API, deployed endpoint, Credential state, or
P0-P3 behavior.
