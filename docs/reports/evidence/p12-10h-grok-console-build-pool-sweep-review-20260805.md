# P12-10H Grok Console/Build eligible-pool sweep review

Status: `REVIEWED_CONSOLE_EGRESS_BLOCKED_BUILD_PASS`

## Review scope

- Read-only grok2api export, CPAR encrypted pool import, and source-count/eligibility filtering.
- CPAR public base URL + client-key preflight, route explanation, and real upstream attempts.
- Credential rotation evidence, failure ownership, rollback, and production invariants.

## Findings

1. The sweep did not merely repeat one account: the CPAR attempt ledger recorded 25 distinct
   Console credential IDs, one request per credential, with no retry or cross-provider fallback.
2. All 25 Console requests reached the upstream boundary and converged on `EgressRejected/egress`.
   The earlier local `RouteNotFound` activation checks had zero upstream attempts and were excluded
   from the provider result.
3. The one Build record that satisfied the existing import contract passed a real CPAR Responses
   JSON request. No claim is made for the 828 Build records that still require reauthentication or
   for source records lacking the required model evidence.
4. Account encryption, pool compilation, route selection, public authentication, and transactional
   rollback behaved as designed. No production state was touched.

## Verdict

The pool mechanism and test workflow are healthy. The remaining Console blocker is external
provider/session access, not failure to rotate credentials. P12-10H remains open until a valid
Console session pool can produce a successful public CPAR call and the remaining Build OAuth repair
is addressed.
