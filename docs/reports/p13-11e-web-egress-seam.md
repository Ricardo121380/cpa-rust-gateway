# P13-11E E3 Grok Web sticky egress/session/clearance seam report

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

Date: 2026-08-17

## Outcome first

P13-11E E3 adds a transport-free, Provider-local Grok Web attempt seam above the E1 typed
egress/session/clearance state. One logical attempt is bound to an already-owned CPAR
`CredentialLease`, one exact `grok.web` Upstream/Endpoint namespace, one named sticky egress
identity, one Provider-session lineage and one clearance lineage. It does not select an account,
rotate to a sibling, call Autoreg, open a proxy, resolve DNS, invoke FlareSolverr or contact Grok.

The seam deliberately does not copy the legacy Web behavior that may refresh clearance and send a
second inference after a `403`. E1 permits one inference submission and only pre-inference
recovery. E3 therefore classifies the first failure, ends the logical attempt and, only for an
explicit sanitized clearance challenge, marks the exact lineage for one bounded recovery in a
later logical attempt.

## Implemented boundary

- `gateway-router` provides an atomic exact-clearance refresh ownership ticket. Challenge marking,
  refresh begin, completion and failure execute under one state lock; a live owner cannot be
  overwritten, a stale or expired ticket cannot complete a replacement owner, and sibling
  Provider/session/target lineages remain unchanged.
- `provider-grok` provides a dedicated `GrokWebProviderEgressAttempt`. Construction rejects Direct
  egress, foreign Provider/Upstream/Endpoint identities, a non-`grok_web_sso` lease, Credential
  revision mismatch, inactive session, foreign clearance target and blocked exact egress before a
  transport could run.
- Statsig environment and signer submissions, clearance refresh and the sole inference have
  separate counters within the finite E1 Web budget. Cache/local computation is not counted as
  physical HTTP. The fifth auxiliary submission, a second recovery, a second inference, or any
  operation after the first semantic event fails closed.
- Unknown `403` evidence remains `AmbiguousProvider/None` and changes no state. Independently
  confirmed forbidden evidence remains Credential-owned and does not poison egress or clearance.
  Only the closed `ClearanceChallenge` evidence marks the exact clearance `RefreshRequired`; it
  cannot cause a same-attempt replay.
- Receipt and `Debug` projections retain only bounded opaque IDs, revisions, closed states and
  counts. They retain no URL, proxy value, Cookie, SSO/OAuth token, Statsig value, request body or
  raw Provider response.

## Why production Web wiring remains outside E3

The current process-level Web proxy envelope does not expose a stable, non-secret egress node or
profile ID. E1 correctly rejects `Direct` for Web, while deriving a sticky identity from a raw
proxy URL or an account reference would either leak configuration or conflate account and egress
ownership. E3 therefore proves the attempt/state contract using an injected opaque named target.

Real proxy-node bindings, physical Statsig/FlareSolverr accounting, response-to-challenge evidence,
and a public Provider canary require a later explicitly reviewed configuration/network CR. The
legacy no-attempt P12 Web adapter remains unchanged and is not counted as E3 evidence.

## Local evidence

The deterministic fixtures cover:

1. exact named sticky egress, live `grok_web_sso` lease, active session and absent/fresh clearance;
2. Direct, wrong Provider/channel/kind/revision/session/target and blocked egress rejection;
3. zero/two/four hidden Statsig calls, fifth-call rejection and one sole inference;
4. atomic clearance singleflight, completion/failure, expired/stale ticket rejection, same-deadline
   ABA protection and sibling isolation;
5. unknown `403`, confirmed account forbidden and explicit clearance challenge as three different
   ownership paths;
6. one later pre-inference fake recovery, no current-attempt replay and semantic-event closure; and
7. value-free snapshots plus zero Provider, DNS, proxy, Store, Autoreg and FlareSolverr calls.

## Verification

- `cargo test --locked -p gateway-router provider_egress_state -- --nocapture`: `13/13` passed.
- `cargo test --locked -p gateway-router --all-targets`: `164/164` passed.
- `cargo test --locked -p provider-grok --test p13_11e_web_egress -- --nocapture`: `11/11`
  passed.
- `cargo test --locked -p provider-grok --test p13_11e_native_egress`: `4/4` passed,
  including the exact foreign-Endpoint lease rejection added during review.
- `cargo test --locked -p provider-grok --all-targets`: all runnable tests passed; the existing
  externally authorized probes remained ignored and were not executed.
- `cargo test --locked -p gateway-upstream --all-targets`: `37/37` passed.
- `cargo check --locked -p gateway`: passed after mapping `EndpointMismatch` to the existing safe,
  non-retryable credential-unavailable boundary.
- strict Clippy passed for `gateway-upstream`, `gateway-router`, `provider-grok`, and `gateway`, with
  all targets, all features, and `-D warnings`.
- `cargo fmt --all -- --check`, `./scripts/check.sh docs`, and `git diff --check` passed.

Independent review found and closed four correctness gaps before acceptance: semantic-event versus
clearance-challenge mutation is now one `ledger -> Provider runtime` critical section; the first
successful failure classification is terminal; a live clearance owner cannot be overwritten by a
generic setter; and native/Web attempts both prove the leased Endpoint, not merely a matching
Credential ID/revision. The final review found no remaining behavior blocker.

## Explicit non-evidence

This slice does not prove that a current Grok Web account, SSO/Cookie, Statsig service, proxy node,
clearance, FlareSolverr instance, DNS path, public CPAR URL or Provider response works. It does not
register or repair accounts and it does not read or run Autoreg. It does not change OpenAPI, Prism,
the management UI, server configuration, staging or production.

E3 remains part of the single P13-11E phase review. It does not run a formal Delivery Gate by
itself. After local review, the next action is an aggregate E0-E3 closeout decision; optional E4
management projection and E5 real-network canary do not start automatically.
