# P12-10H native Grok CPAR live E2E receipt

Status: `BLOCKED_EXTERNAL_RATE_LIMIT`

Date: 2026-08-04

## Boundary

This was a loopback-only staging exercise on the Oracle Singapore host. It used the signed ARM
artifact for revision `a8455ca77dcbdfa95b1c392b42a117ccb02ba7de`, a temporary Config Version and one
Build account imported through the root-only grok2api memory pipe. It did not touch the production
database, production listener, Caddy, DNS, CC Switch, old CPA, or public traffic. grok2api was
started only long enough to export the bounded staging record and was stopped afterward.

## Implementation and artifact gates

- The runtime shape gate now admits only the native Grok Build `allow_unlisted_model=true` plus
  `reasoning=false` override required by the reviewed `CanonicalBridge` route; other adapters retain
  the one-field override shape.
- Local `./scripts/check.sh fast` passed after the change.
- GitHub CI `30905365066` passed Fast, Full supply-chain, and Required delivery jobs.
- Signed dual-architecture release `30906451455` passed; the ARM artifact was independently
  verified before staging installation.

## Direct CPAR result

The value-free curl harness performed the authenticated `/v1/models` preflight successfully, then
sent at most 100 inference calls through the CPAR base URL and stopped at the first failure:

| Measure | Result |
|---|---:|
| Requested calls | 100 |
| Attempted calls | 27 |
| Successful calls | 26 |
| Responses calls completed | 9 |
| Chat calls completed | 9 |
| Messages calls completed | 9 |
| First failure | `http_4xx` |
| Protected Attempt attribution | `ProviderRateLimited/provider`, retry-eligible |
| Account state after failure | active; no persisted cooldown or quota window |

The receipt contains only bounded counts and fixed categories. No endpoint, model, credential,
request, response, token, or token fingerprint was retained.

The source pool could not safely provide a five-account Build batch for a second run: fewer than
five eligible active Build records were available. No expired, disabled, or otherwise ineligible
record was forced into the staging pool, and no repeated request was sent after the provider-level
rate limit.

## Rollback and invariants

- The temporary native graph was rolled back through the protected management lifecycle.
- The temporary imported account batch was rolled back; staging ended with zero native accounts.
- Staging listeners were closed; the isolated database reported `quick_check=ok` and zero foreign
  key violations.
- The Oracle Singapore production CPAR remained active with its existing loopback listeners.
- grok2api remained stopped after the bounded export; no source account was deleted or mutated.

## Verdict

Native route binding, artifact provenance, CPAR authentication, all three protocol projections and
26 successful live calls are evidenced. The 100-call live gate is not passed because the external
provider rate-limited the single eligible account and a safe multi-account retry was unavailable.
P12-10H and the final P12-10 retirement/Tag closeout remain open.
