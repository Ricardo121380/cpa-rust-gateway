# P12-10I-12 Console same-egress reference parity receipt

Status: `BLOCKED_EXTERNAL_EGRESS_WITH_REFERENCE_PARITY`

## Boundary

This diagnostic compares CPAR's current Console result with an exact frozen grok2api reference on
the same Oracle Singapore host and egress. It uses a root-only online copy of the existing source
database, a loopback-only temporary reference container, and one bounded public reference request.
It does not modify CPAR code, production databases, active configuration, listeners, routes,
Caddy/DNS, CC Switch, the old CPA, or public traffic.

No SSO, OAuth, cookie, API key, endpoint, model, request/response body, account identity, or token
fingerprint is recorded here.

## Reference and source census

| Check | Result |
|---|---|
| Frozen source | grok2api `v3.0.10`, revision `c27f0545197b3edf41d5deedcc2c3c3597887766` |
| Runtime image | Existing exact `v3.0.10` image on the Oracle host |
| Source database handling | SQLite online backup into a root-only temporary directory |
| Network exposure | Temporary container bound only to loopback |
| Console inventory | 898 enabled/active Console records in the source snapshot |
| Historical control | 25 successful Console audit records were present |
| Optional egress state | No stored clearance-cookie or configured egress-node records were present |

Source review found that grok2api can optionally attach clearance cookies and renew them after a
403, and emits two additional browser client-hint headers. Those mechanisms were not active in the
current source snapshot. Their presence in source is therefore a diagnostic lead, not evidence that
CPAR should copy them or that they would fix the current upstream response.

## Controlled comparison

The first readiness checks occurred before the temporary process had populated its route/account
caches. They returned an unavailable result and sent no generation request. After an eight-second
readiness interval, the authenticated model listing returned HTTP 200 with eight available entries.

For the only public generation request:

- one account associated with a recent historical successful Console audit was selected;
- all other Console accounts were disabled only in the copied database;
- copied cooldown/failure and quota-recovery/model-block state for that account was cleared;
- one non-streaming Responses request was sent through the reference public API;
- the public result was HTTP 503;
- the reference audit recorded provider `grok_console`, upstream HTTP 403, final category
  `upstream_unavailable`, and two internal upstream attempts;
- the final attempt reached `upstream_response` with HTTP 403 and no transport error.

The copied database remained `quick_check=ok`.

## Interpretation

CPAR's earlier 25-account eligible-pool sweep reached the Console upstream and classified every
account as `EgressRejected/egress`. The exact grok2api reference now reaches the same upstream from
the same host and receives HTTP 403, including with a historically successful account. Under the
current external state, the failure therefore reproduces outside CPAR and is not evidence of a
CPAR-specific request-construction defect.

This result does not prove that the providers are permanently equivalent, nor that optional
clearance handling or header differences can never matter. It proves only that changing CPAR based
on those source differences is not justified by the present same-egress evidence.

## Cleanup and production invariants

- The temporary container and root-only working directory were removed.
- No temporary reference container or matching root directory remained after cleanup.
- The source database was never opened for mutation; only its online backup was changed.
- CPAR production service, release pointer, active graph, database, listeners, and public routes
  were not changed.

## Verdict

P12-10I-12 is `BLOCKED_EXTERNAL_EGRESS_WITH_REFERENCE_PARITY`. Console remains externally blocked,
but the blocker now has exact same-version, same-host reference parity. No CPAR Console code change
is authorized or technically justified by this comparison.
