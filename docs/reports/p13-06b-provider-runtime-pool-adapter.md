# P13-06B Provider runtime account-pool adapter

## Status

`LOCAL_PASS_PENDING_PHASE_GATE` — the runtime adapter, composition injection, focused local tests,
and review are complete. The formal P13 Delivery Gate remains deferred to P13 closeout. This
report intentionally does not claim a new live Provider, production, or server result.

## Objective

Connect existing Provider-owned runtime pools to the P13-06A secret-free management facade without
duplicating the scheduler. The implemented source matrix is:

- native Grok Build, Web, and Console pools;
- ordinary ChatGPT/Codex and Krill endpoint/credential pools;
- existing `EndpointCredentialPools`, `RuntimeHealthRegistry`, and `RuntimeQuotaRegistry` as the
  shared runtime primitives.

Each Provider retains its credential shape, channel binding, egress policy, failure domain, and
lifecycle policy. The adapter publishes a single bounded snapshot and performs no Provider I/O.

## Implemented

- `apps/gateway` builds one `ProviderAccountPoolAdapter` during the existing runtime composition
  and injects it into the protected management service.
- The adapter observes the same `Arc<EndpointCredentialPools>`, `Arc<RuntimeHealthRegistry>`, and
  `Arc<RuntimeQuotaRegistry>` used by request scheduling; no management-only scheduler or copied
  runtime registry was introduced.
- Ordinary active Config bindings provide rows for the existing ChatGPT/Codex/Krill-style pools.
  Native Grok Build/Web/Console rows are derived from redacted native metadata and exact
  Provider/Endpoint bindings.
- A narrow `CredentialPoolEntrySnapshot` exposes only credential kind, priority, weight,
  concurrency, expiry, and point-in-time lease count. Its debug projection remains secret-free.
- Health and Quota are read by exact Endpoint/Credential and configured Endpoint/Credential/Model
  keys. Authentication state remains independent from runtime availability; native cooldown is
  evaluated from the shared Health registry at the live observation time and naturally expires.
- Freshness and cursor retention are separate: the current snapshot is fresh for five seconds,
  while up to eight prior snapshots remain usable for at most two minutes. An old cursor then
  receives `409`; each adapter instance uses a random nonce so restart/same-millisecond cursor
  collisions are rejected.
- Native metadata and pool capacity are compiled from the same runtime-row snapshot. An
  active+enabled+available row requires an exact diagnostic whose kind, priority, weight, and
  concurrency match; missing or drifted diagnostics fail the management source. Inactive,
  disabled, or unavailable rows remain non-admissible metadata rather than inventing capacity.
- Query fingerprints are length-prefixed and opaque identifiers are bounded to 128 characters.
- Missing active configuration retains the P13-06A fail-closed source. If management descriptor,
  capacity, or metadata construction fails, composition injects a rejecting facade and the route
  returns `503`; this does not stop or mutate the serving data plane, and no other Provider is
  substituted.

## Frozen boundaries

- read-only snapshot composition; no lease acquisition, cursor mutation, or scheduler replacement;
- exact Provider/channel/account attribution and no cross-Provider credential or egress fallback;
- no credential plaintext/ciphertext, URL, Header/Cookie, body, Client Key digest, raw quota
  window, or Provider response in the snapshot or logs;
- no automatic refresh/reauth/replenishment (P13-12), generic proxy pool (P13-11), routing policy
  changes (P13-07), production deployment, or formal P13 Delivery Gate.

## Acceptance matrix

