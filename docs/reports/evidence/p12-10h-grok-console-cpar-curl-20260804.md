# P12-10H Grok Console public CPAR curl receipt

Status: `BLOCKED_NO_GROK_ROUTE`

Date: 2026-08-04

## Boundary

This was a real data-plane check against the active CPAR instance on the Oracle Singapore host.
The harness read the loopback CPAR endpoint, an existing client key, and a fixed Console test model
from root-only `0600` files. None of those values, model names, request bodies, response bodies,
tokens, or fingerprints are retained here.

The test used `run-p12-10h-grok-cpar-e2e.py`, which invokes `curl` for every HTTP operation and
stops before inference when the authenticated model preflight cannot prove a Grok route.

## Result

| Measure | Result |
|---|---:|
| Requested inference calls | 100 |
| Authenticated `/v1/models` preflight | `200` / JSON |
| Grok/Console model visible | `no` |
| Inference calls attempted | 0 |
| Successful calls | 0 |
| Failure category | `grok_route_missing` |
| Upstream request | `not_sent` |

The harness therefore did not send a Chat, Responses, or Messages request. This is an intentional
fail-closed result, not a Console success or an upstream failure.

## Interpretation

The earlier five-account Console result was a native provider probe. It verified account attribution,
Canonical completion, and three protocol projections, but it did not prove the public CPAR
`base URL + client key` path. This receipt is the missing public-path check and shows that the
current active CPAR graph has no visible Grok/Console route to exercise.

## Invariants and next action

- CPAR remained active with its existing loopback listeners.
- No Config Version, route, credential, client key, Caddy, DNS, old CPA, CC Switch, or grok2api
  state was changed.
- Temporary input and receipt files were removed after the run.

The next live Console test is allowed only after a native Console route and account pool are bound
in an isolated CPAR graph. It must reuse this public curl harness and stop on the first failure.
