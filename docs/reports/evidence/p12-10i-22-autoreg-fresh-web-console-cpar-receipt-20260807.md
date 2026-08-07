# P12-10I-22 Autoreg fresh Web/Console CPAR HTTP receipt

## Scope

This receipt records one bounded re-registration and public CPAR HTTP run. Autoreg task 59
produced one fresh registration, which was passed through a root-only controlled pipe into an
isolated Oracle Singapore ARM64 staging graph. The test used the CPAR Base URL and a staging
client key, with `/v1/models` preflight and a stop-on-first-failure inference matrix. Production
CPAR, its active Config Version, Caddy/DNS, CC Switch, incumbent CPA, and public traffic were not
changed.

No credential, cookie, endpoint secret, account identity, model value, request body, or response
body is recorded here.

## Console

| Check | Result |
|---|---|
| Source | Fresh Autoreg task 59; one registration completed successfully |
| Import | One Console credential batch accepted `1/1` |
| Route lifecycle | Isolated Console route created, restarted, published, and selected exactly one candidate |
| `/v1/models` | Passed through the CPAR Base URL and staging client key |
| Real inference | `6/6` requests passed: Responses, Chat, and Messages, each JSON and SSE |
| Upstream attribution | All requests were sent via CPAR to the selected Console provider |

Console is `PASS` for this fresh account and this isolated route. The result covers text only;
media routes remain outside this task.

## Web

| Check | Result |
|---|---|
| Source | The same fresh Autoreg registration, admitted with a temporary one-hour staging lease |
| Credential shape | Web envelope carried both source session-cookie fields required by the upstream reference |
| Import | One Web credential batch accepted `1/1` |
| `/v1/models` | Passed through the CPAR Base URL and staging client key |
| Real inference | First Responses JSON attempt only; stop-on-first-failure rule applied |
| Failure | `http_5xx`, projected as `EgressRejected/egress`; `1/6` attempted |
| Protocol coverage | Chat, Messages, and SSE were not sent after the first failure |

Web remains `BLOCKED_WITH_EVIDENCE`. The fresh account and dual-cookie envelope were accepted by
CPAR, but the Oracle egress was rejected before a usable Web response. This is not evidence that
the new account is invalid, and it is not caused by the local 90-day safety policy.

## Differential and cleanup evidence

- A direct no-credential request to the same upstream origin returned `403` from Oracle Singapore
  while the corresponding Jakarta request returned `200`.
- The Oracle FlareSolverr response was parsed successfully, but the follow-up origin request still
  returned `403`; this classifies the remaining Web blocker as Oracle egress/WAF parity rather
  than a CPAR route or credential-shape failure.
- The Console and Web account batches were rolled back after the matrix. The isolated staging
  directory was moved to recoverable server trash and no staging process remained running.
- Autoreg configuration was restored after registration. Production health stayed `ok`, the
  active production version stayed `p12-09-codex-production-v2`, and the production Grok account
  count was unchanged.
- The global 90-day Web safety cap remains unchanged; the one-hour lease was test-only.

## Decision

P12-10I-22 closes the fresh-account Console text path for this staging run and retains Web as
`BLOCKED_EXTERNAL_EGRESS_ORACLE`. A later Web attempt is meaningful only after changing the
upstream exit/WAF condition or using an explicitly authorized equivalent egress; it must reuse the
same CPAR Base URL + client-key boundary and first-failure stop rule.
