# P9-02 Grok Web browser egress-session report

| Field | Value |
|---|---|
| Plan version | `v1.42` |
| Task | `P9-02` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001` |
| Scope / budget | `M`; deterministic in-memory browser egress-session blueprint only. No browser/profile, Cookie source, proxy/TUN configuration, DNS, TLS, server, Web endpoint, or production traffic was read or changed. |
| References | Matrix `C29`、`C31`、`D28`、`E27-E29`; [ADR-0062](../adr/ADR-0062-grok-web-browser-egress-session-fingerprint.md); [BC-PROVIDER-018](../contracts/BC-PROVIDER-018-grok-web-browser-egress-session.md) |

## Delivered behavior

`provider-grok` now constructs an immutable `GrokWebBrowserEgressSession` from a current isolated
Web credential, an opaque egress-session ID, an explicit User-Agent, an explicit TLS profile, and
an explicit Direct or local-DNS SOCKS5 proxy choice. It carries account/lineage/revision/expiry and
Cookie scope state without any Build, Official, Kiro, browser, environment, or network input.

The session makes replacement explicit: a credential from another account/lineage or a later
revision is rejected. It cannot be silently substituted into an existing egress session. HTTPS
Cookie header construction is scope-bound to a validated host/path, orders longer Cookie paths
first, uses zeroizing storage, and fails closed for no match or a finite-size excess.

User-Agent, Cookie values, and proxy endpoints are redacted from diagnostics. The TLS profile is
an explicit label only; this task does not claim to create a browser-like TLS handshake. P9-03 will
own the fixed Web request/response boundary, and P9-07 will own WAF-versus-account 403 handling.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p9_02_web_egress_session` | PASS; three synthetic fingerprint/scope, rejection, and replacement-isolation tests passed. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --test p9_02_web_egress_session -- -D warnings` | PASS. |
| Focused review | PASS: no ambient browser/proxy/network source exists; mutable replacement is denied; HTTPS Cookie scope/header is bounded and redacted; Web state remains independent of Build/Official/Kiro. |

## Deferred external proof

No Web session, Cookie, TLS fingerprint, egress node, anti-bot response, or server/provider call
was used. This is local implementation evidence, not proof that the selected profile works against
grok.com. P9-09/G9 remain deferred until a P9-owned test account and explicit Canary approval.
