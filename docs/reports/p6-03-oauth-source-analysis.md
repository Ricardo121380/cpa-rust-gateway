# P6-03 Grok Build OAuth source-shape analysis

| Field | Value |
|---|---|
| Task | `P6-03` / `CR-P6-03-008` |
| Method | Read-only, clean-room behavioral comparison of frozen CPA, grok2api, and `Sub2API` source snapshots |
| Scope | Known OAuth credential representations only; no account file, token, cookie, model mapping, request, or response value was copied or retained |

## Observed compatible behavior

| Source family | Compatible credential shape | P6 import decision |
|---|---|---|
| Standard OAuth / existing P6-01 | `access_token`, `refresh_token`, relative `expires_in` | Retain the existing strict importer, Device Code flow, and Refresh flow. |
| CPA xAI auth file | `type=xai`, access token, refresh token, absolute RFC3339 `expired`; a relative lifetime may remain as non-authoritative metadata | Add a dedicated in-memory CPA importer. Require the xAI type and derive the P6 absolute expiry only from `expired`. |
| grok2api and `Sub2API` account credential | access token, refresh token, absolute RFC3339 `expires_at`; public client/scope metadata may be present | Add a dedicated in-memory account importer. Require any supplied client or issuer metadata to match the fixed Build identity. |
| Official Grok CLI cache | one exact `issuer::public-client-id` indexed object, with `key` as the Bearer access token, `refresh_token`, and absolute RFC3339 `expires_at` | Add an indexed-cache importer. It accepts only the fixed issuer/client key and maps `key` only to the in-memory access token. |

All three references use the same public xAI OAuth client identity and refresh-grant concept. The
implementation therefore adapts storage shapes rather than inventing a new OAuth protocol or a
second credential lifecycle.

## Safety and compatibility choices

- Every importer accepts bytes supplied by its caller; production `provider-grok` never opens a
  credential path, cache, database, or environment variable.
- Absolute expiries parse strictly as RFC3339, must be in the future, and may be at most 366 days
  after the supplied observation instant. Competing absolute-expiry aliases fail closed.
- Duplicate JSON names at every depth, a wrong issuer/client, an absent indexed entry, a wrong
  token type, invalid fields, and oversized input fail closed without rendering a value.
- Unknown ancillary metadata is ignored only after strict JSON parsing; it cannot alter the selected
  access token, refresh token, issuer, client, scope, or authoritative expiry.
- The ignored P6-03 test harness keeps its existing synthetic JSON input for prior evidence and adds
  an explicit absolute local official-CLI-cache path alternative. It reads at most 64 KiB into
  zeroizing memory, forbids providing both sources, and never prints the path or cache content.
- `Sub2API` SSO/Cookie-to-Build conversion is deliberately excluded. It is a separate Web
  credential boundary rather than a standard OAuth credential-source adapter.

## Clean-room and license boundary

The CPA (`v7.2.80`, snapshot commit `09da52ad509e`) and grok2api (snapshot commit
`ec6cddca7d2454996540adbf994f3c3d4ed2d2a1`) references were MIT-licensed. The `Sub2API`
snapshot (commit `57914967cbb127ff715719c3879d881c10d75274`) was LGPL-licensed. This repository
uses only the documented/interoperable field behavior above; it copied no source text, data model,
or implementation from any reference.

## Validation evidence

Synthetic coverage in `p6_01_build_oauth` exercises standard JSON, CPA, account, and indexed-cache
sources; RFC3339 milliseconds; expired/overlong/conflicting expiry; wrong client/issuer; duplicate
cache entries; absent indexed entries; unknown metadata; and `Debug` redaction. The ignored harness
adds a synthetic file-only cache input test for mutual exclusion, absolute-path admission, bounded
read, and non-retention in diagnostics. One separately authorized exact local-cache preflight also
confirmed the user-authenticated official CLI representation imports and builds without DNS, HTTP,
refresh, or a Provider request. It retained no path or credential value. No real OAuth input
participates in the synthetic tests.
