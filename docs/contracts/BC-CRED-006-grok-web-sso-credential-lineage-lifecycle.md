# BC-CRED-006 Grok Web SSO credential lineage and revisioned lifecycle

| Field | Value |
|---|---|
| Contract | `BC-CRED-006` |
| Task | `P9-01` |
| ADR | [ADR-0061](../adr/ADR-0061-grok-web-sso-credential-lineage-lifecycle.md) |
| Matrix | `C29`、`C31`、`E27-E29` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001`; no browser, Cookie source, or Web request was read or used |
| Domain | Strict SSO Cookie import, AEAD-sealed storage-neutral envelope, non-secret lineage, isolated expiry, and revision-CAS lifecycle |

## Preconditions and bounds

1. The caller owns the bytes. This boundary opens no file, browser profile, Cookie jar, OAuth cache,
   database, server configuration, proxy, route, or network connection.
2. One input is at most 64 KiB and has exactly `kind`, `account_ref`, `lineage_ref`, `revision`,
   `expires_at_ms`, and `cookies`. Duplicate fields at any depth and unknown fields fail closed.
3. Account and lineage are short opaque ASCII references, not email/account values. A session must
   expire after observation time and within 90 days. It has at most 32 scoped Cookies.
4. Every Cookie has a safe name/value/domain/path and `secure=true`; its scope is unique on
   `(name, normalized-domain, path)`. Values remain zeroized and never enter `Debug`.
5. An explicitly injected `SecretStore` may seal an exact credential into an opaque envelope. The
   associated data contains the `grok.web` domain plus opaque account/lineage, revision, and
   expiry; opening with a wrong key, modified envelope, changed metadata, malformed payload, or
   expired recovered credential fails closed. This type does not choose or access storage.

## Required behavior

| Concern | Required behavior |
|---|---|
| Provider isolation | Provider ID is exactly `grok.web`; no Build OAuth, Official API key, Kiro credential, quota, failure, or continuity type is accepted or mutated. |
| Source lineage | Retain only `ImportedSso` and opaque `lineage_ref`. It describes provenance without retaining browser-path, account, email, Cookie, OAuth, or Build identity. |
| Expiry | A credential is unusable when `now_ms >= expires_at_ms`. Expired, non-positive, overflowed, or overlong imports fail before state creation. |
| Cookie safety | Reject control characters, Cookie delimiters, insecure Cookies, IP/non-host-like domains, unsafe paths, and ambiguous duplicate scopes. P9-02 owns actual Cookie-header serialization. |
| Encryption | Cookie plaintext is serialized only into zeroizing memory for injected `SecretStore` AEAD sealing. Envelope diagnostics redact ciphertext; the envelope has no persistence, browser, proxy, or network side effect. |
| Revision lifecycle | A replacement succeeds only if expected revision equals current, replacement revision is exactly `current + 1`, and account/lineage exactly match. Stale writers return `Conflict`; mismatches do not mutate state. |
| Diagnostics | Debug may show provider ID, opaque references, expiry, revision, Cookie count and Cookie scope metadata. It never shows Cookie values. |

## Corresponding tests

- `strict_sso_import_retains_only_validated_scopes_and_redacts_cookie_values`
- `sealed_sso_envelope_is_aead_protected_and_expiry_checked`
- `malformed_expired_or_ambiguous_sso_exports_fail_closed`
- `revision_cas_keeps_web_account_and_lineage_isolated`
