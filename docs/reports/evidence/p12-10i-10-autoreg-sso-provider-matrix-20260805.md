# P12-10I-10 Autoreg SSO provider matrix

Status: `BLOCKED_WITH_EVIDENCE`

## Scope

Exercise the Autoreg task-55 SSO artifact through CPAR's Build, Console, and Web provider
boundaries. The source values were kept in the remote in-memory/controlled-pipe path only. This
receipt records no SSO value, OAuth value, cookie value, account identity, endpoint, model, request
body, response body, or token fingerprint.

The active CPAR graph was not changed. The production native pool and its existing Build account
were not used as a public route.

## Source-side evidence

- The Autoreg task contains an SSO artifact and an authentication-session artifact with a bounded
  cookie set. Cookie scopes, secure/http-only flags, and duplicate-scope checks passed without
  exposing cookie values.
- The task source contains an SSO-to-xAI OAuth conversion path and a separate CPA-compatible JSON
  export path. The conversion was invoked once through a controlled remote pipe and returned a
  valid Build OAuth shape.
- The generated credential was never written to the repository, a receipt, shell history, or a
  production configuration file.

## Results

### Build: conversion and import passed; the first probe exposed a CPAR probe bug

- SSO-to-Build conversion: `PASS`.
- CPAR native Build import: `PASS` (`accepted_accounts=1`, encrypted store transaction).
- The first native probe returned `ProbeRejected` before any upstream request.

The rejection was caused by the probe implementation, not by an upstream response. The command
selected one account by import batch, then compiled the entire enabled Build pool and required the
compiled pool to contain exactly one account. A staging copy can contain other Build accounts, so
that cardinality check rejected a valid batch-scoped target before leasing it.

The probe now leases only the selected batch account with a non-secret Credential-ID eligibility
predicate. This keeps normal runtime scheduling unchanged and makes `--batch` mean the account it
names, even when other accounts are present in the pool. A rebuilt artifact still needs one fresh
isolated native probe before this path can be called externally verified.

### Console: shape accepted; upstream egress rejected

- The same SSO was wrapped as the independent Console SSO envelope and imported into a fresh
  isolated store: `PASS`.
- The native Console probe reached provider start and stopped with the fixed category
  `EgressRejected/Egress`.
- No public CPAR request was sent; the active graph has no Grok route.

This is an external egress/provider boundary, not a proof that the SSO envelope is malformed.

### Web: shape rejected by the explicit lifetime policy

- The Autoreg cookie `expires` values were normalized from Unix seconds to Unix milliseconds.
- Cookie shape, secure/http-only flags, scope consistency, and deduplication passed.
- The earliest normalized expiry was roughly six months out, beyond CPAR's strict 90-day Web
  session lifetime. The importer rejected the record before the account transaction; no Web
  request was sent.

The expiry was not shortened or clamped. Raising the bound or accepting a conservative capped
session requires a separate security-reviewed change request.

## Local implementation and validation

The probe fix is limited to `apps/gateway/src/grok_admin.rs`: remove the whole-pool cardinality
assumption and use `try_lease_eligible` for the selected account. The following value-free checks
passed locally:

```text
cargo fmt --all -- --check
cargo test --locked -p gateway
cargo test --locked -p gateway-upstream credential_pool
cargo clippy --locked -p gateway -- -D warnings
```

The existing pool test confirms that an eligibility predicate selects one Credential without
changing the pool scope or lease accounting.

## Boundary and next action

- No production listener, active Config Version, route, public traffic, old CPA, grok2api, Caddy,
  DNS, or CC Switch state changed.
- Rebuild the corrected artifact and repeat one isolated SSO-to-Build native probe. Only after that
  passes should a separately approved staging route use CPAR's real base URL and client key.
- Console still needs an egress/provider fix and a new isolated probe.
- Web needs an explicit policy decision for the greater-than-90-day source session; silently
  extending the bound is not allowed.

Until those actions complete, P12-10I-10 remains `BLOCKED_WITH_EVIDENCE`; the Build fix is local
and reviewed but is not a production readiness claim.

## Follow-up

[P12-10I-11](p12-10i-11-autoreg-sso-build-exact-artifact-receipt-20260805.md) subsequently rebuilt
the fix as an exact signed ARM artifact and passed the required two-account, SSO-derived native
Build probe with full rollback. The P12-10I-10 status remains `BLOCKED_WITH_EVIDENCE` only because
its independent Console egress and Web lifetime-policy results are still unresolved.
