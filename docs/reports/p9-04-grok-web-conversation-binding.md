# P9-04 Grok Web Conversation binding report

| Field | Value |
|---|---|
| Plan version | `v1.42` |
| Task | `P9-04` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001` |
| Scope / budget | `M`; in-memory Conversation/Parent state. No browser/profile, SSO/Cookie source, server, proxy/TUN configuration, live Web endpoint, or Web request was read or used. |
| References | Matrix `C29-C31`、`D28`、`E27-E29`; [ADR-0064](../adr/ADR-0064-grok-web-conversation-egress-binding.md); [BC-CONT-003](../contracts/BC-CONT-003-grok-web-conversation-egress-binding.md) |

## Delivered behavior

`provider-grok` now owns local `GrokWebConversationState`. It binds an opaque Conversation/Parent chain to exactly one P9-02 account, SSO lineage, credential revision, credential expiry, and browser egress-session ID. Initial and continuation snapshots are explicit. Parent updates reject duplicates and never mutate after an expired or mismatched session.

An account-unavailable projection exists only as a state transition after exact binding validation. It does not classify a 403, quota, WAF, or Web response; P9-07 owns that evidence. Diagnostics redact conversation, parent, account, lineage, and egress values.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p9_04_web_conversation` | PASS; three synthetic binding, progression, expiry, unavailable-account, and redaction tests passed. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --test p9_04_web_conversation -- -D warnings` | PASS. |
| `./scripts/check.sh full` | PASS; local workspace Full gate, plan-state guard, Rust format/Clippy/tests, supply-chain checks, docs checks, and Secret scan passed. |
| Focused review | PASS: all session binding dimensions are rechecked, including expiry; state mutations happen only after validation; no request or external-state capability was added. |

## Deferred external proof

No local fixture proves real Web Conversation or Parent-ID protocol behavior. P9-09/G9 remain deferred to a P9-specific test account and explicit Canary authorization. P8 Official E2E and P7 Kiro OAuth remain in the final external-authentication package.
