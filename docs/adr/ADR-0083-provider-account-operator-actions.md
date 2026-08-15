# ADR-0083: Provider account-pool operator actions and failure feedback

- Status: Accepted; local implementation complete, P13 phase gate pending
- Date: 2026-08-15
- Scope: protected management operations over the existing serving account-pool state

## Context

P13-06A/B provide a secret-free inventory of the same ordinary and native account pools used by
the request scheduler. Operators still need a bounded way to quarantine one exact account or ask
the existing controlled recovery state machine to re-evaluate a blocked account. They also need
failure evidence that explains why a row is unavailable. Reusing a second scheduler, parsing raw
logs, or exposing upstream responses would create a second source of truth and would risk leaking
credentials.

## Decision

Add two protected operations:

1. `cool_down` writes a finite cooldown to the exact Endpoint-Credential or
   Endpoint-Credential-Model Health key. The duration is bounded and the action never acquires a
   lease or contacts a Provider.
2. `request_recovery` delegates to the existing account/quota recovery ticket state machine. It
   may return rejected, recovery-required, or probe-scheduled state; it cannot move a quota window
   before its reset and cannot perform OAuth refresh/reauth.

Both operations use the existing management listener, Management Key, same-origin CSRF and
selected `X-Config-Version` admission. They write a value-free resource audit record but do not
publish a Config Version or increment its graph revision: Health/Quota is runtime state, not
durable configuration.

Add a bounded failure-feedback read model sourced only from gateway-owned durable `AttemptEvent`
rows. It returns opaque Provider/Channel/Account attribution, request/attempt ids, terminal time,
closed `GatewayError` code/scope and retry decision. It excludes upstream model labels, URLs,
headers, cookies, bodies, raw messages, secrets and client-key digests. Filtering is exact and
pagination is stable; source decode/size failures fail closed.

## Alternatives rejected

- **Provider-specific HTTP calls from management:** would turn an operator click into an
  unbounded network action and duplicate Provider executors. Provider-specific refresh/reauth stays
  in P13-12.
- **Durable enable/disable mutation in this task:** ordinary bindings already use Config Version
  mutation; native account stores need Provider-specific lifecycle semantics. Mixing them here
  would make one generic action unsafe across Grok, Codex and Krill.
- **Raw log/error forwarding:** raw upstream values can contain tokens, URLs or account data. The
  typed Attempt event already carries the safe classification required by operators.

## Consequences

- Runtime actions are immediately visible through the existing P13-06B inventory, while a process
  restart intentionally rebuilds state from the normal runtime bootstrap rules.
- The management OpenAPI contract changes. Codex synchronizes the vendored Prism contract and
  generated client; Claude Code adds the UI state/actions later. The handoff is recorded in
  `docs/cross-boundary-log.md`.
- Automatic refresh, reauthentication, replenishment and proxy-pool management remain deferred.
