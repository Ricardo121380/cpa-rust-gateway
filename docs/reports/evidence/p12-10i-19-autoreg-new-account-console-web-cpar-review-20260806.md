# P12-10I-19 Autoreg new-account Console/Web CPAR review

Decision: `BLOCKED_WITH_EVIDENCE`

1. **PASS — registration and transfer.** Autoreg completed one new registration; independent
   Console and Web envelopes imported into the isolated native pool with no rejected records.
2. **PASS — real CPAR boundary.** Both routes passed `/v1/models` preflight and used temporary
   client keys over the actual CPAR data listener.
3. **BLOCKED — Console.** Responses JSON succeeded once, then Chat JSON returned `http_5xx`
   before a completed attempt. The remaining protocol matrix was correctly not retried blindly.
4. **BLOCKED — Web.** The capped session was admitted and the request reached an upstream attempt,
   which returned the value-free `EgressRejected/egress` category.
5. **PASS — isolation.** Route rollback, account-batch rollback, listener cleanup, and production
   invariants all passed.

The Web route helper was also corrected to remove a Build-only reasoning override that violated the
runtime route-access shape; the corrected helper passed Python compilation and the subsequent Web
staging graph started successfully.
