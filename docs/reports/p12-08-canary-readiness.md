# P12-08 Canary readiness report

Status: `SUPERSEDED_BY_DIRECT_REPLACEMENT`

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

## Corrected gate decision

The user clarified that CPAR replaces CPA rather than sharing production traffic with it.
`CR-P12-ROLLOUT-002` therefore supersedes the double-acceptance gate: CPA does not need to accept a
CPAR `rgw_` key, and the observed 401 is not a blocker. P12-08 now prepares a direct, all-traffic
cutover plus an explicit per-client key migration and rollback list. P12-09 switches the production
hostname entirely to CPAR, exercises one complete rollback and recovery, and P12-10 disables the old
CPA after the 72-hour CPAR observation passes.

Evidence is value-free: no endpoint, credential, model, request/response body, key value, or token
fingerprint is recorded.
