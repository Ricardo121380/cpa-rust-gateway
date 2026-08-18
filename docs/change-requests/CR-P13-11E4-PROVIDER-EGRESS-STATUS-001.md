# CR-P13-11E4-PROVIDER-EGRESS-STATUS-001

## 1. Status and authority

| Field | Value |
|---|---|
| Status | **Formally accepted — `DONE_WITH_BOUNDARY`**; immutable tag and single formal phase Gate passed |
| Task | `P13-11E4` Provider-specific egress runtime status projection |
| Predecessor | `P13-11E` E0-E3, formally closed by immutable tag `phase-p13-provider-egress-complete` |
| Delivery class | Read-only management contract and local synthetic/runtime-composition evidence |
| Not authorized | Provider/proxy/DNS traffic, recovery actions, server/staging/production mutation, Autoreg, or E5 canary |

This CR is a successor to
[CR-P13-11-PROVIDER-SPECIFIC-EGRESS-001](CR-P13-11-PROVIDER-SPECIFIC-EGRESS-001.md).
It does not move, amend, or reinterpret the E0-E3 tag, closeout commit, Gate, or evidence.

## 2. Problem

P13-11E E1-E3 introduced three independent, value-free runtime domains:

- exact Provider/Upstream/Endpoint-owned egress state;
- exact Credential-revision-owned Provider session state;
- exact session/egress-owned clearance state.

The current management API exposes static Config Version bindings and a Provider account-pool
summary, but it does not expose those three exact runtime domains. Extending the existing
`runtime_status` summary would destroy failure ownership: an egress circuit, an expired session,
and a clearance refresh cannot be represented as the same fact.

## 3. Approved scope

P13-11E4 may add one protected, read-only management projection with these properties:

1. A single atomic snapshot retains exact `provider_id`, `upstream_id`, `channel_id` (the exact
   configured Endpoint ID), closed channel kind, source domain, and the domain-specific opaque
   identities and revisions.
2. Egress, session, and clearance remain separate rows and closed state machines. There is no
   synthesized overall health field.
3. The snapshot binds both the active Config Version lineage and an independent runtime-state
   revision. Config revision and runtime revision cannot substitute for each other.
4. Keyset pagination binds Config lineage, runtime snapshot, observation time, filter fingerprint,
   and the last stable key. A still-retained immutable snapshot remains valid across live runtime
   advance; an unknown/expired snapshot or bound-lineage mismatch fails with a safe conflict rather
   than mixing pages.
5. The projection is value-free and `Cache-Control: no-store`.
6. The HTTP layer consumes an injected facade. It does not contact a Provider, decrypt a
   credential, open a proxy, resolve DNS, acquire a serving lease, or start recovery.

E4B has delivered `GET /admin/operations/provider-egress-status`, updated the authoritative OpenAPI,
synchronized the Prism contract/generated client, and recorded the Claude Code action-required
handoff. The earlier planning/governance slice itself did not change those surfaces.

## 4. Initial source boundary

The first application source may expose only the native Grok Build and Grok Console runtime state
that is already present in the composed process. That is acceptable and must be explicit:

- an empty Web/clearance result is a truthful empty projection, not proof of healthy Web;
- Web and clearance rows appear only after an exact Web runtime source is wired and reviewed;
- generic compatible endpoint egress has its own P13-11B/D runtime owner and must not be silently
  merged into the native Provider source;
- Official, Codex/ChatGPT, Kiro, Claude-compatible, and other channels remain empty unless an exact
  Provider-local source is explicitly injected;
- source absence returns a safe unavailable result or an empty declared scope according to the
  frozen facade contract; it never fabricates `available`.

This source boundary is composition evidence only. It is not evidence that a Build or Console
account, Provider endpoint, proxy, or production route is usable.

## 5. Closed runtime projection

| Domain | Closed states | Time-derived transition used only for display |
|---|---|---|
| Egress | `available`, `cooling_down`, `circuit_open`, `probe_due`, `probe_in_flight`, `disabled` | cooling deadline -> available; circuit/probe deadline -> probe due |
| Session | `absent`, `active`, `expired`, `challenge_required`, `invalid` | active expiry -> expired |
| Clearance | `absent`, `fresh`, `expired`, `refresh_required`, `refresh_in_flight`, `invalid` | fresh expiry -> expired; refresh-owner expiry -> refresh required |

The projection evaluates the effective state at the snapshot's fixed `sampled_at_ms`. Reading the
projection does not perform any transition, reclaim a ticket, mutate a circuit, or start a probe,
session rebuild, or clearance refresh.

