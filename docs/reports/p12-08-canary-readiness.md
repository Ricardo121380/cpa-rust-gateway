# P12-08 Canary readiness report

Status: `BLOCKED`

Date: 2026-08-02

## Result

P12-08 production Canary did not start. The read-only inventory found the new gateway active on
loopback with one active configuration graph, one active credential binding, one active route and one
dedicated `rgw_` client key. The management availability, catalog status and observability endpoints
returned HTTP 200 with the active configuration header. The live Caddy configuration validated, still
routes the production site entirely to the incumbent, and contains no public management-plane route.

The required double-acceptance precondition was then tested in the smallest reversible transaction. A
root-only backup of the incumbent CPA configuration was created, the dedicated gateway key was added to
the CPA `api-keys` list, and the incumbent service was restarted. The gateway accepted the key (HTTP 200
on its local models request), but the incumbent returned HTTP 401. The CPA configuration was restored
byte-for-byte from the preimage and the service was restarted; the incumbent is active and its original
key count is restored. No Caddy reload, DNS change, public traffic split, new gateway publish, or upstream
request was performed by this attempt.

## Gate decision

`CR-P12-08-002` remains open but blocked. Do not enter 10% until CPA accepts the same `rgw_` key during a
fresh, auditable double-acceptance check. The failure is an incumbent key-acceptance/configuration
compatibility issue, not evidence of a new-gateway protocol failure. The next bounded investigation is
to determine CPA's supported key-registration path or key format, without weakening the `rgw_` Caddy
namespace and without changing the incumbent's existing keys.

Evidence is value-free: no endpoint, credential, model, request/response body, key value, or token
fingerprint is recorded.
