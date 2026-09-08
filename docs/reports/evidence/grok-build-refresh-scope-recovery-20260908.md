# Grok Build OAuth scope recovery — 2026-09-08

## Incident and cause

SSH access to Oracle recovered. The September 2 Build import was `reauth_required`,
revision 0, with 14 refresh failures and no successful refresh. The service itself
was active; a different Build account continued refreshing successfully.

Replacing the failed import with a current official CLI credential restored catalog
discovery but immediately reproduced refresh failure. A bounded token refresh from
the same VPS returned HTTP 200, a refresh token and a 21,600-second lifetime. Its
scope differed from the original CPAR scope even when compared as a set. The current
access token's scope also includes `conversations:read`, `conversations:write`,
`workspaces:read`, and `workspaces:write`.

The OAuth parser required exact scope-string equality and rejected successful
responses carrying additional granted scopes. This explains the reproduced refresh
failure; historical logs do not retain enough detail to attribute all 14 failures
or the final reauthentication transition to a particular upstream error.

## Fix and validation

Scope validation now requires containment of all required scope tokens, independent
of order, and preserves the issuer's returned scope. Missing required scopes remain
rejected. Absolute-expiry imports accept the same baseline-preserving expansion.
No requested OAuth privileges, issuer, client identity, endpoint or protocol changed.

Regression coverage includes expanded/reordered scopes, durable round-trip, a second
refresh, and rejection of required-scope loss. Production deployment and refresh /
Responses acceptance will be recorded below after execution.

## Recovery boundary

The root-only existing rollback/import commands replaced only the failed Build
batch. A stopped-service database backup was retained at
`/var/backups/cpa-rust-gateway/build-recovery-20260908T1239Z`.
Plaintext credentials moved only through process memory and encrypted SSH; the
official local cache was atomically updated after its successful refresh. No
credentials, account identities, Client Keys, raw token responses or model output
are included in this receipt.