## 6. Security and management admission

- Reuse the existing Management Key, peer/origin admission, selected `X-Config-Version`, safe
  errors, and no-store response behavior.
- Because E4 is GET-only, it does not invent a CSRF requirement for non-browser reads. Existing
  BC-MGMT-003 browser admission remains authoritative; no unsafe method is added.
- Never return endpoint/proxy URLs, DNS answers, API keys, OAuth/SSO material, Cookies, Headers,
  request/response bodies, ciphertext/plaintext, client-key digests, raw Provider errors, refresh
  tickets, or ownership generations.
- Named egress targets are bounded opaque IDs only. They are not proxy endpoints.
- Unknown channel capability, unsupported session/clearance domain, poisoned source, invalid
  lineage, duplicate identity, or over-limit state fails closed.

## 7. Implementation slices

| Slice | Output | Gate boundary |
|---|---|---|
| E4A | atomic typed snapshot/revision, domain rows, validation, filtering and cursor semantics | focused Rust tests only |
| E4B | protected GET handler, authoritative OpenAPI, Prism sync/generated client and Claude Code handoff | focused HTTP/contract/client tests |
| E4 closeout | independent review, aggregate local Full and one formal Delivery Gate if separately authorized by the plan | no repeated expensive Gate per slice |

## 8. Acceptance matrix

| Area | Required evidence |
|---|---|
| Snapshot | one-lock/one-revision snapshot; fixed observation time; stable order; bounded rows; stale runtime/config/filter cursor conflict |
| Isolation | same opaque IDs in different Provider/Upstream/Endpoint/revision domains never merge; unsupported session/clearance fails closed |
| HTTP | protected admission, exact Config lineage, strict query decoding, bounded pagination, safe `400/409/503`, no-store response |
| Redaction | serialized response, error, Debug and audit surfaces contain no secret, endpoint, proxy, body, raw response, DNS answer, ticket or generation |
| Regression | static account inventory, Provider account pools/actions/failures, generic compatible runtime and public data plane remain unchanged |
| Contract | authoritative OpenAPI first, Prism sync, generated-client freshness, protected-operation regression, precise Claude Code handoff |
| Side effects | zero Provider, proxy, DNS, recovery, Store decrypt, serving lease, Autoreg, server, staging or production action |

## 9. Explicit non-goals

- No operator POST/action, automated probe, refresh, reauth, replenishment, quota reset, or
  clearance recovery.
- No Build/Console fixed-proxy or pool transport wiring.
- No Web production transport, Statsig, FlareSolverr, or clearance executor.
- No generic-compatible/native source union without an explicit source owner and exact lineage.
- No E5 real-network canary and no statement of real account, network, or production usability.

## 10. Rollback

The default facade remains rejecting or explicitly empty for an unwired declared source. Removing
the E4 HTTP route/facade restores the prior management surface without changing serving state,
durable Config Version data, credentials, or the immutable E0-E3 evidence.

## 11. Local implementation result

E4A/E4B focused implementation gates passed: gateway-router `170/170`, gateway-control `87/87`,
gateway bin `114/114`, E4 HTTP `4/4`, management OpenAPI `12/12`, existing management runtime `3/3`,
and the provider-grok full suite with its authorized/live tests intentionally ignored. Strict
Clippy passed for gateway-control, gateway-router, gateway-http-actix, gateway and provider-grok;
fmt and Prism check passed.

The implementation calls the exact configured `EndpointId` `channel_id`; it validates one
`channel_kind` per exact identity without duplicating that kind in the cursor key. IDs are
byte-bounded and the opaque cursor is bounded to 4096 characters. Provider-session rows are limited
to Console/Web, clearance rows to Web, and Web targets are named. A live `refresh_in_flight` row
requires its exact atomic owner, while generation/ticket values remain hidden. Invalid source shape
fails closed with safe `503`.

Current process composition projects Build/Console state only. Production Web/clearance may be
empty; synthetic HTTP clearance is serializer-contract evidence only; generic-compatible state
remains under a separate owner. Independent final review passed with no remaining P0-P3 finding;
aggregate local Full passed `43/43`. Immutable tag `phase-p13-provider-egress-status-complete`
points to exact closeout commit `ce98faa9306d076f5af53b9eef0c818abb1cb9c8`; formal Delivery Gate
[32110872875](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/32110872875) passed all
required jobs. The status is `DONE_WITH_BOUNDARY`. No Provider/proxy/DNS/server/production/Autoreg
or real-health claim is made. E5 remains `DEFERRED_UNAUTHORIZED`.
