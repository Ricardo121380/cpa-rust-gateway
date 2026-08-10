# P12 / G12 closeout receipt

## Status

`READY_FOR_G12`

This receipt is the immutable closeout candidate for the P12 delivery gate. It consolidates the
completed production evidence without starting a new provider probe. The Web egress boundary,
automatic Autoreg reauth/replenishment, Kiro OAuth, and Official API-key E2E remain explicitly
deferred and are not promoted by this receipt.

## Accepted production text channels

| Channel | Result | Evidence |
|---|---|---|
| Official Codex / ChatGPT OAuth | PASS | `p12-codex-official-endpoint-switch-20260809.md`; three client projections and JSON/SSE evidence |
| Krill | PASS | `p12-production-channel-add-20260810.md`; independent upstream, credential, egress, and failure domain |
| Grok Build | PASS | `autoreg-oracle-cpar-production-canary-20260810.md`; explicit Oracle source import and Responses/Chat/Messages JSON/SSE `6/6` |
| Grok Console text | PASS | `autoreg-oracle-console-production-canary-20260810.md`; post-reload Responses/Chat/Messages JSON/SSE `6/6` |
| Grok Web text | DEFERRED / BLOCKED_WITH_EVIDENCE | Oracle egress/WAF remains externally blocked; no new Web request is sent in this closeout |
| Grok Web media | DEFERRED | Typed media protocol/HTTP contract is a separate scope |

All live matrices were performed through the real CPAR public data plane with a client key. Native
provider probes and loopback checks are supporting evidence only; they do not replace the public
HTTP evidence.

## G12 closeout evidence

| Gate | Result | Boundary |
|---|---|---|
| Current available text channels | PASS | Codex, Krill, Grok Build, and Grok Console have bounded JSON/SSE evidence |
| Health and route admission | PASS | Active Config Version, provider binding, listeners, and route explanations remain consistent |
| Database integrity | PASS | Production SQLite `quick_check=ok`; no new P0 integrity finding in the closeout evidence |
| Rollback readiness | PASS | P12-09 rollback preimage, Oracle/Jakarta migration preimage, and CPAR batch rollback material retained |
| Secret/credential boundary | PASS | No credential value, token, Cookie, request body, response body, or raw trace is stored in this receipt |
| P0/P1 closeout review | PASS_WITH_NO_NEW_FINDING | No new P0/P1 condition is present in the bounded evidence; historical provider-specific blockers remain classified |
| 72-hour / 1250-success observation | OPTIONAL | Removed as a hard gate by `CR-P12-10I-023` |
| Old CPA retirement | PENDING_G12 | The old CPA remains available as rollback target until the formal Delivery Gate succeeds |
| Formal Delivery Gate | PENDING | This exact closeout candidate is the requested target for one Fast + Full + supply-chain run |

## Production invariants

- Oracle Autoreg is the single active Autoreg service; Jakarta remains fenced/stopped with rollback
  material retained.
- Autoreg registration scheduler and automatic reauth remain manual/disabled.
- CPAR, Caddy, DNS, CC Switch, and the old CPA are not changed by preparing this receipt.
- No provider fallback, silent pool merge, or Web retry is introduced.
- Ordinary push, PR update, review, rebase, and merge frequency remains unrestricted; only the
  explicit P12 closeout invokes the expensive Delivery Gate.

## Closeout decision

This is a `READY_FOR_G12` candidate, not yet the final P12 status. After the formal Delivery Gate
passes, the operator may execute the already-reviewed old-CPA retirement step, perform a short
CPAR-only health/readiness check, append the gate and retirement receipts, and mark P12
`DONE_WITH_BOUNDARY`. If the gate fails, do not retire the old CPA or start P13; repair the earliest
affected closeout target and rerun only that P12 gate.
