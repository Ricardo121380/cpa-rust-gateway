# P12 / G12 closeout review

## Verdict

`PASS_WITH_EXPLICIT_DEFERRED_BOUNDARIES`

The exact closeout revision `bab02f1` passed the single formal Delivery Gate (run
`31396631571`). The old CPA was then stopped and disabled without deleting its data or rollback
materials, and CPAR-only readiness was rechecked successfully. This closes the available Release 1
text scope while keeping every externally blocked or deliberately deferred channel explicit. It
does not claim that Grok Web, Kiro OAuth, Official API-key E2E, or automatic Autoreg reauth is
complete.

## Findings

1. **Available text channels are independently attributed.** Codex, Krill, Grok Build, and Grok
   Console evidence is kept in separate provider/credential/egress domains. Build evidence is not
   used as Console or Web evidence.
2. **Console production evidence uses the correct runtime boundary.** The accepted Console matrix
   is the post-reload public matrix; the earlier pre-reload observation is not counted as final
   evidence because native account pools compile at service startup.
3. **Oracle Autoreg migration is single-active and reversible.** The Oracle source adapter,
   explicit CPAR provider binding, batch rollback material, and Jakarta fencing are recorded. The
   registration scheduler remains disabled/manual, so this closeout does not imply unattended
   replenishment.
4. **Web is fail-closed.** The known Oracle egress/WAF blocker remains `DEFERRED`/
   `BLOCKED_WITH_EVIDENCE`; no account validity or Web availability conclusion is inferred from
   Console or Build success.
5. **Long soak is not silently reintroduced.** The current acceptance uses bounded real JSON/SSE,
   route/health/database checks, rollback readiness, and review. The 72-hour and 1250-success
   figures remain optional operational observations only.
6. **CI frequency policy is preserved.** This closeout requests one explicit Fast + Full +
   supply-chain Delivery Gate. It does not require a separate expensive run for each documentation
   file or ordinary branch synchronization.
7. **Formal Gate and retirement evidence are complete.** Authorize, Fast, Full supply-chain, and
   Required all succeeded for the immutable revision. The old CPA units are inactive/disabled,
   CPAR remains active, old ports are absent, CPAR listeners remain present, and a root-only client
   key still obtains a valid public model-list shape. SQLite quick checks remain `ok`.

## Closeout actions completed

1. Local docs/full gates and final review passed.
2. Formal Delivery Gate run `31396631571` passed for exact revision `bab02f1`.
3. Old CPA was stopped/disabled after the gate; data, units, containers, and rollback preimages
   were retained.
4. CPAR-only readiness and SQLite integrity were rechecked successfully.
5. The development plan was updated to `P12 DONE_WITH_BOUNDARY`; P13 remains deferred pending a
   separate Change Request.

## Non-actions

- Do not start Web egress work or a new Web probe.
- Do not enable Autoreg scheduler, automatic reauth, or replenishment.
- Do not modify Caddy/DNS/CC Switch or create a new production route.
- Do not start P13 without a separate approved Change Request and scope/evidence plan.
