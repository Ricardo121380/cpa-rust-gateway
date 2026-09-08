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

Regression coverage includes expanded/reordered scopes, absolute-expiry reimport, a
second refresh, and rejection of required-scope loss.

The complete `provider-grok` package passed 201 tests. The follow-up gateway
administration checks passed both `grok_admin::tests` tests.

## Production refresh and Responses acceptance

- Scope-fix revision: `8a279fc48e4485a6e2b19ed84a86edbb563e441c`.
- Signed release: https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34227749027
  (both Linux architectures passed).
- ARM64 SHA-256: `8634831e8fb828702a406369ad7cb499bb1ffeb57aa5602e4f4517b7d4ef5836`.
- Independent local Cosign verification and repository artifact verification passed;
  the uploaded and installed binary hashes matched.
- Final pre-cutover backup: `/var/backups/cpa-rust-gateway/scope-fix-20260908T125208Z`.
- Target account refreshed at startup to revision 1, then through the running worker
  to revision 2. Both succeeded with failure count 0 and a future refresh deadline.
- SQLite quick check passed, with zero foreign-key violations.
- Authenticated public `/v1/models` returned HTTP 200 and both Build model IDs.
- One non-streaming public Responses request for each of `grok-4.6` and `grok-4.5`
  returned HTTP 200, `completed`, and non-empty output, using the existing Pi Client Key.

The entitlement-sync command exposed a second defect: it and the root-only Build
probe only decoded source JSON, whereas successful refresh stores authenticated
compact credential bytes. Both call sites now use the existing active-runtime
decoder, which supports both forms and still rejects expired access tokens. This
does not change any management HTTP shape.

## Final deployment and acceptance

- Final runtime revision: `928e971eb7d6f520b50ded677803e259d4549cb1`.
- Signed release: https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34228892449
  (both Linux architectures passed).
- ARM64 SHA-256: `7ec82552fbf0d24cbc9ae9871d65543f8881a97dda7f68d5ceaf10e9975b5342`.
- Independent signature and complete artifact verification passed; target-host
  uploaded and installed hashes matched before switching the release pointer.
- Final backup: `/var/backups/cpa-rust-gateway/scope-fix-20260908T130401Z`.
- The native entitlement synchronizer successfully read compact refreshed state and
  recorded `domain=grok_build`, `tier=supergrok`, `source=provider_subscription`,
  `confidence=authoritative`.
- Startup performed a third successful refresh; target revision is 3, status is
  active, failure count is 0, and the next automatic refresh is scheduled.
- Protected runtime status is `active / available`, with the authoritative tier and
  a future access-token expiry. Database integrity and foreign keys passed again.
- Public model listing and one Responses request per Build model passed again on
  the final binary. Total bounded inference checks in this repair: four requests,
  two per model across the two deployed revisions, all HTTP 200 / completed.
- CPAR, Caddy and the independent Autoreg unit are active. Only CPAR was restarted.

**Verdict: PASS for this Grok Build refresh and runtime recovery.** No frontend,
OpenAPI, Client Key, Pi provider configuration, other channel credential, DNS or
proxy settings changed. Other missing/expired catalog targets and overall P13-15
completion remain outside this repair. Pi's existing local model selection still
contains only `grok-4.5`; the server's authenticated catalog contains both Build IDs.
Long-term unattended operation beyond the tested refresh cycles is not claimed.

## Recovery boundary

The root-only existing rollback/import commands replaced only the failed Build
batch. A stopped-service database backup was retained at
`/var/backups/cpa-rust-gateway/build-recovery-20260908T1239Z`.
Plaintext credentials moved only through process memory and encrypted SSH; the
official local cache was atomically updated after its successful refresh. No
credentials, account identities, Client Keys, raw token responses or model output
are included in this receipt.
