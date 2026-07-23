# P9-07 Grok Web 403 egress/account attribution report

| Field | Value |
|---|---|
| Plan version | `v1.42` |
| Task | `P9-07` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001` |
| Scope / budget | `M`; value-free HTTP status/evidence classification and in-memory local availability state. No Web endpoint, error body, browser/profile, Cookie source, server, proxy/TUN configuration, or external request was used. |
| References | Matrix `C29-C31`、`D28`、`E10`、`E22`、`E27-E29`; [ADR-0067](../adr/ADR-0067-grok-web-403-egress-account-attribution.md); [BC-SEC-004](../contracts/BC-SEC-004-grok-web-403-egress-account-attribution.md) |

## Delivered behavior

`provider-grok` now makes generic Grok Web 403 responses egress-local by construction. Only `403 + ConfirmedForbidden`, a separately supplied bounded evidence category, can create `CredentialForbidden/Account`; malformed, out-of-range, or non-403 account evidence has no disposition or local state effect.

The egress state binds account, lineage, credential revision/expiry, and egress-session ID. An unknown 403 requires rebuilding only that exact session. The account state binds the credential lifecycle without the egress ID, so confirmed account evidence prevents the same credential revision from using any sibling egress session while leaving a replacement revision distinct. Neither state accepts the other action owner.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p9_07_web_failure_attribution` | PASS; three synthetic exact-egress, exact-account-lifecycle, invalid-evidence, and wrong-owner no-mutation tests passed. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --test p9_07_web_failure_attribution -- -D warnings` | PASS. |
| `./scripts/check.sh full` | PASS; local workspace Full gate, task-state guard, Rust format/Clippy/tests, supply-chain checks, docs checks, and Secret scan passed. |
| Focused review | PASS: 403 has a single owner, unknown/WAF-shaped failures cannot ban accounts, confirmed evidence cannot be misapplied to egress, and no parser/send path exists. |

## Deferred external proof

No fixture proves live WAF categories, Web error-body syntax, account restriction semantics, credential state, or egress behavior. P9-09/G9 remain deferred to a P9-specific test account and explicit Canary authorization; P8 Official E2E and P7 Kiro OAuth remain in the final external-authentication package.
