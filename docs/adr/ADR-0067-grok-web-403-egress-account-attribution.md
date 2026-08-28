# ADR-0067: Grok Web 403 egress/account attribution

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P9-07` |
| Matrix / Contract | `C29-C31`、`D28`、`E10`、`E22`、`E27-E29`; [BC-SEC-004](../contracts/BC-SEC-004-grok-web-403-egress-account-attribution.md) |

## Context

A Grok Web 403 can represent WAF, network-egress, policy, session, or actual account rejection. Treating every 403 as an account ban loses usable accounts; treating actual account evidence as a transient egress condition can repeatedly use a forbidden credential lifecycle. Fixture-only P9 work must not inspect a real error body or guess account evidence.

## Decision

1. Classify a 403 with no independently established account evidence as `EgressRejected/Egress` with the sole action `RebuildEgressSession`.
2. Admit `ConfirmedForbidden` only when attached to a 403. It produces `CredentialForbidden/Account` and the sole action `MarkExactAccountForbidden`; non-403 account evidence fails closed.
3. Bind egress rejection state to account, SSO lineage, credential revision/expiry, and egress-session ID. It cannot reject a sibling egress session or account lifecycle.
4. Bind account-forbidden state to account, SSO lineage, credential revision/expiry but intentionally not egress-session ID. It blocks that exact credential lifecycle across sibling egress sessions, while a new credential revision is a distinct lifecycle.
5. Keep 401, 429, transient, and permanent status classification value-free. No classifier path parses raw Web response material or changes Build/Official state.

## Consequences

- A generic 403 cannot turn into a persistent account ban; its local remediation is isolated to one egress session.
- A confirmed account prohibition cannot be accidentally cleared by rotating only egress, but it cannot cross revision or lineage.
- P9-09 may wire a live evidence extractor only under an explicit Canary authorization. It must construct the bounded evidence value before this classifier and cannot add body parsing here.

## Alternatives considered

- Mark every 403 as account forbidden: rejected because WAF and egress rejection are common, distinct owner categories.
- Always rotate egress: rejected because proven account prohibition would cause repeated, non-useful session churn.
- Parse 403 body strings in this module: rejected because protocol bodies drift, could retain sensitive data, and require separate live evidence validation.

## Validation and rollback

Synthetic tests prove unknown-403 exact-egress isolation, confirmed account evidence across sibling egress sessions but not credential revision, invalid evidence rejection, wrong-owner no-mutation, and safe classifications. Rollback removes this local module/test/documentation only; it sends no Web request and changes no browser, proxy/TUN, server, or production state.
