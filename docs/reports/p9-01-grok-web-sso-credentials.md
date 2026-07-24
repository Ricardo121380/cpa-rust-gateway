# P9-01 Grok Web SSO credential, lineage, and lifecycle report

| Field | Value |
|---|---|
| Plan version | `v1.42` |
| Task | `P9-01` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001` |
| Scope / budget | `M`; in-memory SSO Cookie credential/lifecycle plus storage-neutral AEAD envelope. No browser/profile, Cookie source, account, server, proxy/TUN, Web endpoint, or production configuration was read or changed. |
| References | Matrix `C29`、`C31`、`E27-E29`; [ADR-0061](../adr/ADR-0061-grok-web-sso-credential-lineage-lifecycle.md); [BC-CRED-006](../contracts/BC-CRED-006-grok-web-sso-credential-lineage-lifecycle.md) |

## Delivered behavior

`provider-grok` now owns an isolated `grok.web` SSO credential type. It accepts a narrow strict
JSON export with opaque account/lineage references, absolute expiry, monotonic revision, and
bounded secure Cookie scopes. Duplicate JSON names, unknown fields, invalid/expired lifetimes,
unsafe Cookie text, IP/non-host-like domains, insecure Cookies, and duplicate Cookie scopes stop
before lifecycle state is created.

Cookie values are zeroized and redacted from both Cookie and credential debug forms. The only
value accessor is documented for immediate P9-02 request assembly; P9-01 itself performs no
header serialization, browser/profile read, egress, HTTP request, refresh, or persistence.

An injected `SecretStore` can seal the exact credential into a redacted AEAD envelope without
writing it anywhere. Its associated data binds `grok.web`, opaque account/lineage, revision, and
expiry. A wrong key or an expired recovered session fails closed. This is a storage-neutral
cryptographic primitive, not a browser Cookie importer or persistence mechanism.

The in-process Web slot performs exact expected-revision replacement. It rejects a stale writer
without mutation and rejects any replacement whose account/lineage differs or whose revision does
not advance exactly one step. Build, Official, Web, and Kiro states remain separate.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p9_01_web_credentials` | PASS; four synthetic import/redaction, AEAD/expiry, rejection, and CAS-isolation tests passed. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --test p9_01_web_credentials -- -D warnings` | PASS. |
| Focused review | PASS: strict duplicate handling is reused, no ambient source/network access exists, Cookie values are zeroized/redacted, and CAS cannot cross account or lineage. |

## Deferred external proof

No SSO account, Cookie, browser profile, or Web request has been used. P9-02 owns explicit
BrowserEgressSession construction; P9-09/G9 remain deferred until P9 has its own test account and
Canary authorization. This local task does not convert or share SSO material with Grok Build.
