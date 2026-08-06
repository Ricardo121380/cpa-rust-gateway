# P12-10I-17 Grok three-channel CPAR HTTP E2E receipt

Status: `BLOCKED_WITH_EVIDENCE`

## Boundary

This was a fresh Oracle Singapore loopback staging run using the signed ARM64 artifact for
revision `d0b00b25f4cb67c6fc4dc3148ccc582e51b7054b` from release run `31088168853`. Each provider
route used its own temporary CPAR client key and the existing bounded `curl` harness. No production
database, public listener, Caddy/DNS, CC Switch, old CPA, or public traffic was changed.

## Value-free results

| Channel | Model preflight | CPAR requests | Result | Stop reason |
|---|---:|---:|---|---|
| Build | PASS (`200`) | 1 | BLOCKED | first Responses request returned `http_5xx` |
| Console | PASS (`200`) | 1 | BLOCKED | first Responses request returned `http_5xx` |
| Web | not reached | 0 | BLOCKED | source exporter rejected the only eligible candidate as `web_expiry_unavailable` |

The harness stopped on the first failure for Build and Console, so Chat/Messages and SSE were not
sent after the first failed Responses request. The Web path was stopped before account import and
before any CPAR or upstream request. No request/response body, endpoint value, model secret,
credential, cookie, token, or token fingerprint was retained.

## Artifact and rollback

- Local artifact verification passed for the exact revision, ARM64 target, signed manifest and receipt.
- The Web route binder was run only on the isolated management listener and produced no production
  configuration change.
- The temporary Console batch was rolled back: one imported account removed; staging database
  `quick_check=ok`.
- The staging gateway was stopped and loopback listeners were cleared.
- The production service remained `active`, its active configuration remained the existing
  production version, and no production Grok route was published.

## Verdict

Native route binding and the real CPAR HTTP boundary are exercised for Build and Console, but neither
channel passed the text inference check because the first upstream attempt returned `http_5xx`.
Web remains blocked by the source session lifetime policy and needs a newly authorized valid Web
session before a real CPAR request is permitted. This receipt does not claim any of the three
channels is production-ready.
