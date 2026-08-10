# P12 / G12 closeout receipt

## Status

`DONE_WITH_BOUNDARY`

This receipt is the final P12/G12 closeout record. The formal Delivery Gate passed for the exact
revision `bab02f1` (run `31396631571`), and the old CPA services were stopped and disabled after
the gate. Data, units, containers, rollback preimages, and the Jakarta source project remain
retained for recovery. The Web egress boundary, automatic Autoreg reauth/replenishment, Kiro
OAuth, and Official API-key E2E remain explicitly deferred and are not promoted by this receipt.

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
| Old CPA retirement | PASS | `cli-proxy-api.service` and `cpa-manager-plus.service` stopped and disabled at `2026-08-10T14:24:59Z`; files, containers, data, and units retained |
| Formal Delivery Gate | PASS | Exact revision `bab02f1` / run `31396631571`: Authorize, Fast, Full supply-chain, and Required all succeeded |
| CPAR-only readiness | PASS | CPAR service active; old CPA ports `8317/18317` absent; CPAR listeners `18180/18181` present; public client-key `/v1/models` HTTP 200 with six-model response shape; production SQLite quick checks `ok` |

## Production invariants

- Oracle Autoreg is the single active Autoreg service; Jakarta remains fenced/stopped with rollback
  material retained.
- Autoreg registration scheduler and automatic reauth remain manual/disabled.
- CPAR, Caddy, DNS, and CC Switch are unchanged; old CPA is now stopped/disabled as the planned
  direct-replacement cutover, with rollback material retained.
- No provider fallback, silent pool merge, or Web retry is introduced.
- Ordinary push, PR update, review, rebase, and merge frequency remains unrestricted; only the
  explicit P12 closeout invokes the expensive Delivery Gate.

## Closeout decision

P12 is now `DONE_WITH_BOUNDARY`. The bounded Release 1 text scope is closed with explicit external
and deferred boundaries. P13 is not started; it requires a separate approved Change Request and
new scope/evidence plan. Rollback remains available by re-enabling the retained old CPA units and
using the preserved preimages; no rollback was needed during this closeout.
