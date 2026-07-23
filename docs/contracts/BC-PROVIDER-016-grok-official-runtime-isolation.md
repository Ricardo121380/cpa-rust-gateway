# BC-PROVIDER-016 Grok Official runtime quota and failure isolation

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-016` |
| Task | `P8-05` |
| ADR | [ADR-0058](../adr/ADR-0058-grok-official-runtime-isolation.md) |
| Matrix | `C01`、`C03`、`C31`、`C33`、`F07`、`G24-G27` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P7-G7-001`; no Official E2E has run |
| Domain | Exact Official runtime quota handoff, stateless continuity, and failure remediation ownership |

## Preconditions and bounds

1. The caller explicitly injects one Router-owned `RuntimeQuotaRegistry`, Official Endpoint ID,
   Official Credential ID, and safe Header metadata. Direct state methods take an explicit
   observation time; the opt-in adapter handoff obtains it only from the injected/default
   Router clock. This boundary reads no OAuth, API-key source, request body, Header value,
   database, server, proxy, browser, route, or account state.
2. It accepts only P8-03's complete bounded Requests/Tokens windows and `Retry-After` metadata.
   Reset addition must be representable and observation time strictly positive. Raw Header names
   and values remain unavailable after P8-03 parsing.
3. `CR-P7-G7-001` permits local evidence only. Official E2E, P8 closeout, Delivery Gate, merge,
   release, and `DONE` remain blocked pending G7 and the P7 Delivery Gate.

## Required behavior

| Concern | Required behavior |
|---|---|
| Exact state target | Construct only binding-wide `(Official Endpoint ID, Official Credential ID)` quota snapshots; source is `Header`, confidence is `Observed`, and labels are `official.requests` / `official.tokens`. No Build/Web target is read or written. |
| Complete metadata | No complete Official resource window means no state write. Each complete resource maps its fixed limit/remaining/reset to a checked absolute reset time. A malformed value fails before registry mutation. |
| `429` | Only a classified Official `429` calls the generic exact-target quota cooldown. Positive `Retry-After` is Header/Observed; missing/zero uses the bounded generic Estimated/Estimated fallback. |
| Other failures | `401` permits Official credential replacement only. Unknown `403` is egress-local and non-mutating. `408`/`5xx` request only Official endpoint cooling. Other statuses remain permanent/non-mutating. No action names another Provider. |
| Affinity and replay | Official continuity is `Stateless`: it has no cache affinity, response ownership, or reasoning replay and cannot import a Build OAuth/runtime/continuity type. |
| Router separation | The Router is supplied only an already-sanitized exact quota snapshot. It continues to own availability/recovery and cannot infer Provider credentials, routes, Header values, or account state. |
| Diagnostics | Runtime-state Debug/status failures expose no endpoint ID, credential ID, Header value, model, account, API key, OAuth, Build cache identity, or Tool/Reasoning value. P8-03 metadata may retain only its documented structural field-presence/count projection. |

## Corresponding tests

- `official_header_quota_is_exact_and_cannot_mutate_build_state_or_affinity`
- `official_failures_only_classify_or_cool_their_own_exact_quota_target`
- `runtime_state_receives_only_the_explicit_official_transport_observation`
