# P12-10H native Grok Console route and public CPAR curl review

Status: `REVIEWED_BLOCKED_EXTERNAL_CONSOLE_403`

## Review scope

- Exact revision `82ceb1d7c6899f3c615d7ed1cff2e400d5e1118a`, signed ARM artifact provenance, and
  local gates.
- Isolated staging route publish, single-candidate explanation, account import/rollback, and
  loopback-only service boundary.
- Real CPAR base URL + client-key HTTP execution for Chat, Responses, and Messages JSON/SSE.
- Separation of implementation/protocol evidence from external provider availability.

## Findings

1. The route was bound and visible only in isolated staging. The public `/v1/models` preflight and
   protected route explanation passed; there was no zero-send `grok_route_missing` shortcut.
2. All six protocol/mode requests were sent through CPAR and reached the upstream boundary. Each
   stopped at the same projected `EgressRejected / egress` category, consistent with an upstream
   403. No request-conversion `ClientRequestError`, cross-provider fallback, or retry was observed.
3. The output-limit regression is fixed narrowly: the supported Responses extension is consumed and
   mapped to the Console request; unrelated extensions remain fail-closed. The targeted test and
   local fast gate passed.
4. Four imported Console accounts did not change the external result. Their accounts were rolled
   back transactionally; the source pool and production CPAR state were not modified.
5. The staging lifecycle restart exposed that endpoint capabilities are snapshotted at process
   start. Restarting the isolated unit before validation was required; this did not affect the
   production service. After the run, the route DB was quarantined and a clean staging DB was
   created because the in-memory predecessor needed by the management rollback endpoint was not
   available after restart.

## Verdict

The implementation and public data-plane plumbing are reviewable and reproducible, but the live
Console acceptance is **blocked**, not passed. `P12-10H` remains `BLOCKED_EXTERNAL_CONSOLE_403`.
The next valid acceptance requires a provider-side fix or newly valid Console sessions, followed by
the same bounded CPAR curl matrix. No production cutover, public route change, or grok2api source
mutation is authorized by this receipt.
