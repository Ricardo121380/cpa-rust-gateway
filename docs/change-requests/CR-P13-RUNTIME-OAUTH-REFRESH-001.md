# CR-P13-RUNTIME-OAUTH-REFRESH-001 · CPAR-owned refresh for imported OAuth credentials

## 1. Status and authority

| Field | Value |
|---|---|
| Status | **APPROVED / IN_PROGRESS — P13-16A local implementation passed; production verification pending** |
| CPAR task | `P13-16` |
| Authority | User correction on 2026-09-02: refreshable credentials saved in CPAR must refresh automatically inside CPAR |
| Supersedes | Only the runtime-refresh wording in `CR-P12-AUTOREG-SEPARATION-001/002`; project separation and registration ownership remain unchanged |
| Initial delivery | Active Grok Build and official Codex OAuth channels |

## 2. Corrected ownership boundary

CPAR and Autoreg remain completely independent projects. The earlier documents drew the refresh
line too broadly. The corrected boundary is:

- **CPAR owns** proactive refresh of an already-imported OAuth grant when CPAR stores the refresh
  token, the exact Provider/channel refresh protocol is implemented, and the Credential is bound
  to the active CPAR graph. CPAR also owns encrypted CAS persistence, runtime-pool replacement,
  expiry exclusion, backoff/reauth state, audit and restart catch-up.
- **Autoreg owns** account registration, browser login, initial OAuth/SSO authorization, account
  replenishment and interactive recovery after the Provider revokes or invalidates the saved
  refresh grant. It does not have to be online for ordinary CPAR token renewal.
- **Static API keys do not refresh.** CPAR can monitor, cool down, disable or rotate them only after
  an explicit replacement value is supplied.
- **SSO cookies are not OAuth refresh tokens.** Grok Console keeps its independent short-lived DPoP
  session exchange/cache, while Grok Web keeps its independent session/clearance recovery domain.
  A Build refresh worker must never claim Console or Web work.

An `invalid_grant`, revoked refresh token or account-entitlement failure therefore becomes a
redacted `reauth_required`/unavailable state. CPAR must stop leasing that Credential; Autoreg or an
operator may later provide a fresh authorization package. This handoff is not a runtime dependency
between the two services.

## 3. P13-16A implementation contract

The first production slice covers every refreshable OAuth Credential in the current active Oracle
CPAR graph:

1. On process startup, run one bounded catch-up pass before compiling the serving Credential pools.
2. While the listeners are running, execute a one-minute bounded refresh cadence off the HTTP
   worker threads.
3. Grok Build claims only `provider=build` native refresh jobs. Existing worker claim leases,
   revision CAS, exponential failure state and encrypted durable replacement remain authoritative.
4. Official Codex selects only active `oauth_json` Credentials bound to the exact official Codex
   endpoint shape. Generic compatible OAuth-shaped data is not silently treated as Codex.
5. Refresh before expiry, persist the complete normalized envelope through the active-graph
   Credential CAS/audit path, then atomically replace only the matching in-memory material.
6. A request already holding a Credential lease pins the old revision and secret until completion;
   only later leases observe the replacement.
7. Logs and reports expose counts and safe failure classes only. Tokens, account identities, URLs
   with query data and Provider response bodies are never emitted.
8. Listener shutdown aborts and joins the periodic owner. There is one refresh owner per running
   gateway process and no request-path refresh.

## 4. Follow-on coverage

P13-16 is not complete merely because the current production graph contains only Build and Codex.
Before enabling another refreshable channel, the same exact-channel worker contract must be
composed for that channel:

- Claude OAuth: use the existing Claude refresh transaction only when an exact active Claude
  endpoint and its configured egress are present;
- Kiro Social/IdC OAuth: use the existing credential-type-specific refresh and region/profile
  rules; P7 remains deferred, so production activation is not part of P13-16A;
- future OAuth Providers: add an explicit adapter; never infer refresh behavior from
  `kind=oauth_json`, display name or a compatible JSON shape.

P13-16 reaches `DONE_WITH_BOUNDARY` only after active-channel tests, production restart/catch-up,
post-expiry continuity evidence, bilingual operator documentation and the formal Delivery Gate.

## 5. Rollback

Before production installation, retain both the prior binary symlink and a verified SQLite
preimage. The new worker can rotate encrypted Credential bytes before listener bind; therefore a
rollback after any startup mutation must restore the matching database preimage as well as the old
binary. Never copy decrypted credentials into rollback receipts or shell history.
