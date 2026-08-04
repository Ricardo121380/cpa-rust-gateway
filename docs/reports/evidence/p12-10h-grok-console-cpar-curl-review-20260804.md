# P12-10H Grok Console public CPAR curl review

Status: `REVIEWED_BLOCKED_NO_GROK_ROUTE`

## Scope reviewed

- The real CPAR data-plane execution of `run-p12-10h-grok-cpar-e2e.py`.
- Authenticated `/v1/models` preflight and the zero-inference stop condition.
- Separation between the native Console probe receipt and public HTTP E2E evidence.
- Loopback, production, credential, and cleanup invariants.

## Findings

1. The test did use the CPAR data-plane endpoint and client-key authentication. The preflight
   returned HTTP 200 JSON, but no Grok/Console model was visible.
2. The harness correctly stopped with `grok_route_missing` before sending inference. No other
   channel was counted as Console, and no unbounded retry or fallback occurred.
3. The prior five-account native Console probes remain valid as provider-adapter evidence only;
   they cannot close public HTTP E2E.
4. No active graph, source account, service, public route, or temporary secret file was changed by
   this check.

## Verdict

The public Console HTTP test is correctly **blocked**, not passed. P12-10H remains open until an
isolated native Console route is visible and the same CPAR curl harness completes the approved
protocol matrix. The real HTTP requirement is now the authoritative acceptance rule for subsequent
CPAR channel tests; native probes are supplemental evidence only.

