# BC-PROVIDER-018 Grok Web browser egress-session fingerprint

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-018` |
| Task | `P9-02` |
| ADR | [ADR-0062](../adr/ADR-0062-grok-web-browser-egress-session-fingerprint.md) |
| Matrix | `C29`、`C31`、`D28`、`E27-E29` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001`; no browser, proxy configuration, DNS, TLS, or Web request was read or used |
| Domain | Immutable Web Cookie/User-Agent/TLS-profile/proxy fingerprint and exact credential-version binding |

## Preconditions and bounds

1. A caller supplies a validated `GrokWebCredential`, opaque egress-session ID (1–128 ASCII
   identifier bytes), explicit User-Agent (1–512 visible ASCII bytes), explicit TLS-profile label
   (1–128 ASCII identifier bytes), already-admitted `UpstreamProxy`, and non-negative observation
   time.
2. The input credential must be valid at construction; its account, lineage, revision, expiry,
   and scoped secure Cookies become immutable session components.
3. `cookie_header_for_https` accepts only a bounded DNS-like non-IP host and absolute path without
   query/fragment. The selected header is at most 64 KiB and uses zeroizing storage.

## Required behavior

| Concern | Required behavior |
|---|---|
| Provider isolation | Provider ID is exactly `grok.web`; the session accepts `GrokWebCredential` only and does not read/mutate Build, Official, or Kiro credential, quota, failure, or continuity state. |
| Fingerprint | Egress-session ID, credential account/lineage/revision/expiry, Cookies, User-Agent, TLS profile, and Proxy are fixed at construction. There is no ambient browser/proxy discovery or inferred TLS profile. |
| Credential replacement | `require_current_credential` rejects expired, account-mismatched, lineage-mismatched, or revision-mismatched credentials. A new revision requires a new browser egress session. |
| Cookie scope | HTTPS header selection applies only secure Cookies matching the requested host domain and RFC-style path boundary. Longer paths sort first; name/domain break ties deterministically. Empty scope and oversized headers fail closed. |
| Diagnostics | Cookie values, User-Agent, and SOCKS5 endpoint are redacted from `Debug` and error messages. Only opaque identity, profile label, counts, revision/expiry, and `UpstreamProxy` safe projection may appear. |
| I/O boundary | No filesystem/browser profile, environment proxy, server, DNS, socket, TLS handshake, HTTP request, or proxy/TUN mutation is allowed. P9-03 owns an injected fixed Web request boundary. |

## Corresponding tests

- `browser_egress_session_binds_exact_credential_fingerprint_and_cookie_scopes`
- `browser_egress_session_rejects_expiry_scope_and_unsafe_fingerprint_inputs`
- `browser_egress_session_cannot_follow_account_lineage_or_revision_replacement`
