# P9-06 Grok Web source-labelled quota report

| Field | Value |
|---|---|
| Plan version | `v1.42` |
| Task | `P9-06` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001` |
| Scope / budget | `M`; strict synthetic quota syntax and in-memory state only. No Web/REST/gRPC-Web endpoint, browser/profile, Cookie source, server, proxy/TUN configuration, or external request was used. |
| References | Matrix `C31`、`C33`、`C34`、`D28`、`E27-E29`; [ADR-0066](../adr/ADR-0066-grok-web-source-labelled-quota-observations.md); [BC-PROVIDER-020](../contracts/BC-PROVIDER-020-grok-web-source-labelled-quota.md) |

## Delivered behavior

`provider-grok` now decodes only the P9-06 synthetic REST and gRPC-Web quota fixture shapes. A retained quota window carries a bounded opaque tier, coarse window kind, remaining/total capacity, duration/reset/observation instants, source, and fixed `Observed` confidence. It cannot be constructed from an unknown or inconsistent shape.

`GrokWebQuotaState` binds its values to exact account, SSO lineage, credential revision/expiry, and browser egress-session identity. It stores REST and gRPC-Web observations independently by source/window kind, rejects stale-session writes and same-time conflicts without mutation, and does not treat any source as billing authority.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p9_06_web_quota` | PASS; three synthetic source/window, ordering/session, malformed/cross-shape, and redaction tests passed. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --test p9_06_web_quota -- -D warnings` | PASS. |
| `./scripts/check.sh full` | PASS; local workspace Full gate, task-state guard, Rust format/Clippy/tests, supply-chain checks, docs checks, and Secret scan passed. |
| Focused review | PASS: distinct source snapshots cannot silently merge, all mutating input is exact-session bound, and the module exposes no live quota send path. |

## Deferred external proof

Fixtures prove only the local grammar and state invariants. They do not prove current REST or gRPC-Web quota schemas, tier meaning, entitlement, reset semantics, WAF behavior, account state, or billing. P9-09/G9 remain deferred to a P9-specific test account and explicit Canary authorization; P8 Official E2E and P7 Kiro OAuth remain in the final external-authentication package.
