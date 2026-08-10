# Review: Autoreg Oracle → CPAR production canary

## Scope

Reviewed the single-account import receipt, native probe, public CPAR matrix and production
invariance claims for `CR-P12-AUTOREG-MIGRATE-001`. No secret-bearing file or response body was read
into the review.

## Checks

- Import cardinality is exact (`1/1/1`) and explicitly bound to native `grok_build`.
- The public matrix covers Responses, Chat Completions and Anthropic Messages in both JSON and SSE.
- The authoritative matrix is isolated from the earlier mixed historical diagnostic.
- Every tuple is single-attempt and value-free; no failure was hidden by fallback.
- Expiry-derived refresh scheduling and priority ordering prevent the known expired Build account from
  masking the fresh canary account.
- CPAR Config Version, legacy CPA, Caddy/DNS, CC Switch and grok2api invariants are recorded.

## Findings

`PASS_WITH_ROLLBACK_READY`: the requested provider-binding/import and real public HTTP evidence are
complete. The result is deliberately narrower than a general Grok certification: Web remains blocked
by external egress/WAF behavior, Console has its own upstream boundary, and Autoreg automatic
registration remains disabled/manual pending a separate operator-approved canary.
