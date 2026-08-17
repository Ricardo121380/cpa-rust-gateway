# BC-SEC-008 Provider-specific egress, health, and recovery isolation

| Field | Value |
|---|---|
| Contract | `BC-SEC-008` |
| Task | `P13-11E` (`E0` planning/review; `E1-E3` implementation slices) |
| ADR | [ADR-0097](../adr/ADR-0097-provider-specific-egress-health-recovery.md) |
| Status | **READY_FOR_FORMAL_DELIVERY_GATE — E0-E3 complete locally; formal Gate pending** |
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

## 5A. E1 implementation evidence

`crates/gateway-router/src/provider_egress_state.rs` now provides the typed E1 seam without creating
another Credential/Quota owner:

- exact Provider/Upstream/Endpoint capability registration for Generic, Grok Build/Console/Web,
  Official API, Codex/ChatGPT, Kiro, Claude-compatible, and other compatible adapters;
- independent Egress, Provider-session, and Clearance keys/states with explicit deterministic
  expiry and fail-closed unknown-state behavior;
- capability-gated sticky egress (Web rejects Direct), bounded pre-submit recovery, and a transport
  ledger that counts hidden auxiliary calls before one inference submission;
- value-free failure ownership for Egress, Credential, Quota, ambiguous 403, Session, Clearance,
  Adapter/Protocol, and post-semantic outcome.

The E1 local evidence is recorded in
[the E1 report](../reports/p13-11e-provider-specific-egress-state.md): gateway-router all-target
tests `158/158`, strict Clippy, format, and diff checks passed. The synthetic fixture records zero
Provider, DNS, Store, and proxy calls. No public API/OpenAPI/Prism surface changed, so no frontend
handoff is required for E1.

## 5B. E2 native Build/Console adapter evidence

The E2 local evidence is recorded in
[the E2 report](../reports/p13-11e-native-adapter-seam.md):

- the existing CPAR native account-pool compilation and exact `CredentialLease` remain the only
  selection/lease owner; the new seam only validates the exact Build or Console channel before
  adapter execution;
- Build and Console use distinct fixed Provider IDs, credential kinds, egress keys and (for
  Console) session keys; no state or credential can cross the two namespaces;
- Build records the single inference submission after egress admission and request construction;
  Console records DPoP/bootstrap as bounded auxiliary traffic, marks a failed session as
  `challenge_required`, and suppresses the legacy refresh/second inference while an E2 attempt is
  active;
- the E2 fixture covers exact lease handoff, one synthetic inference, Console bootstrap accounting,
  unknown/confirmed `403` ownership and cross-channel rejection (`4/4`); provider-grok, router and
  gateway regressions remain green (`158/158` router, `109/109` gateway);
- no Provider, DNS, proxy, Store, Autoreg, server, staging or production call was made, and no
  OpenAPI/Prism/frontend surface changed.

E2 is local implementation evidence only. A real Provider/network canary remains E5 and requires
a new CR.

## 5C. E3 Grok Web attempt and atomic clearance evidence

The E3 local evidence is recorded in
[the E3 report](../reports/p13-11e-web-egress-seam.md):

- one transport-free `GrokWebProviderEgressAttempt` binds the fixed `grok.web` namespace, exact
  Upstream/Endpoint, an already-owned `grok_web_sso` Credential lease and revision, named sticky
  egress, active Provider session, and exact clearance lineage;
- clearance challenge marking, refresh begin, completion and failure use one atomic runtime state
  owner ticket. A runtime-private generation prevents a stale or same-deadline ticket from
  completing a replacement owner, and a live refresh cannot be overwritten by another challenge;
- Statsig environment/signer submissions, one clearance refresh and the sole inference are
  separately counted by the bounded Web ledger. The fifth auxiliary request, second recovery,
  second inference and any operation after semantic output fail closed;
- unknown `403` is `AmbiguousProvider/None` and changes no account/egress/session/clearance state;
  confirmed forbidden is Credential-owned and does not poison egress/clearance; only explicit
  `ClearanceChallenge` marks the exact clearance `RefreshRequired` for a later logical attempt;
- Direct egress, foreign Provider/Channel/Credential/session/target, blocked exact egress, inactive
  session and invalid clearance all stop before accounting or transport; and
- the fixture is deterministic and transport-free (`11/11` Web scenarios, `13/13` exact-clearance
  state scenarios). It does not contact Provider, DNS, proxy, Store, Autoreg or FlareSolverr.

E3 does not attach this seam to the legacy production Web adapter because the current process-level
proxy envelope does not expose a stable, non-secret egress node/profile identity. It also does not
infer a clearance challenge from an arbitrary `403` body. Physical proxy/Statsig/FlareSolverr
accounting and real response evidence require a later reviewed configuration/network CR.

## 6. Aggregate acceptance checklist

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

The aggregate local Full passed `43/43`; the independent
[phase review](../reports/evidence/p13-11e-phase-review-20260817.md) found no remaining P1/P2
blocker. E0-E3 remain pending the explicitly authorized immutable
`phase-p13-provider-egress-complete` formal Gate. Build/Console native fixed/pool egress and the
production Web transport are not included. E4 is `DEFERRED_OPTIONAL`, and E5 remains unauthorized.
