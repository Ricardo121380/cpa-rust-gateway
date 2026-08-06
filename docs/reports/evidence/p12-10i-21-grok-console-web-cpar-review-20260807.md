# P12-10I-21 Grok Console/Web CPAR HTTP review

## Review verdict

`BLOCKED_WITH_EVIDENCE` is the correct outcome. The test exercised the requested real CPAR HTTP
boundary and kept provider, route, protocol, and credential failure ownership separate.

## Findings

1. **Console route admission — PASS.** The isolated Config Version was created from the current
   production parent, published only after a restart, and its route explanation selected exactly
   one candidate. `/v1/models` passed before each inference attempt.
2. **Console credential attribution — PASS.** Five non-adjacent SSO records were imported in
   separate one-account batches. Every failure was the same value-free
   `CredentialUnauthorized/credential` attempt outcome. No cross-account fallback or protocol
   retry occurred, so the evidence does not blame the CPAR translator or egress policy.
3. **Web lifetime policy — PASS.** The source inspection found no explicit expiry in the 898-account
   Web pool and no expiry claim in the SSO token payloads. Refusing import is required by the
   bounded 90-day credential contract; inventing a deadline would be an unsafe false positive.
4. **Security and cleanup — PASS.** Credentials traversed only the remote root pipe and the
   encrypted staging importer. Receipts contain counts and fixed categories only. The staging
   account batch, route, process, and temporary files were rolled back or moved to recoverable
   trash; production health, active version, and account count were unchanged.

## Remaining work

Do not mark Console or Web as accepted. Resume only with a current, explicitly authorized Console
SSO or Web session that carries a verifiable expiry. Keep the next run on the same CPAR base URL +
client-key harness, with one account at a time and first-failure stop. No GitHub CI run is required
for this documentation-only closeout under `CR-EXEC-008`; the local code gates for the prior Web
recovery implementation remain the authoritative implementation evidence.
