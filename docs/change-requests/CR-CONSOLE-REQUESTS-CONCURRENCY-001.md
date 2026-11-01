# Concurrent request-ledger reads return 500

Status: open — observed by the frontend (Claude Code) while verifying `web/console` against a real gateway.

## Observation

A freshly initialized gateway (`develop` at the `feature/management-console` base, debug build,
`gateway serve` with one management worker, empty state directory) answers concurrent reads of
`GET /admin/requests` and `GET /admin/requests/summary` with:

```
500 {"error":{"code":"management_internal_error","message":"Management operation failed"}}
```

The same requests succeed when sent one at a time. Reproduction with a session token `T`:

```bash
now=$(($(date +%s)*1000)); f=$((now-86400000))
for i in $(seq 1 6); do
  curl -s -w " %{http_code}\n" "http://127.0.0.1:18181/admin/requests/summary?from_ms=$f&to_ms=$now&bucket_ms=600000" \
    -H "X-Management-Key: $T" &
done; wait
```

Typically 2–5 of 6 return 500; with 24 parallel pairs about three quarters fail. The gateway logs
nothing for these failures. A management overview page legitimately issues two summaries and one
list at once, so a normal page load hits this.

## Likely cause

`crates/gateway-http-actix/src/management_resources/request_history.rs` maps every store error
other than `InvalidPersistedGatewayEvent` and `ConfigVersionRevisionConflict` to
`internal_error()`. A transient store condition under concurrency (presumably SQLite busy/locked on
the operations reader) is therefore reported as a permanent server fault.

## Requested change

Either serialize/queue these reads, or report a transient store condition as
`503 management_operations_busy` instead of 500 — the code `ManagementOperationsError::ReadCapacityExceeded`
already produces in `management_resources.rs` — ideally with `Retry-After`, and log the underlying
store error class. If that code becomes part of the answer, please also name it in
`docs/openapi/management-v1.json` so clients can tell "busy, retry" from "not enabled" (both are 503).

## Frontend meanwhile

`web/console` retries 500s and `management_operations_busy` twice with backoff and shows a retry
action when they persist. It treats other 503s as "not enabled in this deployment". It does not hide
or fabricate data.
