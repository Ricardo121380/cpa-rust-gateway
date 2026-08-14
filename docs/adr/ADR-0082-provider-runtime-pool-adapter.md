# ADR-0082: Provider runtime account-pool adapter

- Status: Accepted; P13-06B local implementation and review complete, phase gate pending
- Date: 2026-08-15
- Task: P13-06B
- Contract: [BC-MGMT-015](../contracts/BC-MGMT-015-provider-runtime-pool-adapter.md)
- Evidence: [P13-06B report](../reports/p13-06b-provider-runtime-pool-adapter.md)
- Scope: Provider-owned runtime composition into the P13-06A management facade

## Context

P13-06A established a Provider-neutral, secret-free snapshot facade and the protected
`GET /admin/operations/provider-account-pools` contract. That facade deliberately has no
knowledge of a Provider's credential shape or runtime scheduler. The repository already has
those primitives in separate owners: `EndpointCredentialPools` owns bounded leases and
concurrency, `RuntimeHealthRegistry` owns exact account/circuit state, `RuntimeQuotaRegistry`
owns exact account/model quota state, and Provider stores own authentication metadata and
maintenance deadlines.

The next slice must make the management page useful without creating a second scheduler or
turning a read request into a Provider request. The initial adapters cover the existing native
Grok Build/Web/Console pools and ordinary Provider-owned pools for ChatGPT/Codex and Krill. Each
adapter remains responsible for its own credential shape, endpoint binding, egress policy,
failure domain, and lifecycle policy.

## Decision

Introduce an explicit Provider adapter at the application runtime-composition boundary. The
adapter receives already compiled, Provider-owned runtime material and publishes one bounded
`ProviderAccountPoolSnapshot` to the P13-06A facade:

1. join redacted Provider account metadata to the compiler-approved Provider/Endpoint binding;
2. join secret-free pool diagnostics (`active_leases`, priority, weight and concurrency);
3. read exact Endpoint/Credential Health and exact Endpoint/Credential[/Model] Quota at one
   observation time;
4. map the resulting values to the existing P13-06A authentication and runtime status enums;
5. publish the complete page atomically with one `snapshot_id` and `observed_at_ms`.

The production composition passes the adapter the same `Arc<EndpointCredentialPools>`,
`Arc<RuntimeHealthRegistry>`, and `Arc<RuntimeQuotaRegistry>` used by request routing. There is no
copied scheduler state or second Health/Quota registry. The adapter does not select, lease,
refresh, or recover an account while building a snapshot. A lease held by a request may change
immediately after the snapshot is observed; `active_leases` is explicitly a point-in-time
diagnostic and is never an admission promise.

Ordinary Endpoint/Credential bindings, including the existing ChatGPT/Codex and Krill graphs, are
compiled from the active `ControlPlaneConfiguration`. Provider-native Grok Build/Web/Console
accounts are joined through the exact configured `GrokAccountEndpointBinding` and redacted native
metadata. Native account secrets, import batches, Cookies, OAuth values, and endpoint URLs never
enter the descriptor. Build expiry can be read from the compiled pool diagnostic; Web/Console
expiry remains unknown unless a later Provider-owned metadata contract supplies it.

The native descriptor does not copy a fixed cooldown deadline. Native compilation seeds the shared
`RuntimeHealthRegistry`, and every projection asks that registry with the adapter's live
observation clock; cooldown therefore expires naturally at query time. Native metadata and its
compiled pool are taken from the same runtime-row compilation, so a row cannot combine metadata
from one observation with capacity from another.

Snapshots have two deliberately separate lifetimes. A current snapshot is fresh for five seconds,
which bounds the amount of live-state recomputation while a page sequence is being read. Up to
eight prior snapshots are retained for at most two minutes so an in-flight cursor can finish even
after a refresh; a cursor whose snapshot has aged beyond that retention window receives the safe
`409` conflict. Each adapter instance has a random process nonce in its snapshot namespace, so a
restart or a second adapter cannot accidentally accept a cursor generated at the same millisecond
by an earlier instance. This is an observation cache, not durable configuration.

The public status projection keeps authentication lifecycle separate from runtime availability:

- authentication: `active`, `reauth_required`, `disabled`, `expired`;
- runtime: `available`, `cooling`, `circuit_open`, `quota_blocked`,
  `unauthorized`, `recovery_in_flight`, `expired`.

No new wire status is added in this slice without a separate contract review. Until such a
review exists, the public `unauthorized` value is a deliberately coarse projection for an
account/credential that cannot currently be scheduled; the internal Health registry remains the
source of the more precise `AccountForbidden` versus `CredentialUnauthorized` distinction.
When several exact Health/Quota targets apply to one row, the read model chooses the strongest
observed state in this order: `expired`, `recovery_in_flight`, `unauthorized`, `circuit_open`,
`cooling`, `quota_blocked`, then `available`. This aggregation changes no runtime state.

