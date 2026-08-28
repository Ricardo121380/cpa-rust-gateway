# ADR-0062: Grok Web browser egress-session fingerprint binding

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P9-02` |
| Matrix / Contract | `C29`、`C31`、`D28`、`E27-E29`; [BC-PROVIDER-018](../contracts/BC-PROVIDER-018-grok-web-browser-egress-session.md) |

## Context

`grok.web` is a browser-originated surface: SSO/Cloudflare Cookies, a User-Agent, a TLS profile,
and a selected egress path jointly form one anti-bot-sensitive fingerprint. A future conversation
must not silently change one of these components, and a refresh must not mutate an in-flight
session from the old Cookie revision. The local P9 boundary still prohibits a browser read, proxy
change, TLS handshake, DNS lookup, or Web request.

## Decision

1. Construct an immutable `GrokWebBrowserEgressSession` only from a currently usable
   `GrokWebCredential`, opaque egress-session ID, explicit User-Agent, explicit TLS-profile
   label, one `Direct` or validated local-DNS SOCKS5 `UpstreamProxy`, and caller-supplied time.
2. Bind account reference, SSO lineage, revision, expiry, Cookie scopes, User-Agent, TLS profile,
   proxy, and egress-session ID for the session lifetime. A current credential with a different
   account, lineage, revision, or expiry is rejected; it cannot update the session in place.
3. Build a Cookie header only through `cookie_header_for_https`: validate a bounded host/path,
   select scoped secure Cookies deterministically by path specificity, retain the header in
   `Zeroizing<String>`, and reject empty/oversized scopes. Cookie/User-Agent/proxy values are
   redacted from diagnostics.
4. Reuse only the generic, already-validated `UpstreamProxy` value as an immutable choice. This
   module does not inspect environment/system proxy configuration or construct an HTTP/TLS client.

## Consequences

- P9-03 has one exact input for a fixed admitted Web target: HTTPS Cookie header, User-Agent, TLS
  profile label, and proxy choice are all explicit and cannot drift between request steps.
- P9-04 can bind a Web Conversation to the egress-session ID and the exact Web credential version.
- An expired or replaced SSO credential causes a safe mismatch/expiry result; it does not change
  account health or egress state. P9-07 owns 403 classification.

## Alternatives considered

- Reuse an `UpstreamTransportProfile` as the Web session: rejected because it lacks Cookie,
  User-Agent, TLS-profile, account/lineage, and revision binding.
- Discover the default browser User-Agent, TLS fingerprint, or proxy settings: rejected because
  ambient machine state creates non-repeatable authority and leaks browser/proxy configuration.
- Permit request code to concatenate all Cookies: rejected because it can send a Cookie to the
  wrong host/path, hide scope mistakes, and retain unbounded plaintext header storage.

## Validation and rollback

Synthetic tests cover exact fingerprint/Cookie binding, deterministic scoped headers, User-Agent
injection rejection, expiry, account/lineage/revision replacement rejection, and redacted SOCKS5
diagnostics. Formatting, focused Clippy, Secret scan, and the P9 local Full gate must pass.
Rollback removes the session module, tests, ADR/contract/report/index entries, and does not read
or change a browser, Cookie source, proxy/TUN configuration, server, account, DNS, TLS, or Web
traffic.
