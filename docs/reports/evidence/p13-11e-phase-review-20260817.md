# P13-11E Provider-specific egress phase review — 2026-08-17

Status: `READY_FOR_FORMAL_DELIVERY_GATE`

## Review conclusion

P13-11E E0/E1/E2/E3, their focused evidence, the aggregate local Full receipt, and the frozen
scope are internally consistent. Independent goal-backward review found no remaining P1/P2
correctness, identity ownership, cross-Provider fallback, bounded-attempt, semantic-closure,
atomic-clearance, secret-projection, or evidence-attribution blocker.

This review authorizes only preparation of the exact source closeout candidate. Creating the
annotated tag and running the formal GitHub Delivery Gate still require explicit operator
authorization. This review does not authorize a Provider request, proxy/DNS/FlareSolverr probe,
Autoreg operation, server mutation, staging deployment, or production traffic.

## Goal-backward review

| Required property | Review result |
|---|---|
| Provider and Channel ownership | Generic compatible, Grok Build, Grok Console, Grok Web, Official, Codex/ChatGPT, Kiro/Claude and other compatible families retain explicit Provider/Upstream/Endpoint namespaces. “Krill” remains one generic endpoint instance rather than a global channel. |
| Exact lease identity | Build, Console and Web attempts consume the existing CPAR exact Credential lease and verify Provider, Channel, Endpoint, Credential ID, kind and revision before accounting or adapter execution. A foreign Endpoint or sibling lineage fails closed. |
| Failure-domain separation | Credential, Quota, Egress, Provider-session, Clearance and Adapter/Protocol state remain separate. One state transition cannot silently repair or poison another domain. |
| Failure attribution | Unknown `403` remains ambiguous and does not ban an account; confirmed account evidence remains Credential-owned; only an explicit sanitized clearance challenge marks the exact clearance lineage for a later attempt. |
| Bounded transport ledger | Build submits at most one inference. Console accounts for DPoP/bootstrap auxiliary work and suppresses legacy refresh/second inference in the E2 attempt. Web separately bounds Statsig, clearance recovery and one inference. |
| Semantic closure | After the first semantic event, replay, recovery completion/failure and a second inference are rejected. A terminal failure latch prevents later reclassification or mutation. |
| Atomic clearance ownership | Exact clearance refresh uses generation-owned tickets. Concurrent starts, live-owner overwrite, stale or same-deadline ABA tickets, expired ownership and foreign lineage completion fail closed. |
| No fallback | No E0-E3 path borrows another Provider, Channel, Credential, session, clearance or egress identity. Sticky-target loss fails closed rather than rotating across domains. |
| Secret and value-free boundary | IDs, states and reasons are bounded and value-free. Debug, error and receipt evidence contains no endpoint URL, API key, OAuth/SSO token, Cookie, proxy credential, request/response body or client-key digest. |
| Existing owner preservation | Existing Credential/Quota Health, account-pool scheduling, P13-11 generic compatible egress lease, and Provider adapter owners remain authoritative; E0-E3 do not create a second scheduler or Autoreg dependency. |
| Side effects | Implementation and verification are local deterministic, fake-transport or loopback evidence. No Provider, real proxy, DNS, FlareSolverr, Autoreg, server, staging, production or public traffic was used. |
| Frontend boundary | E0-E3 change no public or management OpenAPI, Prism contract/client or frontend code. No new Claude Code interface handoff is required. |

## Verification reviewed

- E0 planning/contract commit: `2f5ba8fc8dff2541238c866a03b48ac8a3bf3f0c`.
- E1 typed-state commit: `71af9e4e8e03fc7f59e95b6f252d274fdab9a3e0`.
- E2 Build/Console adapter commit: `f78302093e7e242c6bfcdd34f48514ea37f43cdb`.
- E3 Web/atomic-clearance implementation baseline:
  `7c182af13912a72be304448308dba7fda963ec82`.
- Aggregate local Full: `43/43` steps passed from `2026-08-17T15:10:21Z` through
  `2026-08-17T15:14:07Z` on `Darwin 25.2.0 arm64`; durable receipt:
  `p13-11e-aggregate-full-20260817.md`.
- Current focused totals: Web `11/11`, exact-clearance `13/13`, Build/Console `4/4`,
  gateway-router `164/164`, and gateway-upstream `37/37`.
- Historical E0/E1/E2 reports remain point-in-time receipts and retain their commit-time wording
  and totals.
- The four pre-existing untracked helper files remained outside every staged commit.

## Explicit acceptance boundary

`DONE_WITH_BOUNDARY` for E0-E3 will mean that the Provider-local state/attempt/atomic-clearance
seam is accepted. It will not mean that native Provider proxy pools or real Grok Web egress are
implemented or usable:

- Build/Console use typed Direct egress in the current application composition; native
  Config-Version-owned fixed/pool node wiring is not part of E0-E3.
- Web remains a transport-free seam and is not connected to production Web transport.
- No account, proxy, Statsig, clearance, FlareSolverr, DNS, public CPAR, staging or production
  result is represented by this review.
- Autoreg remains a separate project and is not a CPAR credential-registration or repair worker.

## E4/E5 decision

- E4 exact egress/session/clearance management projection is useful but not required for E0-E3
  correctness or security acceptance. It is marked `DEFERRED_OPTIONAL` and must start later under
  an independent Task Card/CR. If it changes the management surface, the required order is
  authoritative OpenAPI, Prism synchronization/generated client, cross-boundary log, then Claude
  Code handoff.
- E5 real Provider/proxy/DNS canary remains deferred and unauthorized. It requires a new explicit
  target, request/budget limit, value-free receipt, rollback rule and operator approval.

## Formal closeout target

- Branch: `codex/p13-11-egress`.
- Candidate immutable annotated tag: `phase-p13-provider-egress-complete`.
- Candidate tag message: `P13-11E provider-specific egress closeout`.
- The tag must point to the exact pushed closeout commit containing this review and the aggregate
  Full receipt; it must not point to the implementation-only baseline.
- Existing `phase-p13-egress-complete` and `phase-p13-egress-management-complete` tags must not be
  moved, deleted, recreated or reinterpreted.
- Only one tag-triggered `delivery-gate` run is allowed for this target. Authorize, Fast, Full
  supply-chain and Required must all succeed before E0-E3 become `DONE_WITH_BOUNDARY`.

## Decision

P13-11E E0-E3 are `READY_FOR_FORMAL_DELIVERY_GATE`. A failed or cancelled formal job leaves them
open and requires repair on a new exact closeout target; it does not permit starting E4/E5, a later
task, or weakening any Provider/Channel/egress/session/clearance boundary.