| Area | Minimum evidence | Status |
|---|---|---|
| source composition | ordinary active graph produces a Provider/Channel/Account row; native Grok metadata maps through exact binding | LOCAL PASS |
| lease semantics | live lease count is attributed only to its exact account; existing scheduler/drop regressions preserve priority/weight/capacity | LOCAL PASS |
| health/circuit | cooling, circuit-open, unauthorized and recovery-in-flight project independently while an available sibling remains available | LOCAL PASS |
| quota | exact account quota-blocked projection; model-scoped Health/Quota reads remain exact and sibling-isolated | LOCAL PASS |
| expiry/auth | compiled expiry becomes `expired`; reauth and disabled remain distinct without a lease or repair side effect | LOCAL PASS |
| cursor/cache | five-second freshness, eight-snapshot/two-minute retention, old-cursor conflict, and per-adapter nonce namespace | LOCAL PASS |
| diagnostic consistency | active/enabled/available exact diagnostic; kind/priority/weight/concurrency drift rejected; same native runtime-row snapshot for metadata and pool | LOCAL PASS |
| filter/ID bounds | length-prefixed filter fingerprint and 128-character opaque-ID boundary | LOCAL PASS |
| management failure isolation | descriptor/capacity/metadata build failure produces rejecting facade/`503` without serving-plane mutation | LOCAL PASS |
| Provider isolation | exact Provider filter and duplicate identity/binding rejection; no cross-Provider fallback or credential conversion | LOCAL PASS |
| security | no Secret/URL/Header/Cookie/body/digest/raw-quota fields; no Provider send/refresh/scheduler path | LOCAL PASS / REVIEWED |
| restart database E2E | no new file-backed restart test was added in this slice | NOT CLAIMED |
| real multi-account matrix | no claim of two real accounts for every Provider; existing P12 Grok suites are regression evidence only | NOT RUN |

## Focused local gate

```text
cargo test --locked -p gateway provider_account_pool_adapter
cargo test --locked -p gateway invalid_management_projection_does_not_block_the_serving_pool
cargo test --locked -p gateway active_singleton_graph_builds_an_encrypted_runtime_without_a_send
cargo test --locked -p gateway native_grok_metadata_maps_to_the_configured_provider_channel_without_a_send
cargo test --locked -p gateway-upstream credential_pool
cargo test --locked -p gateway-control provider_account_pool
cargo test --locked -p gateway-http-actix --test p13_04_management_inventory
cargo test --locked -p gateway-http-actix --test p10_01_management_openapi_contract
cargo test --locked -p provider-grok --test p12_10b_native_account_pool \
  --test p12_10c_native_account_scheduling --test p12_10d_native_account_workers
npm --prefix web/prism run check
cargo clippy --locked -p gateway -p gateway-upstream -p gateway-control \
  -p gateway-http-actix -p provider-grok --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
./scripts/check.sh docs
git diff --check
```

Result receipt: `FINAL_LOCAL_PASS_53_CHECKS`. The final focused local matrix contains 53 tests:
8 adapter, 3 runtime-composition, 7 upstream pool, 5 control, 1 management-inventory HTTP,
8 OpenAPI contract, and 21 native Grok regression tests. The review-fix cases cover
model-scoped Health/Quota, fresh-versus-retained cursors, diagnostic drift, nonce isolation,
bounded filters, dynamic cooldown expiry, and management failure isolation.

The existing P12 native Grok pool/scheduling/worker suites remain regression evidence, not new
P13 live claims. The focused gate remains local and bounded; no GitHub Delivery Gate was started.

## Review

- Runtime ownership: PASS — management observes the exact routing pools and registries.
- Side effects: PASS — snapshot construction has no Provider send, lease acquisition, refresh,
  reauth, scheduler, Store write, or production mutation.
- Isolation: PASS — Provider/Channel/Account joins are exact, duplicate identities fail closed,
  and no Provider fills another Provider's row.
- Secret boundary: PASS — descriptors and diagnostics contain metadata only; no secret-bearing
  response or debug field was added.
- Consistency: PASS — five-second freshness is separate from eight-snapshot/two-minute cursor
  retention; old cursors fail safely only after retention, and the nonce prevents restart
  collisions.
- Diagnostic integrity: PASS — available rows require exact compiled diagnostics and matching
  kind/priority/weight/concurrency; native metadata and pool capacity use one runtime-row snapshot.
- Management failure isolation: PASS — descriptor/capacity/metadata build failures become a
  rejecting facade and `503` without stopping or mutating the serving data plane.
- Evidence scope: PASS — this report does not claim a real per-Provider two-account matrix or a
  new file-backed restart E2E.

## Risks and follow-up

The compiled pool diagnostic now exposes the narrow metadata needed for this read model. It does
not expose credential revision because the public P13-06A item has no revision field and adding
one would require a separate contract change. If the public runtime enum must distinguish
Provider-forbidden from credential-unauthorized, that likewise requires contract review rather
than an undocumented status change. Native cooldown remains owned by shared Health rather than a
descriptor deadline; future changes must preserve the query-clock expiry and exact diagnostic
invariants above.

The next slice is P13-06C: bounded operator actions and explicit failure feedback with existing
management admission/audit/revision boundaries. Automatic reauth remains P13-12; generic proxy
pools remain P13-11.
