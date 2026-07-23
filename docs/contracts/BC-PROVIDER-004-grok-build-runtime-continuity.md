# BC-PROVIDER-004 Grok Build runtime state and continuity boundary

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-004` |
| Task | `P6-04` to `P6-07` |
| ADR | [ADR-0045](../adr/ADR-0045-grok-build-runtime-continuity.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Domain | Build capability/billing/quota snapshots, cache identity/affinity, response continuity, replay, and bounded failure actions |

## Required behavior

| Concern | Required behavior |
|---|---|
| Catalog and Billing | A snapshot is exact-Credential, non-empty, duplicate-free, time-stamped, and atomically replaces only an older snapshot. A later Credential's plan/models cannot be read through another Credential. |
| Quota | Free, pay-as-you-go, subscription-monthly, and Web-weekly windows remain distinct. Billing is authoritative; Header/Web are observed; local arithmetic is estimated. Equal or stale observations do not overwrite newer state. |
| Cache identity | The client key, selected upstream model, and client cache key are HMAC-derived under a caller-held tenant secret with a versioned domain. The raw Canonical cache key must not appear in a Build body, debug output, or durable affinity key. |
| Cache affinity | The durable key is client key + `grok.build` + model + derived identity. Credential or egress replacement requires an atomic break record containing prior/next Credential, reason, time, and estimated cache loss. |
| Ownership | A `(client key, downstream response id)` maps to exactly one Build Credential and upstream response id until expiry. Missing, expired, and mismatched ownership are explicit errors; no replacement Credential is selected. |
| Replay | Reasoning/Tool replay is AEAD-sealed under exact client/model/session associated data. It is size-bounded, redacted in `Debug`, deduplicated only in the same namespace, and clears only that namespace. |
| Failure action | Invalid grant/token or `401` requires reauthorization. `403` without independent account evidence is `EgressRejected` with no Credential mutation. Free quota, account `429`, provider `429`, and transient `408`/`5xx` have distinct non-permanent actions. |

## Bounds and exclusions

- Durable identifiers are bounded and fail closed before SQLite. No state API renders a Token, raw cache key, upstream response id, replay plaintext, database path, or tenant secret.
- This contract accepts supplied observations; it does not issue Billing/model discovery HTTP, schedule an account, mutate a live server, or claim a direct Build request succeeded.
- Existing P6-02 Credential revision/CAS controls remain the only token-refresh persistence path.

## Corresponding tests

- `account_catalog_state_is_credential_scoped_and_monotonic`
- `quota_state_is_credential_scoped_source_labelled_and_monotonic`
- `cache_affinity_is_tenant_scoped_and_rebinding_has_durable_break_evidence`
- `response_ownership_and_reasoning_replay_are_exact_encrypted_and_clearable`
- `failure_matrix_never_turns_egress_or_transient_faults_into_permanent_credential_state`
- `build_request_uses_the_current_cli_profile_and_exact_admitted_target`