## Provider and channel isolation

Provider identity and channel/Endpoint identity are joined before publication and are validated
as an exact pair. A missing or failed Grok Build account cannot cause a Console, Web, ChatGPT,
Codex, or Krill account to be selected. The adapter never converts a credential between Provider
shapes, shares a proxy/egress decision, or performs cross-Provider fallback. Duplicate
Provider/Channel/Account identities and duplicate native Provider/Endpoint bindings fail before a
snapshot is published.

An `active` + `enabled` + `available` row must have one exact compiled-pool diagnostic. Its account
kind, priority, weight, and concurrency must match the descriptor byte-for-byte; a missing entry or
any drift makes the source unavailable rather than publishing a misleading capacity row. Inactive,
disabled, or unavailable metadata may remain visible as a non-admissible state without a runtime
diagnostic. Query filters are fingerprinted with length-prefixed values, and all opaque IDs are
bounded to 128 characters, preventing delimiter collisions and unbounded cursor material.

## Restart and consistency

The adapter is rebuilt by the normal application composition from the active configuration,
Provider-native metadata, and newly compiled runtime pools. Active leases and the fresh/retained
snapshot cache are process-local and are never reconstructed from an earlier process. A newly
built adapter therefore starts with a nonce-scoped snapshot identity and cannot accept an old
process's cursor, even if both observations occur in the same millisecond.

P13-06B does not add a new file-backed restart E2E and does not claim persistence for process-local
leases or cache entries. Existing P4/P12 persistence and native-pool tests remain regression
evidence; an explicit restart database matrix, if required for later operator mutations, belongs
to P13-06C or the P13 phase review.

Health and Quota reads use one explicit observation time and exact keys. Account-level or
model-level state for one Endpoint/Credential never blocks a sibling account, another Endpoint,
or another Provider.

## Security and non-goals

- No Provider HTTP request, OAuth exchange, refresh, reauth, Autoreg scheduler, or quota fetch is
  started by a snapshot request.
- No credential plaintext/ciphertext, URL, Header/Cookie, request body, Client Key digest, or
  Provider response is returned or logged.
- No second selector/scheduler is introduced; request routing continues to use the existing
  scheduler and leases.
- Automatic refresh/reauth/replenishment remains P13-12; generic Provider proxy pools remain
  P13-11; cost-aware routing remains P13-07; production deployment/traffic unchanged.
- A Provider source error returns a value-free unavailable result; partial cross-Provider data is
  not substituted. If management descriptor, capacity, or metadata construction fails, the
  composition injects a `RejectingProviderAccountPoolFacade` and the management route returns
  `503`; this fail-soft management projection does not stop or mutate the serving data plane.
- No server, production Config Version, credential pool, or GitHub Delivery Gate is changed by
  this local implementation slice.

## Consequences

The management runtime page can consume a stable live-state shape while the application keeps
Provider-specific implementations out of `gateway-control`. The implementation adds only narrow,
secret-free compiled-pool diagnostics for credential kind, expiry, scheduling values, and current
lease count; it does not decrypt credentials in the management handler.

Focused adapter, runtime-composition, pool-diagnostic, P13-06A management, and existing P12 Grok
regressions support the local-complete decision. The final local matrix contains 53 tests: 8
adapter, 3 runtime-composition, 7 upstream pool, 5 control, 1 management-inventory HTTP, 8
OpenAPI contract, and 21 native Grok regression tests. The review-fix cases cover model-scoped
Health/Quota, fresh-versus-retained cursors, diagnostic drift, nonce isolation, bounded filters,
dynamic cooldown expiry, and management failure isolation. These results do not constitute a
real two-account test for every Provider, a new database-restart E2E, a production rollout, or
the formal P13 Delivery Gate.

## Alternatives considered

- A second management-only account store was rejected because it would drift from the actual
  scheduler, Health, and Quota objects.
- Calling Providers from the management handler was rejected because a read request must not
  create upstream cost, authentication side effects, or a new availability dependency.
- Flattening every Provider into one credential type was rejected because Grok native accounts and
  ordinary ChatGPT/Codex/Krill bindings have different lifecycle and egress ownership.

## Validation and rollback

Validation is recorded in the P13-06B report. Rollback removes the composition adapter injection
and returns the route to P13-06A's fail-closed `RejectingProviderAccountPoolFacade`; it does not
require data migration, credential rewriting, or production rollback.
