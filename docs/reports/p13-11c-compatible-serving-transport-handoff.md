# P13-11C compatible serving transport handoff report

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

Date: 2026-08-16

## Intended outcome

Connect the P13-11B generic compatible runtime to the real serving adapter without creating a
second Credential scheduler, weakening JSON/SSE deadlines, or allowing cross-Provider fallback.

## Frozen boundary

Local implementation and deterministic tests only. No real Provider request, proxy probe, DNS,
server, staging, production, Autoreg, credential refresh, Grok Web clearance, management OpenAPI,
Prism or frontend change is authorized by this task.

## Implementation evidence

Implemented the request-time handoff in the production composition and serving driver:

- generic OpenAI Chat, OpenAI Responses, and Anthropic Messages Endpoints receive a
  `CompatibleEndpointRuntime` compiled from the same active Config Version and shared
  Credential/Health/Quota registries;
- the existing `RouteCredentialScheduler` remains the only Credential lease owner; the driver
  obtains only an exact egress lease for the already-selected Credential and never advances a
  Credential cursor a second time;
- the selected egress proxy is overlaid onto the existing response-mode transport profile, so
  JSON and SSE timeout/cache/emulation settings remain mode-specific; the egress lease is retained
  by the response source until drop/cancel/error;
- transport send failures feed back to the profile's exact Endpoint, Endpoint-Credential, or
  Egress-Node scope and return `CompatibleEgress` so the orchestrator does not add a duplicate
  Endpoint cooldown;
- native Grok/Kiro/Provider-specific adapters remain outside this generic handoff.

## Verification

Local verification passed:

- `gateway-upstream`: 37 tests;
- `gateway-router`: 151 tests, including exact Credential-to-egress handoff, scoped feedback,
  lease release, and unavailable sticky-node fail-closed behavior;
- `gateway-control`: 72 tests, including active generic runtime compilation and explicit proxy-pool
  fixture coverage;
- `gateway` binary/tests: 106 tests;
- strict Clippy for `gateway-router`, `gateway-upstream`, `gateway-control`, and `gateway`;
- `cargo fmt --all`, `git diff --check`, and `CHECK_REPORT_PATH=/tmp/cpar-p13-11c-fast-20260816-final.md
  ./scripts/check.sh fast` (PASS).

No Provider, DNS, proxy, server, staging, production, or GitHub Delivery Gate was run.

## Review

Local review completed with no P1/P2 correctness findings in the implemented boundary. The review
confirmed that native Provider adapters are not routed through the generic egress registry, the
same Credential lease is not reacquired, mode-specific profiles are preserved, egress capacity is
RAII-released, and scoped transport feedback does not trigger a second Endpoint cooldown.

## Next boundary

After the phase-specific review and one explicitly authorized phase Delivery Gate, P13-11 may add
a separately approved Config-Version proxy-pool management surface and explicitly authorized
Provider-specific probes/recovery. P13-11C does not infer such configuration from a Provider name
or credential format, and the current deployment remains Direct-only.
