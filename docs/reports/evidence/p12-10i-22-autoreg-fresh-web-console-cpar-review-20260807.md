# P12-10I-22 Autoreg fresh Web/Console CPAR HTTP review

## Review verdict

`BLOCKED_WITH_EVIDENCE` is the correct task-level result: Console text passed the real CPAR HTTP
matrix, while Web is externally blocked at the Oracle egress/WAF boundary. The two outcomes are
kept separate and are not generalized across channels.

## Findings

1. **Fresh-account provenance — PASS.** Autoreg task 59 completed one registration. The credential
     value traversed only the remote controlled pipe and encrypted staging importer; no secret was
   written to the repository or receipt.
2. **Console public boundary — PASS.** The route was isolated, restarted, and selected one
   candidate. CPAR `/v1/models` preflight passed, followed by six real client-key requests covering
   Responses, Chat, and Messages in JSON and SSE; all six were sent through CPAR and passed.
3. **Web public boundary — BLOCKED_WITH_EVIDENCE.** The fresh one-hour lease and dual-cookie
   envelope were admitted and passed `/v1/models`, but the first Responses JSON request returned a
   value-free `EgressRejected/egress` 5xx. Stop-on-first-failure prevented accidental protocol
   expansion or retries.
4. **External attribution — PASS.** The same-origin direct differential (Oracle `403`, Jakarta
   `200`) and the parsed-but-still-403 FlareSolverr attempt locate the Web failure at exit/WAF
   parity. No account-invalid conclusion is made from this run.
5. **Policy and cleanup — PASS.** The global 90-day Web cap was not removed; only a one-hour test
   lease was used. Both account batches, staging route, processes, and temporary files were
   rolled back or moved to recoverable trash. Production health, active version, and account count
   remained unchanged.

## Local verification

The implementation changes were reviewed with provider/gateway targeted tests, strict Clippy,
formatting, whitespace, plan-state, documentation-link, and secret-scan gates. No GitHub CI run is
required for this documentation-and-staging closeout under `CR-EXEC-008`.

## Remaining work

Console text is closed for this fresh-account run. Web remains externally blocked; resume only with
an authorized equivalent egress/WAF condition or a fresh differential that changes that evidence.
Media routes and any production activation are outside this task.
