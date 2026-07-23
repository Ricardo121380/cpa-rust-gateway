# P9-03 Grok Web fixture Chat and stream report

| Field | Value |
|---|---|
| Plan version | `v1.42` |
| Task | `P9-03` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001` |
| Scope / budget | `M`; local request blueprint and synthetic SSE parser. No SSO source, browser/profile, server, proxy/TUN configuration, live Web endpoint, or Web request was read or used. |
| References | Matrix `C29-C31`、`D28`、`E27-E29`; [ADR-0063](../adr/ADR-0063-grok-web-fixture-chat-stream-boundary.md); [BC-PROVIDER-019](../contracts/BC-PROVIDER-019-grok-web-fixture-chat-stream.md) |

## Delivered behavior

`provider-grok` now creates a deliberately non-sendable, non-routable fixture Chat blueprint from the immutable P9-02 browser egress session. The only accepted Canonical input is a single extension-free user Text message; conversation/parent state, Tool emulation, Thinking, cache, history, opaque content, and arbitrary Web endpoints remain outside this task.

The request blueprint receives its scoped HTTPS Cookie and explicit User-Agent from P9-02 in zeroizing storage and redacts them, the fixture target, model, and body from diagnostics. It has no network, browser, endpoint-admission, or transport operation.

The incremental strict SSE decoder maps a documented synthetic Web grammar through `CanonicalEventState`. It enforces exact response IDs and assistant-message lifecycle, rejects malformed/duplicate/unknown/oversized data, requires `done/[DONE]`, rejects after-terminal data, and produces the same Canonical sequence across arbitrary valid chunk cuts.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p9_03_web_chat` | PASS; four synthetic request/redaction, rejection, chunk-invariance, and malformed/EOF/terminal-safety tests passed. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --test p9_03_web_chat -- -D warnings` | PASS. |
| `./scripts/check.sh full` | PASS; local workspace Full gate, plan-state guard, Rust format/Clippy/tests, supply-chain checks, docs checks, and Secret scan passed. |
| Focused review | PASS: no live target or send API exists; request semantics are intentionally narrower than P9-04/P9-08; strict SSE state is transactional and passes through Canonical lifecycle validation. |

## Deferred external proof

This is only a local grammar contract, not proof of the current Grok Web request shape, Cookies, anti-bot policy, WAF, quotas, or remote stream semantics. P9-09/G9 remain deferred until a P9 Web test account and explicit Canary authorization are available. P8-07/G8 Official API E2E and P7-09 Kiro OAuth remain together in the final external-authentication package.
