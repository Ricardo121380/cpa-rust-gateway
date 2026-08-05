# P12-10H Grok Console/Build eligible-pool sweep receipt

Status: `CONSOLE_POOL_EXTERNAL_EGRESS_BLOCKED_BUILD_POOL_PASS`

Date: 2026-08-05

## Boundary

This was an isolated loopback-only staging run on the Oracle Singapore host using the signed
revision `82ceb1d7c6899f3c615d7ed1cff2e400d5e1118a`. The grok2api source was read through its
supported export API and was never modified. No source credential, token, endpoint, model, request,
response body, or identity value is retained here.

The source census found `898` Console records and `829` Build records. The frozen migration
contract admitted `25` Console records with an active, non-cooldown state and a source-observed
successful model, plus `1` Build record with an active, unexpired credential and the reviewed model
shape. Remaining source records were not guessed or force-enabled.

## Console pool

| Measure | Result |
|---|---:|
| Imported Console accounts | `25` |
| Import rejected / created | `0 / 25` |
| Route explanation | one candidate selected |
| Public `/v1/models` preflight | `200` / JSON |
| CPAR Responses JSON calls | `25` attempted / `0` successful |
| Distinct credential IDs observed in attempts | `25` |
| Failure category | `EgressRejected / egress` for `25/25` |
| Retry/fallback | none |

The first activation check produced local `RouteNotFound` responses with zero upstream attempts
because the transient process needed one restart after publishing the route. After that restart,
the final sweep reached the upstream boundary for all 25 distinct credentials. This activation
detail is not counted as an upstream failure.

## Build pool

| Measure | Result |
|---|---:|
| Imported Build accounts | `1` |
| Import rejected / created | `0 / 1` |
| Route explanation | one candidate selected |
| Public `/v1/models` preflight | `200` / JSON |
| CPAR Responses JSON calls | `1` attempted / `1` successful |
| Upstream request | `sent_via_cpar` |

## Rollback and invariants

- Console rollback removed `25` accounts; Build rollback removed `1` account.
- Both staging databases were quarantined as route evidence and replaced with clean empty databases;
  both reported `quick_check=ok`, foreign-key violations `0`, and zero native accounts.
- No staging unit remained active. The temporary grok2api container was stopped and removed after
  the read-only export.
- Production CPAR remained active; its release pointer and database fingerprint were unchanged.
  No production graph, listener, Caddy/DNS configuration, CC Switch setting, or public traffic was
  changed.

## Verdict

The CPAR account-pool import, weighted rotation, public authentication, route selection, and
rollback path worked. The eligible Console pool is currently blocked by a uniform upstream
403/Egress condition, while the only currently eligible Build account passed. This does not make
the entire Grok pool production-ready: the other source records remain ineligible under the frozen
fail-closed import contract and require valid source state/model evidence before testing.
