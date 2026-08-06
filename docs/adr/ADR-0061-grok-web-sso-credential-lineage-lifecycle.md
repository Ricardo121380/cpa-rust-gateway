# ADR-0061: Grok Web SSO credential lineage and revisioned lifecycle

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P9-01` |
| Matrix / Contract | `C29`、`C31`、`E27-E29`; [BC-CRED-006](../contracts/BC-CRED-006-grok-web-sso-credential-lineage-lifecycle.md) |

## Context

`grok.web` uses browser-originated SSO session material rather than Grok Build OAuth or an Official
API key. Its Cookie scopes, account, egress, quota, failure, and future conversation state must
stay isolated. P9-01 needs a deterministic local way to accept a user-supplied SSO export, retain
the minimum request-facing material, document non-secret provenance, and prevent a stale session
replacement from overwriting a newer one. It must not search a browser profile or call Web APIs.

## Decision

1. Accept only a strict bounded JSON shape with opaque non-PII account and lineage references,
   non-negative revision, bounded absolute expiry, and one or more explicitly scoped Cookies.
2. Reject duplicate JSON names, unknown fields, expired sessions, unsafe Cookie values,
   IP/private-like Cookie domains, insecure Cookies, and duplicate `(name, domain, path)` scopes.
   The strict direct importer also rejects an expiry beyond 90 days. A governed migration adapter
   may accept a valid longer source expiry only by persisting
   `min(source_expiry, observed_at + 90 days)` and reporting a value-free capped-record count.
3. Keep Cookie values in `Zeroizing<String>` and redact them from every debug representation.
   Public access to a value is limited to an immediate later Web-session request constructor. An
   injected `SecretStore` can produce a storage-neutral AEAD envelope; it binds the exact Web
   account/lineage/revision/expiry in associated data and performs no storage I/O itself.
4. Retain source lineage as `ImportedSso` plus an opaque reference only. No Build/Official/Kiro
   credential, account value, browser profile path, email, OAuth value, or Cookie value is copied.
5. Use a provider-private mutexed slot with expected-revision replacement. Replacement must keep
   the exact account and lineage and increment revision by one; stale writers conflict without
   mutation.

## Consequences

- P9-02 can construct one BrowserEgressSession from a validated immutable Web credential version.
- A dedicated later storage boundary can retain the opaque AEAD envelope without reimplementing
  Cookie serialization or accepting a cross-account/cross-lineage substitution.
- A Cookie/session refresh is explicit and revision guarded; a stale 403/refresh path cannot
  replace a newer session or silently cross account lineage.
- A long-lived browser Cookie cannot widen CPAR authority: migration stores at most 90 days, while
  direct callers retain the original overlong-session rejection.
- The current slot deliberately has no persistence, browser/profile discovery, egress, or HTTP;
  later P9 tasks own those external boundaries and any durable runtime decision.

## Alternatives considered

- Reusing `GrokBuildCredential`: rejected because OAuth tokens and Cookie sessions have different
  lifecycle, source, and isolation requirements.
- Reading the default browser Cookie jar: rejected because it creates implicit local authority,
  profile leakage, and non-repeatable tests.
- Treating any 403 as a session replacement trigger: rejected because P9-07 must distinguish WAF
  egress rejection from account evidence first.

## Validation and rollback

Synthetic imports cover valid redacted scoped Cookies, AEAD envelope ciphertext/redaction,
wrong-key and expiry rejection, duplicate root fields, duplicate Cookie scopes, unsafe Cookie
values, expired sessions, and stale/cross-lineage CAS. Formatting, focused Clippy, Secret scan,
and the P9 local Full gate must pass. Rollback removes the isolated module, test, ADR, contract,
report, and index entries only; it neither reads nor changes a Cookie, SSO account, browser
profile, Web endpoint, proxy/TUN configuration, server, or production traffic.
