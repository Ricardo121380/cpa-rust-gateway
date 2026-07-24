# BC-CONT-003 Grok Web Conversation account and egress binding

| Field | Value |
|---|---|
| Contract | `BC-CONT-003` |
| Task | `P9-04` |
| ADR | [ADR-0064](../adr/ADR-0064-grok-web-conversation-egress-binding.md) |
| Matrix | `C29-C31`、`D28`、`E27-E29` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001`; local-only and zero-network |
| Domain | Opaque Conversation/Parent lifecycle with exact Web account/credential/egress binding |

## Preconditions and bounds

1. A conversation ID and a parent ID are each non-empty visible ASCII and at most 512 bytes.
2. Creation takes an unexpired P9-02 browser egress session and a non-negative caller time.
3. A state object retains one exact account reference, SSO lineage, credential revision, credential expiry, and egress-session ID for its entire lifetime.

## Required behavior

| Concern | Required behavior |
|---|---|
| Initial turn | `prepare_turn` returns the caller-owned Conversation ID and no parent after revalidating the bound egress session. |
| Continuation | `record_parent_message` accepts a distinct opaque parent only after exact binding validation. A later `prepare_turn` returns the latest parent. A duplicate retained parent fails without mutation. |
| Exact binding | Account, lineage, credential revision, credential expiry, egress-session ID, and current expiry all match on every stateful operation. Any mismatch or expiry is a safe error and does not mutate parent or availability. |
| Account evidence | `mark_account_unavailable` needs an exact current session. It only blocks this local Conversation after a later owner has classified account-level evidence; it does not inspect HTTP/403/Quota/WAF. |
| Isolation | Cookie, User-Agent, proxy, TLS profile, model, request, and response text are absent. Build, Official, and Kiro state cannot be supplied. |
| Diagnostics | Conversation/parent/account/lineage/egress values are redacted from `Debug` and errors. |

## Corresponding tests

- `conversation_turns_keep_one_exact_account_lineage_revision_and_egress_binding`
- `parent_update_expiry_and_account_unavailability_fail_closed_without_cross_session_mutation`
- `conversation_identifiers_and_debug_forms_are_bounded_and_redacted`
