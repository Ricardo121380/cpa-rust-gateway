# BC-MGMT-015: Provider runtime account-pool adapter

## Contract status

Accepted for P13-06B. Local implementation and focused review are complete; the formal P13 phase
Delivery Gate is pending.

## Source and endpoint

The existing protected endpoint remains:

```text
GET /admin/operations/provider-account-pools
```

The handler consumes an injected Provider adapter/facade. It does not contact a Provider and it
does not read or decrypt credential material. The selected source must represent one coherent,
bounded observed snapshot. P13-06B does not add a route or response field; it replaces the
P13-06A production fail-closed source with the application-composed read-only adapter.

## Required source composition

For each configured Provider-owned account, the adapter joins:

1. redacted Provider account metadata and authentication lifecycle;
2. the exact Provider-to-channel/Endpoint binding;
3. the existing immutable Endpoint Credential pool diagnostics;
4. exact runtime Health state for Endpoint/Credential and, when applicable, Endpoint/Credential/
   Model;
5. exact runtime Quota state and the Provider's maintenance deadlines.

The application must inject the exact `Arc<EndpointCredentialPools>`, `Arc<RuntimeHealthRegistry>`,
and `Arc<RuntimeQuotaRegistry>` used by request routing. Ordinary Endpoint/Credential bindings are
derived from the active configuration; native Grok Build/Web/Console descriptors and their pool
entries are derived from the same native runtime-row compilation (`GrokAccountMetadata` plus exact
Provider/Endpoint bindings), never from separately timed reads. The adapter must use one
`observed_at_ms` for all rows and publish a unique, nonce-scoped `snapshot_id`. A source failure is
all-or-nothing for the snapshot; no other Provider may fill the missing rows.

The production snapshot has a five-second freshness lifetime, separate from cursor retention. A
fresh read within that window reuses the current snapshot. On refresh, up to eight prior snapshots
remain available for at most two minutes so an in-flight keyset page can finish against one
observation. A cursor whose snapshot is older than that retention window returns safe `409` rather
than mixing observations. Each adapter instance includes a random nonce in its snapshot namespace,
preventing a restart or parallel adapter from accepting a same-millisecond cursor from another
instance.

## Response projection

The P13-06A response shape remains closed and secret-free. Each item may contain only:

- opaque `provider_id`, `channel_id`, `account_id`, and bounded `account_kind`;
- independent `auth_status` and `runtime_status` enum values;
- `enabled`, `priority`, `weight`, `max_concurrency`, and point-in-time `active_leases`;
- optional `expires_at_ms`, `refresh_due_at_ms`, and `quota_sync_due_at_ms`.

The response must not contain credential bytes/digests, endpoint URL/path, Cookie/SSO/Bearer,
Headers, request bodies, client-key digests, raw quota windows, or Provider response values.

## Status and scheduling invariants

- `disabled` and expired accounts are visible as state, but never enter an eligible lease.
- `reauth_required` accounts remain isolated and are not repaired by this adapter.
- Cooling, open Circuit, exhausted Quota, and recovery-in-flight states map to the corresponding
  runtime status without changing the authentication status.
- A lease held by a request increments only its exact account's point-in-time counter and is
  released on drop; saturation skips only that account within the same Provider/channel pool.
- Priority and weight remain the existing scheduler's semantics. The adapter does not reorder,
  lease, or mutate pool cursors.
- Health and Quota keys are exact Endpoint/Credential[/Model] keys. A sibling account, another
  channel, or another Provider remains unaffected.
- Public `unauthorized` is the current coarse wire projection; the internal Health distinction
  between forbidden and credential-unauthorized must not be silently used for cross-Provider
  fallback.
- Native cooldown is not copied into the descriptor. The shared Health registry is queried with
  the current observation clock, so cooldown naturally expires when the live clock passes its
  deadline.
