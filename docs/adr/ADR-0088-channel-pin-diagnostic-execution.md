# ADR-0088: Protected single-request Channel Pin diagnostic execution

Status: **Accepted — local implementation and review complete; P13 phase gate pending**

## Context

P13-04 through P13-07 provide management read models, Provider-owned pool
observations, operator health actions, and Provider-scoped routing decisions.
Those projections are intentionally read-only and do not prove which exact
route, credential, or transport would be used for one request. When an
operator investigates a route, a broad public request can hide the cause by
selecting a sibling credential, retrying another endpoint, or falling through
to a different Provider.

The gateway therefore needs a narrow diagnostic seam that pins every relevant
serving identity before execution. It must be useful for a value-free receipt
and an audit trail without becoming a second inference API, a general-purpose
test proxy, or an implicit Provider fallback mechanism.

## Decision

Add a protected management-only **Channel Pin** operation. The operator names
the selected Config Version through the existing management admission and
supplies an exact Provider, channel/endpoint, route, credential, public model,
client protocol, and `json` or `sse` mode. The operation creates one bounded
diagnostic request with a fixed, non-sensitive probe payload; arbitrary caller
body material is not part of this first slice.

Execution is available only for the runtime-current active Config Version.
Draft and archived versions remain inspectable through ordinary management
reads but cannot be used to send a Provider diagnostic.

The execution contract is deliberately stricter than ordinary serving. The
first implementation slice (P13-08A) admits only adapters whose transport
boundary is one inference request with no hidden bootstrap HTTP calls:
OpenAI Chat/Responses and generic Anthropic Messages. Native browser/session
adapters that must perform token exchange, Statsig warm-up, or refresh inside
their adapter (currently Grok Console/Web) are rejected before lease/network
and reserved for a follow-up Provider-specific one-shot policy task. This is
an explicit fail-closed capability boundary, not an implicit fallback.

For admitted adapters:

* exactly one candidate and at most one upstream inference send;
* no retry, no quota-recovery fallback, no sibling-credential selection and no
  cross-Provider fallback;
* the exact route and credential must belong to the selected Config Version and
  the route's owning Provider/channel; ambiguity or ownership mismatch fails
  closed before transport;
* the caller's `If-Match` revision is carried into the executor and must equal
  the runtime composition revision at the lease boundary;
* normal management authentication, CSRF (where applicable), selected
  Config-Version, required `If-Match` revision/ETag, egress admission, capability and credential
  eligibility checks remain in force; Channel Pin cannot bypass them;
* the operation is not mounted under `/v1/*`, is not a public user API, and
  does not alter ordinary request retry or routing behavior;
* only Canonical/bridge candidates admitted by the generic OpenAI Chat/Responses
  or Anthropic Messages transport are supported in this slice; `NativeExact`
  candidates fail closed before lease/network;
* a receipt contains only stable identifiers and closed status categories. It
  never contains endpoint URLs, request/response bodies, headers, cookies,
  tokens, credential plaintext/ciphertext, client-key digests, raw upstream
  status text, or provider error payloads;
* the request and pre-execution category are recorded through the existing
  value-free management audit boundary. A failed preflight is still an
  auditable rejected action, and the `channel_pin_started` action is appended
  before any Provider call. The returned receipt is the terminal outcome;
  there is deliberately no post-send audit append that could turn an already
  sent request into an ambiguous retryable error. Exact target attribution and
  `upstream_sent` remain in the returned receipt; the audit schema deliberately
  does not persist a second copy of those fields. The serving Attempt event
  sink is not used for this diagnostic because it runs before bounded source
  drain.

The Channel Pin executor must share the serving composition's route snapshot,
credential pool, lease owner, Health/Quota/Circuit state, egress policy, and
event/attempt stage ledger. It must not construct a second credential store,
second scheduler, hidden proxy pool, or alternate Provider adapter. Provider
capabilities remain Provider-specific; a channel pin for one Provider can
never borrow another Provider's credential or egress.

## Receipt vocabulary

The first contract uses bounded, closed values rather than free-form error
text. The receipt records an opaque request id, selected Provider/channel/route
/credential ids, protocol and mode, `succeeded|failed|rejected` state,
  `upstream_sent` (false for preflight rejection), `attempt_count` (`0` or `1`),
  an optional closed observed stage, whether a semantic response started,
and an observation timestamp. The audit action is persisted separately and no
audit-event identifier is exposed in this first slice. Unknown internal
failures map to a generic safe class.

`json` and `sse` select the requested response projection used by the existing
adapter; a Provider adapter may intentionally force a compatible upstream wire
shape (for example, Codex OAuth Responses uses upstream SSE even for a JSON
management projection). They do not loosen the one-send rule. The diagnostic
drain adds a 45-second idle bound, a 45-second total bound, and a 4096-event
ceiling on top of the adapter's existing byte/frame limits. At most two Channel
Pins may be in flight process-wide; a third is rejected without a lease or
network call. Native adapters with auxiliary requests remain explicitly
unsupported until their own one-shot policy is implemented and tested.

## Alternatives considered

1. **Reuse the public inference route with query parameters.** Rejected: it
   would expose operator controls to users and make exact credential
   attribution/retry semantics ambiguous.
2. **Call the Provider directly from the management handler.** Rejected: it
   would duplicate lease, egress, health and event ownership and could drift
   from serving behavior.
3. **Allow a small retry budget for “diagnostic usefulness.”** Rejected: a
   diagnostic must identify one exact attempt; retries obscure whether the
   pinned credential itself is usable and can create unintended external cost.
4. **Accept arbitrary request bodies.** Deferred: body content can contain
   secrets or large/unbounded data and is unnecessary for the fixed first
   probe. A separate change request is required for richer operator fixtures.

## Consequences

Channel Pin gives operators a deterministic, auditable answer to “was this
exact binding sent, and what closed stage stopped it?” without weakening the
public data plane. It deliberately does not prove account-wide health, proxy
pool quality, or another Provider's availability. A real Provider call remains
an explicit operator action and must use the same selected Config Version and
egress policy; this ADR does not authorize production or staging traffic by
itself.

The management OpenAPI contract and generated Prism client must be synchronized
if the route is implemented. The frontend may display only the closed receipt
fields and must not calculate routing, retry, or Provider fallback decisions.
Any such contract change is recorded in `docs/cross-boundary-log.md` for
Claude Code.

## Validation and rollback

The implementation is accepted only after focused tests prove authentication,
selected-version admission, exact ownership, ambiguity rejection, one-send
and no-retry behavior, first-failure termination, credential attribution,
upstream sent/not-sent reporting, JSON/SSE projection, audit emission,
secret-free receipts, lease release, and rollback/no-mutation behavior.

Tests use deterministic mock drivers and do not contact a Provider. The
pre-execution audit actions are the durable management record; the returned
value-free receipt is the terminal source-of-truth for the bounded drain. The
ordinary serving Attempt event path is deliberately not used because it would
be emitted before response-source drain. Production
and staging remain unchanged during the local slice. If a pin execution or
composition is invalid, the operation rejects before sending; disabling or
removing the management route is a safe rollback and does not require a route
graph migration.
