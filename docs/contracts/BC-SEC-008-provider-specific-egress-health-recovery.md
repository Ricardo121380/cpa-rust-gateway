# BC-SEC-008 Provider-specific egress, health, and recovery isolation

| Field | Value |
|---|---|
| Contract | `BC-SEC-008` |
| Task | `P13-11E` (`E0` planning/review; `E1-E3` implementation slices) |
| ADR | [ADR-0097](../adr/ADR-0097-provider-specific-egress-health-recovery.md) |
| Status | **PLANNED_REVIEWED — implementation not started** |
| Domain | Provider/Channel-local egress, account health, session/clearance and bounded recovery |

## 1. Contract invariants

1. A credential source label (`api_key`, CPA JSON, Sub2API JSON, direct OAuth, custom relay) is
   metadata. It never selects a hidden proxy, browser profile, clearance cache, or fallback.
2. Generic compatible endpoints, Grok Build, Grok Console, Grok Web, Official API-key, Kiro,
   Claude-compatible and other adapters have explicit Provider/Channel ownership. “Krill” is a
   generic endpoint instance, not a global Provider state namespace.
3. Credential/account, quota, endpoint/channel, egress node, session, clearance and protocol state
   are separate. A safe projection may include a precedence, but must preserve the source domain.
4. No state or lease may be read, mutated, or reused across Upstream/Provider/Channel unless an
   explicit adapter capability and exact identity prove ownership.
5. A recovery is at most pre-submit, bounded, deterministic and scoped to one exact Provider/Channel
   binding. After the first semantic event, no transport replay or hidden egress recovery is allowed.
6. E0-E3 do not contact a Provider, proxy, DNS resolver, FlareSolverr, server, staging or production.
   A real canary is E5 and requires a new CR with target, budget, receipt and rollback.

## 2. State domains

| Domain | Required key material | Example states | May mutate |
|---|---|---|---|
| Credential runtime | Config Version + Upstream/Endpoint + Credential revision | available, cooling, quota_blocked, unauthorized, expired, disabled | CPAR pool/lease/failure feedback |
| Egress runtime | Config Version + owning Upstream + exact node/profile | available, cooling, circuit_open, probe_due, probe_in_flight, disabled | same Upstream egress registry |
| Provider session | Provider/Channel + account/revision + session lineage | absent, active, expired, challenge_required, invalid | only that adapter capability |
| Clearance | Provider/Channel + account/session/egress lineage | absent, fresh, expired, refresh_required, refresh_in_flight, invalid | Web/declared adapter only |
| Quota | Endpoint + Credential + Model (and Provider-owned scope where applicable) | available, exhausted, recovery_required, probe_in_flight | existing runtime quota registry |

An implementation must not collapse these domains into one `healthy: bool` or use egress recovery to
repair a credential/session defect.

## 3. Channel requirements

| Channel | Must use | Must not use |
|---|---|---|
| Generic compatible | P13-11D exact egress profile and existing Credential/Quota/egress lease | Grok browser/clearance/Statsig/DPoP state |
| Grok Build | Build-local native account and egress state | Console/Web session or Autoreg database/worker |
| Grok Console | Console-local session/DPoP/bootstrap accounting and egress state | Web clearance, Build OAuth, generic cross-channel fallback |
| Grok Web | sticky browser egress, session and clearance state with exact account lineage | unknown-403 account mutation, Console/Build fallback, unbounded proxy rotation |
| Official API-key | generic direct/fixed/pool egress | browser-specific recovery |
| Kiro/Claude/other | declared adapter profile | inherited Grok state or credential |

For Console and Web, auxiliary HTTP calls are part of the adapter's bounded transport ledger. A
single logical inference attempt cannot claim “one request” while secretly issuing unbounded token,
Statsig, clearance or refresh calls.

## 4. Failure and recovery matrix

| Failure evidence | Owner | Default action |
|---|---|---|
| DNS/TLS/connect/proxy handshake/pre-submit egress rejection | Egress | exact node/profile cooldown or circuit; no credential mutation |
| 401/credential parse/expiry/explicit unauthorized | Credential/session | fail closed for exact revision; external replacement or declared adapter action |
| 429/quota/rate-limit | Quota | exact model/credential cooldown or recovery probe; no cross-provider fallback |
| 403 without confirmed account evidence | Ambiguous Provider/egress | retain classification; no direct account ban and no generic retry |
| confirmed account forbidden | Credential/account | exact account revision blocked; sibling egress remains independent |
| protocol/decode/lifecycle failure | Adapter/protocol | fail request; no egress recovery |
| post-semantic failure | Request/Provider outcome | no replay, no hidden recovery |

## 5. Security and observability

- Management/read projections and errors are value-free: no URL, API key, OAuth/SSO token, Cookie,
  proxy credential, DNS answer, request body, raw Provider response or client-key digest.
- Identity fields are bounded opaque IDs. Debug and audit retain only domain, exact safe identity,
  state transition, bounded reason and revision; they do not retain body/header/cookie material.
- Sticky egress is fail-closed when its exact node is unavailable; it does not silently rotate to
  another Provider or channel.
- E1-E3 use fake transport and deterministic clocks. Any injected network implementation must be
  rejecting by default and must prove zero Provider/DNS/proxy calls in the local test fixture.

## 6. Required tests before implementation slice can close

- generic, Build, Console, Web, Official and other-adapter state namespaces cannot cross-mutate;
- credential, quota, egress, session and clearance failures map to distinct owners;
- unknown 403 does not mark an account forbidden; confirmed account evidence does not poison egress;
- pre-submit recovery is bounded and hidden Console/Web auxiliary calls are counted;
- semantic event prevents replay/recovery; sticky-node loss fails closed;
- disabled/expired/unauthorized rows remain excluded from lease selection;
- fake transport, Store, DNS and Provider call counters remain zero for E0-E3 planning fixtures;
- no public protocol/OpenAPI/Prism change is introduced before a separately reviewed E4 CR.

## 7. Explicit non-evidence

This contract is not evidence that any Grok Build/Console/Web account, proxy node, clearance, SSO,
official key, Kiro credential, relay, server, DNS route or public CPAR endpoint is currently usable.
It is also not an Autoreg registration/refresh/replenishment contract.