- An `active` + `enabled` + `available` row requires one exact compiled-pool diagnostic. The
  diagnostic's account kind, priority, weight, and max concurrency must exactly match the
  descriptor; missing or drifted entries make the source unavailable. Inactive, disabled, or
  unavailable rows may remain visible without a diagnostic because they are not admissible.
- When more than one exact Health/Quota target applies to a row, the strongest public projection
  wins: `expired` > `recovery_in_flight` > `unauthorized` > `circuit_open` > `cooling` >
  `quota_blocked` > `available`.

## Process and rebuild invariants

- The adapter is rebuilt only through the existing application runtime composition.
- Process-local leases and fresh/retained cached snapshots are not persisted or fabricated after a
  rebuild; the per-adapter nonce makes the new cursor namespace distinct.
- Duplicate account identities or duplicate native Provider/Endpoint bindings fail closed before
  publication.
- Query fingerprints use length-prefixed filter values, and every opaque Provider/channel/account
  identifier is bounded to 128 characters.
- If management descriptor, capacity, or metadata construction fails, the composition injects a
  rejecting facade and the protected route returns `503`; the serving data plane remains available
  and is not replaced, stopped, or mutated by this management failure.
- No Provider request, refresh worker, reauth executor, or scheduler mutation runs.
- P13-06B does not claim a new file-backed restart E2E. Existing durable Health/Quota and native
  account-store suites remain regression evidence; a new database restart matrix is outside this
  slice.

## Verification matrix

The local P13-06B evidence is intentionally bounded. It does not claim a real two-account
production test for every Provider and does not reinterpret existing P12 Provider tests as new
P13 live evidence.

| Matrix | Required proof | Evidence status |
|---|---|---|
| source composition | ordinary active Config binding plus redacted native Grok metadata map to exact Provider/Channel/Account rows | LOCAL PASS; no per-Provider two-account production claim |
| lease/concurrency | same compiled pool reports exact live lease count and immutable priority/weight/concurrency; drop restores capacity in existing pool regression | LOCAL PASS |
| Health | cooling, circuit-open, unauthorized, recovery-in-flight and unaffected sibling projection | LOCAL PASS |
| Quota | exact account quota-blocked projection; model-scoped Health/Quota target reads remain exact and sibling-isolated | LOCAL PASS |
| expiry/auth | compiled expiry plus reauth/disabled state map without acquiring or repairing a credential | LOCAL PASS |
| cursor/cache | five-second freshness, eight-snapshot/two-minute retention, old-cursor `409`, and per-adapter nonce namespace | LOCAL PASS |
| diagnostics | active/enabled/available exact-entry requirement; kind/priority/weight/concurrency drift rejected; inactive rows remain non-admissible | LOCAL PASS |
| filter/ID bounds | length-prefixed filter fingerprint and 128-character opaque-ID boundary | LOCAL PASS |
| management failure isolation | descriptor/capacity/metadata build failure maps to rejecting facade/`503` without serving-plane mutation | LOCAL PASS |
| Provider isolation | exact Provider filter, duplicate descriptor rejection, and no cross-Provider fallback | LOCAL PASS |
| security | pool diagnostics contain no secret; snapshot has no URL/body/digest/raw quota; no Provider send path | LOCAL PASS / REVIEWED |
| protected HTTP | Management Key, exact filters, pagination, no-store and safe error regression | P13-06A REGRESSION |
| restart database E2E | no new file-backed restart matrix in P13-06B | NOT CLAIMED |
| real/production Provider matrix | existing P12 Grok suites are regression evidence only | NOT RUN; NOT REQUIRED FOR THIS LOCAL SLICE |

## Out of scope

P13-06B does not implement automatic refresh/reauth/replenishment, generic proxy pools,
cost-aware routing, operator actions/failure feedback, public UI changes, production/server
mutation, or the P13 phase Delivery Gate. Operator actions and explicit failure feedback are the
next P13-06C slice.
