# BC-CATALOG-001: Endpoint-Credential Model Catalog discovery singleflight

| Field | Value |
|---|---|
| Contract | `BC-CATALOG-001` |
| Task | `P4-01` |
| Status | `DONE` after local Fast/Full, GitHub code Gate, and the current docs-only closeout Gate |
| Domain | Provider-owned Model discovery coordination before Catalog snapshot persistence |

## Entry and boundary

`ModelCatalogScheduler::new` accepts one `Arc<dyn ModelCatalogSource>`. A caller supplies a
`ModelCatalogTarget` made from exactly one stable `EndpointId` and one stable `CredentialId`, then
awaits `synchronize`. `ModelCatalogSource::models` is the only Provider boundary that can perform
Provider-specific discovery. It receives the stable identifiers, while the concrete source retains
the private Endpoint address and Credential material.

This contract does not resolve a target from the control-plane repository, persist a Model list,
assign Fresh/Stale/Expired, retain a last-success snapshot, calculate diffs, change a Credential
state, open a route, publish `/v1/models`, construct HTTP in the scheduler, retry/fail over, or
send events. P4-02 through P4-08 own those behaviors.

## Preconditions

- Each target contains valid stable non-secret identifiers. Callers do not substitute a URL,
  header, API key, account name, or raw upstream response for either identifier.
- A scheduler is used from a running Tokio runtime and keeps one Provider-owned source for its
  lifetime. The source is responsible for resolving the stable identifiers to an admitted,
  Provider-specific discovery operation.
- A source returns only `DiscoveredModel` values, whose constructor rejects an empty upstream
  Model string. It returns a secret-free `GatewayError` on failure.
- The caller treats a returned Model list as discovery output only. It is not a public model list,
  an entitlement for another Credential, or a freshness/persistence assertion.

## Required discovery behavior

| Concern | Required behavior |
|---|---|
| Key | Share only when both `EndpointId` and `CredentialId` are exactly equal. |
| Equal-target flight | The first active caller starts one source operation. Every concurrent equal-target caller receives the normalized result of that operation. |
| Credential isolation | Different Credentials always start separate source operations, even when their Endpoint IDs are equal. |
| Initiator cancellation | Cancelling the initiating caller drops only that caller's wait. The detached operation remains available to existing or later followers. |
| Normalization | Sort and deduplicate `DiscoveredModel` values before every receiver observes success. |
| Completion | Remove the in-flight key before notifying receivers. A caller arriving after completion starts a new source operation. |
| Result retention | Retain neither success nor failure after the in-flight operation completes. |

## Invariants

- No singleflight map key lacks a Credential ID, and no same-Endpoint union or cross-Credential
  Model list is constructed by the scheduler.
- The scheduler's output ordering is deterministic for a fixed set of `DiscoveredModel` values.
- One source failure is shared only by callers already joined to the exact active target. It does
  not mutate Credential, Endpoint, Catalog, health, quota, circuit, or Route state.
- The scheduler stores no URL, Credential value, Authorization Header, request body, response
  body, raw source diagnostic, or persistent snapshot. It performs no network I/O itself.
- No request-time Route selection depends on this P4-01 coordination map. P2's injected Catalog
  view remains the existing routing boundary until later P4 snapshot integration.

## Error semantics

| Condition | Result |
|---|---|
| Source discovery failure | Every caller joined to that target's active flight receives the same safe `GatewayError`; no result cache is left behind. |
| Detached flight ends without publishing | `InternalError/Internal`; no source diagnostic is exposed. |
| Empty source Model value | Cannot be represented as `DiscoveredModel`; construction returns `CatalogViewError::EmptyUpstreamModel` before source success can be returned. |
| Initiating caller cancellation | That caller is cancelled; the in-flight source task and any follower remain independent. |

## Corresponding tests

- `gateway-catalog::tests::same_endpoint_and_credential_share_one_concurrent_discovery`
- `gateway-catalog::tests::same_endpoint_with_different_credentials_never_share_discovery`
- `gateway-catalog::tests::initiating_caller_cancellation_does_not_strand_a_later_follower`
- `gateway-catalog::tests::failed_discovery_is_shared_but_not_retained_as_a_result_cache`
- `cargo test --locked -p gateway-catalog`
- `./scripts/check.sh fast` and `./scripts/check.sh full`
