# P12-10H CPAR curl E2E preflight receipt

## Result

- Status: `BLOCKED_NO_GROK_ROUTE`
- Harness: `run-p12-10h-grok-cpar-e2e.py`
- Requested CPAR inference calls: `100`
- CPAR inference calls sent: `0`
- Successful calls: `0`
- Failure category: `grok_route_missing`
- `upstream_request`: `not_sent`

## What was actually tested

The new harness was executed on the current server and used the existing root-only CPAR endpoint
and client-key boundary. It made one authenticated `GET /v1/models` request through CPAR, without
printing or retaining the endpoint, key, model names, or response body. The value-free preflight
projection was:

```text
models_request=ok
model_count=3
grok_route=no
```

The harness therefore stopped before the inference loop. It did not send a Chat, Responses, or
Messages request, so no non-Grok channel was misreported as a Grok result.

## Intended 100-call E2E once the route exists

After a native CPAR Grok model is present and bound to an active client key, the same harness will
send exactly 100 requests through the CPAR base URL using `curl`, round-robin across Chat,
Responses, and Messages and alternating JSON/SSE. It will check only bounded status/content-type,
JSON shape, SSE terminal lifecycle, and value-free counts; it stops at the first failure and writes
no request or response body to the receipt.

## Boundary and next action

- `grok2api` remains stopped as previously authorized; CPAR remains active.
- No CPAR config version, route, credential, client key, Caddy, DNS, old CPA, or CC Switch state was
  changed by this preflight.
- Temporary endpoint/model files were removed after the run; no credential value entered the
  repository or receipt.
- The blocker is structural, not a 72-hour/sample-size issue: CPAR currently has no visible Grok
  route. The next execution must first bind the native Grok account pool in an isolated or approved
  CPAR graph, then rerun this direct HTTP harness.
