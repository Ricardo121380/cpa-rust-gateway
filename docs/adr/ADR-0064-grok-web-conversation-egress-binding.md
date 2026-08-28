# ADR-0064: Grok Web Conversation exact account and egress binding

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P9-04` |
| Matrix / Contract | `C29-C31`、`D28`、`E27-E29`; [BC-CONT-003](../contracts/BC-CONT-003-grok-web-conversation-egress-binding.md) |

## Context

Grok Web conversations are anti-bot-sensitive state, not a generic reusable message history. A continuation must not move from the original account, SSO lineage/revision/expiry, or browser egress session simply because the next request is otherwise valid. P9-03 intentionally supports only an initial synthetic Text request; P9-04 needs local state for future continuation without inferring live Web protocol or touching a browser.

## Decision

1. Create `GrokWebConversationState` from one opaque conversation ID and one non-expired P9-02 `GrokWebBrowserEgressSession` at caller-supplied time.
2. Persist in memory only opaque conversation/parent IDs and exact non-secret binding dimensions: account reference, SSO lineage, credential revision, absolute expiry, and egress-session ID. Cookie, User-Agent, proxy, TLS label, model, prompt, and response text are not copied.
3. Every `prepare_turn`, parent update, and account-unavailable transition rechecks all binding dimensions and expiry. A mismatch does not mutate the state; expiry, account/lineage/revision/expiry/egress mismatch each fail closed.
4. Parent progression is explicit. A later response owner records a distinct opaque parent ID only after exact-session validation; a continuation snapshot has either no parent or the last accepted parent.
5. P9-07 alone classifies account evidence. It may call the narrow `mark_account_unavailable` state transition after binding validation, permanently preventing local continuation; P9-04 does not inspect HTTP, WAF, quota, or credentials itself.

## Consequences

- A Cookie refresh, SSO lineage switch, credential revision/expiry change, or egress rotation needs new conversation state and cannot silently continue an old thread.
- P9-03 remains an initial request grammar. A later authorized request composer can consume only a validated `GrokWebConversationTurn` snapshot.
- `grok.build`, `grok.official`, and Kiro have no path into this local state.

## Validation and rollback

Synthetic tests cover initial/continuation parent progression, exact account/lineage/revision/expiry/egress rejection, expiry, account-unavailable state, duplicate parent refusal, and diagnostic redaction. Rollback removes the local state, test, ADR/contract/report/index entries only; it does not read or mutate a browser, SSO source, Web endpoint, proxy/TUN setting, server, or live account.
