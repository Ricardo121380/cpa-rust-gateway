# P12-10I-01 Native Grok Reauthentication — Review

Date: 2026-08-05
Review scope: `reauth.rs`, migration 0013, account-pool/worker hooks, integration tests and plan
receipt
Verdict: PASS for the approved local scope; no live OAuth or deployment approval implied

## Findings

No blocking findings remain in the reviewed scope.

### Correctness

- Claims are stored independently from the normal refresh/quota worker claim and are leased with a
  bounded expiry.
- Success requires the exact account revision, live reauth claim, provider-specific credential
  validation, and one atomic encrypted replacement; a stale claim rolls the transaction back.
- Transient failures receive bounded backoff. Interactive/denied outcomes set an explicit
  `operator_required` flag, preventing an immediate retry loop; `requeue_reauth` is the only
  explicit release path.
- The coordinator limits a pass to 200 accounts and never starts concurrent OAuth/browser actions.

### Security and isolation

- Plaintext credential bytes are available only through the immediate injected executor job and are
  redacted from `Debug` output and durable receipts.
- Provider identity remains an existing opaque digest; the new table contains account ID and bounded
  scheduling/claim state only.
- No password, browser cookie, SSO value, Bearer value, endpoint, model, or response body is read
  or persisted by this task.
- The source project's grok2api refresh helper is not a runtime CPAR dependency.

### Validation

- Provider reauth integration: 3/3 passed.
- Gateway-store migration and existing persistence suite: 41/41 passed.
- Provider Clippy with `-D warnings`: passed.
- Existing untracked helper `scripts/p12-06-rotate-kiro-credential.py` was preserved and is outside
  this task's ownership.

## Deferred items

The following are intentionally not claimed by this task:

1. A concrete Build/Web/Console transport executor that performs external OAuth or browser work.
2. Interactive device authorization UI/CLI and operator authentication policy.
3. Live reauth against any external account pool.
4. Wiring the coordinator into a deployed service or enabling it in production.
