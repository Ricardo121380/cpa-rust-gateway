# P12-10I-19 Autoreg new-account Console/Web CPAR receipt

Status: `BLOCKED_WITH_EVIDENCE`

## Boundary

One new Autoreg Grok account was registered successfully and its Console SSO and Web session
artifacts were transferred through a value-free controlled pipe into an Oracle Singapore
root-only staging copy. Web source expiry was capped at CPAR's existing 90-day local window.
Each provider used a separate temporary CPAR route and client key. Production CPAR, public
listeners, Caddy/DNS, CC Switch, Autoreg, and the old CPA were not changed.

## Value-free results

| Provider | Import | Models preflight | Requests | Result |
|---|---|---|---:|---|
| Console | PASS | PASS | 2 | 1 success, then Chat JSON `http_5xx`; stopped on first failure |
| Web | PASS (expiry capped) | PASS | 1 | `http_5xx`, attempt classified `EgressRejected/egress`; stopped on first failure |

The Console Responses JSON request reached a successful upstream attempt and produced durable
usage. Its next Chat JSON request failed before a completed attempt was recorded, so the remaining
Chat/Messages/SSE matrix was not sent. The Web Responses request reached the Web adapter and an
upstream attempt, which was rejected at the egress boundary. No request/response body, endpoint,
model secret, account identity, Cookie, SSO, OAuth value, or token fingerprint was retained.

## Cleanup

Both temporary graphs were rolled back, the imported account batches were removed, the staging
gateway was stopped, and loopback listeners were cleared. The staging database returned to its
pre-test state; production remained unchanged.

## Verdict

The new Autoreg account proves one successful Console text request but does not close Console
cross-protocol acceptance. Web remains blocked at the external egress boundary despite valid
session admission and a real CPAR request. Neither provider is production-ready based on this
receipt.
