# ADR-0097: Provider-specific egress, health, and recovery isolation

Status: **Accepted locally — P13-11E E0/E1/E2/E3 LOCAL_PASS_PENDING_PHASE_GATE**

Date: 2026-08-17

## Context

P13-11A/B/C/D provide a safe generic compatible-endpoint profile, one Upstream-owned transport
registry, exact serving Credential+egress leases, Config-Version-owned proxy resources, and
protected management operations. They intentionally do not decide how native Grok Build, Console,
or Web sessions classify failures or recover an egress.

Those channels are not interchangeable. Build may use a native account pool; Console may perform
DPoP/bootstrap requests; Web may bind a browser session and clearance to a sticky egress. Official
API-key, Codex/ChatGPT OAuth, Claude-compatible, Kiro, and arbitrary `base_url + api_key` relays
have different capabilities. Treating all 403s, retries, proxy nodes, or sessions as one global
health value would create silent cross-channel fallback and make an observed account failure look
like an egress failure.

## Decision

P13-11E freezes a Provider-aware state model above the existing P13-11D generic transport seam:

1. **Separate state domains.** Credential/account, quota, endpoint/channel, egress node, provider
   session, clearance, and protocol failure remain independently addressable. A projection may
   compute a precedence for display, but it must retain the source domain and exact identity.
2. **Explicit Provider capabilities.** Each adapter declares whether it supports direct/fixed/pool
   egress, sticky identity, session/clearance, bounded challenge recovery, auxiliary HTTP, and
   one-shot diagnostics. An undeclared capability fails closed; it never inherits Grok Web behavior.
3. **Provider-local ownership.** Generic compatible endpoints use the existing exact Upstream,
   Endpoint-Credential, and egress profile. Native Grok Build/Console/Web use distinct channel
   namespaces. Kiro, Official API-key, Claude-compatible, Codex/ChatGPT, and custom relays cannot
   borrow Grok state or credentials. “Krill” is only a generic endpoint instance.
4. **Recovery is bounded and pre-submit.** A recovery may run only inside the same explicit
   Provider/Channel scope, before a semantic event, with a deterministic attempt bound. Hidden
   adapter requests count toward that bound. Unknown 403 evidence is ambiguous and cannot directly
   mark an account forbidden or trigger a generic retry.
5. **Autoreg remains external.** CPAR may import a new externally delivered credential revision and
   update its own pool state. It does not register accounts, drive OAuth/SSO, refresh tokens, read
   Autoreg storage, or run an Autoreg scheduler.
6. **No network in the planning slice.** E0/E1/E2/E3 use fake transports, loopback, synthetic
   metadata, and deterministic clocks. Real Provider/proxy/DNS/FlareSolverr tests are a separately
   authorized E5 canary and are not evidence for this ADR.

### Channel policy summary

| Channel | Egress/session rule | Recovery rule |
|---|---|---|
| Generic compatible | P13-11D exact Upstream egress; no browser state | same-binding pre-submit only |
| Grok Build | Build-local egress and native account state | egress-only local recovery; credential replacement remains external |
| Grok Console | Console-local egress plus session/DPoP state | capability-gated; auxiliary requests counted |
| Grok Web | sticky browser egress + session + clearance | one bounded challenge recovery only when explicitly supported |
| Official/Kiro/Claude/other | adapter-declared profile, otherwise fail closed | no inherited Grok behavior |

## Alternatives rejected

- **One global egress health enum:** rejected because credential, egress, session and quota have
  different owners and recovery actions.
- **Provider name or credential format selects a proxy:** rejected; configuration and explicit
  binding select egress, not `Krill`, CPA JSON, Sub2API JSON, or OAuth labels.
- **Reuse Web clearance for Console/Build:** rejected; session and browser proof are channel-local.
- **Let Autoreg repair CPAR runtime state:** rejected; Autoreg and CPAR are independent projects and
  exchange only controlled credential envelopes.
- **Run a real probe as part of implementation:** rejected; external egress evidence needs a new
  target, budget, receipt, and rollback boundary.

## Consequences

The E1 implementation adds typed state and fake recovery boundaries without changing public
protocols or the existing generic management contract. E2 wires that seam into the native Grok
Build and Console adapters using the existing CPAR exact lease and transport path. Build records its
submission only after egress admission/request construction; Console counts DPoP bootstrap as
auxiliary traffic and suppresses the legacy second inference when an E2 attempt is active. Both
channels keep independent Provider IDs, credential kinds, session state and bounded ledgers.

E3 adds a transport-free Grok Web attempt seam. It requires an exact `grok_web_sso` lease, named
sticky egress, active Provider-session lineage and exact clearance lineage. Clearance recovery uses
an atomic owner ticket with a runtime-private generation, so concurrent requests and stale
same-deadline tickets cannot overwrite or complete another owner. Unknown `403` remains ambiguous,
confirmed account evidence remains Credential-owned, and only explicit sanitized clearance
challenge evidence marks the exact lineage for a later pre-inference recovery. The current logical
attempt never sends a second inference.

`gateway-router` owns only the capability and value-free state seam; existing generic transport,
Credential Health, and Quota owners remain authoritative for their domains. Management UI and
OpenAPI stay unchanged until a later E4 decision. A Provider-specific adapter may report
`recovery_in_flight` or `challenge_required`, but it cannot silently rotate to another Provider or
mutate an Autoreg account. A real network result remains an explicit, separately attributable
evidence item.

The E1 local receipt is
[p13-11e-provider-specific-egress-state.md](../reports/p13-11e-provider-specific-egress-state.md):
gateway-router all-target tests `158/158` and strict Clippy passed. The E2 local receipt is
[p13-11e-native-adapter-seam.md](../reports/p13-11e-native-adapter-seam.md): provider-grok's
Build/Console fixture is `4/4`, gateway remains `109/109`, and the synthetic transport has zero
Provider/DNS/Store/proxy calls. The E3 local receipt is
[p13-11e-web-egress-seam.md](../reports/p13-11e-web-egress-seam.md): the Web fixture is `11/11`,
the exact-clearance state fixture is `13/13`, and the new seam has no Provider/DNS/proxy/Store or
FlareSolverr dependency.

## Acceptance boundary

E0-E3 are locally accepted when the CR, this ADR, BC-SEC-008, the E0/E1/E2/E3 reports, plan Task
Card, and traceability row agree on the channel matrix, state ownership, no-fallback rule, exact
lease handoff, bounded Console/Web auxiliary accounting, atomic clearance ownership, and
no-network boundary. This local acceptance does not claim code behavior outside the covered
adapters, Provider, proxy, DNS, staging, production, or account success. The next step is the
aggregate E0-E3 local review and phase-closeout decision. Optional E4 management projection and any
E5 canary remain separate acceptance boundaries and do not start automatically.
