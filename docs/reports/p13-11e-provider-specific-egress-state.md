# P13-11E E1 provider-specific egress state report

Status: `DONE_WITH_BOUNDARY`

Date: 2026-08-17

## Outcome first

P13-11E E1 adds a pure local Provider-aware capability/state seam in
`crates/gateway-router/src/provider_egress_state.rs`. The seam keeps the existing generic
P13-11D transport registry and Credential/Quota registries as separate owners. It does not select a
proxy, read Store, resolve DNS, contact a Provider, refresh a credential, or change a public or
management protocol.

## Implemented boundary

- Nine explicit channel families are represented: generic compatible, Grok Build, Grok Console,
  Grok Web, Official API, Codex/ChatGPT, Kiro, Claude-compatible, and other compatible.
- Capabilities are registered by exact `ProviderId + UpstreamId + EndpointId`; duplicate or
  malformed/overlong identities fail closed. Channel defaults declare sticky-egress, Provider
  session, clearance, auxiliary-request, and pre-submit-recovery behavior.
- Egress, Provider-session, and clearance state use independent exact keys. Deadline transitions are
  deterministic at a caller-supplied millisecond clock: cooling/circuit/probe tickets and active
  session/clearance states expire without a background task or implicit sibling rotation.
- A Web capability rejects a direct target because its sticky egress contract is explicit. An
  unavailable exact target returns a closed error; it cannot silently rotate to another channel,
  Provider, account, or pool.
- Sanitized failure evidence preserves ownership across Egress, Credential, Quota, ambiguous 403,
  Session, Clearance, Adapter/Protocol, and post-semantic Request outcome. Unknown 403 evidence has
  no account-ban or generic-retry action; confirmed account evidence is Credential-local.
- `ProviderTransportAttemptBudget` counts hidden auxiliary submissions, bounded pre-submit recovery,
  the single inference submission, and semantic-event closure. Generic/Build/ordinary channels do
  not inherit Console/Web auxiliary behavior; Console/Web budgets are finite and explicit.
- Debug output is value-free and tests use a rejecting synthetic fixture whose Provider, DNS, Store,
  and proxy counters remain zero.

## Verification

- `cargo test --locked -p gateway-router --all-targets`: **158 passed**.
- `cargo clippy --locked -p gateway-router --all-targets --all-features -- -D warnings`: **passed**.
- `cargo fmt --all`: **passed**.
- `git diff --check`: **passed**.
- The focused E1 module contains **7** state/capability/budget tests, including namespace
  isolation, deterministic expiry, sticky-loss fail-closed, unknown/confirmed 403 attribution,
  hidden auxiliary bounds, and zero network/Store counters.

## Review conclusion

The first review found and corrected three boundary issues before this receipt was written:

1. Web's required sticky egress now rejects a `Direct` target instead of treating it as a valid
   browser egress.
2. Generic compatible and Grok Build receive one explicit pre-submit recovery budget, while
   ordinary/official adapters remain conservative and Console/Web auxiliary requests remain capped.
3. Capability failure classification is exposed through the exact capability object, and all
   identity/state debug paths remain bounded and value-free.

No OpenAPI, Prism, frontend, server, staging, production, Provider, proxy, DNS, FlareSolverr,
Autoreg, OAuth, or real account evidence was used. The four pre-existing untracked helper files
remain untouched and unstaged.

## Deferred next slice

E2 may wire the seam into synthetic/loopback Grok Build and Console adapter fixtures, reusing CPAR's
existing imported account pool and exact lease. It must keep Console DPoP/bootstrap calls inside the
finite transport ledger and must not introduce Autoreg access, Web clearance, or real Provider
traffic. E3 remains the separate fake Web sticky-session/clearance slice; E4 would require a new
management contract, and E5 real network evidence requires a new explicitly authorized CR.

## Formal aggregate closeout

The preceding next-slice wording is retained as this E1 receipt's point-in-time history; E2 and E3
were subsequently completed under their own reports. E1 is now accepted as part of P13-11E E0-E3
`DONE_WITH_BOUNDARY`: immutable annotated tag `phase-p13-provider-egress-complete` points to exact
closeout commit `ba2261a5414fe73d147a102a266abd3e9a7fbb5b`, and formal Delivery Gate run
[32044424886](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/32044424886) passed
Authorize, Fast, Full supply-chain, and Required. The historical E1 test totals above are unchanged.
This closeout adds no Provider/network evidence; E4 remains `DEFERRED_OPTIONAL`, and E5 remains
`DEFERRED_UNAUTHORIZED`.
