# Kimi quota diagnostics repair — 2026-09-27

## Result

Implemented locally. Production quota root cause remains unverified until the
new binary is explicitly approved for release and the bounded real metadata
read is repeated. No production writes or inference calls in this change.

The previous implementation converted both request results through `.ok()`,
losing transport/status/timeout evidence. It then mapped a successful but
unrecognized JSON quota payload to the same empty observation. This prevented
explaining the recorded `profile_available=true, quota_available=false`.

## Changes

- `provider-openai-compatible/src/kimi_metadata.rs`: closed safe failure enum,
  independently serialized profile/quota failures, `from_results` preserving
  partial success and distinguishing unrecognized payloads from missing reads.
- `apps/gateway/src/runtime/kimi_metadata.rs`: preserve egress, transport,
  timeout, HTTP status, oversized response and JSON failures. Existing 8-second,
  64 KiB, exact credential, generation and cache boundaries remain unchanged.
- Management HTTP distinguishes local busy and outer timeout. A local Busy is
  not reported as upstream HTTP 429.
- Authoritative OpenAPI updated and sync-contract run. Frontend quota detail
  renders the specific safe failure and does not fabricate 0% or a balance.
- No raw response body or credential is added to logs, DTOs, reports or tests.

## Verification

- PASS: four provider Kimi unit tests, including independent partial failures,
  403/429 status serialization, zero usage vs unknown, and secret exclusion.
- PASS: real Actix management HTTP test with synthetic facade: management auth,
  profile retained when quota fails HTTP 403, safe serialized error, no credential
  leakage and existing inventory cursor revision behavior.
- PASS: seven frontend quota evidence tests; type check; gateway cargo check.
- PASS: strict Clippy for affected provider/HTTP/gateway packages; frontend four-file deterministic build; 21-package boundary check and 107 contract test references.
- NOT_RUN: new production binary, real upstream quota result, real inference.
  Existing production receipt proving identity/four-model catalog remains in
  `evidence/cpar-account-deletion-20260927/`; it does not prove quota success.

Next release must include this patch and predecessor be5f30e (account deletion
entry/refresh). Keep all production accounts and history; exercise deletion
only with synthetic local accounts. After release, one management metadata
observation is enough to select the next fix; do not bypass egress or export
secrets merely to retrieve the upstream payload.
