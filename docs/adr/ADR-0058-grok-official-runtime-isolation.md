# ADR-0058: Grok Official runtime quota and failure isolation

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P8-05` |
| Matrix / Contract | `C01`、`C03`、`C31`、`C33`、`F07`、`G24-G27`; [BC-PROVIDER-016](../contracts/BC-PROVIDER-016-grok-official-runtime-isolation.md) |

## Context

P8-03 deliberately retained only safe Official Header/usage metadata. It did not have authority to
mutate quota state. P6 already owns Build OAuth runtime state, cache affinity, response ownership,
and encrypted reasoning replay; importing any of those into the API-key Official path would violate
the Provider-family isolation contract.

The generic Router quota registry can accept a target-local, source-labelled snapshot, but it never
interprets Header material. P8-05 supplies the narrow Official handoff: exact selected
Endpoint/Credential, fixed rate-limit resource labels, explicit observation time, and no raw value.
It also defines safe ownership for HTTP status failures before a later credential/health owner acts.

## Decision

1. `GrokOfficialRuntimeState` has exactly one fixed `grok.official` Provider ID, injected Router
   quota registry, Endpoint ID, and Credential ID. It maps a complete Official Header tuple only
   to that binding-wide target as `Header/Observed` windows `official.requests` and
   `official.tokens`; reset instants are checked additions to the supplied observation time.
2. A successful response with no complete resource tuple writes no quota. `Retry-After` alone is
   interpreted only for a classified `429`: it writes an exact Official binding cooldown through
   the generic fixed fallback path, preserving `Header/Observed` when positive or
   `Estimated/Estimated` when missing/zero.
3. Failure ownership is explicit and value-free: `401` requires only Official credential
   replacement; unknown `403` is egress-local with no credential mutation; `429` records only the
   exact Official quota; `408`/`5xx` request only Official endpoint cooling; other failures do not
   mutate state. This boundary itself does not inspect bodies, retain raw headers, disable an
   account, retry, or select a route.
4. Official continuity is `Stateless`. It cannot access Build cache affinity, response ownership,
   reasoning replay, OAuth, billing/catalog state, or any future Web state. Same-named public
   models continue to require an explicit Route Candidate and do not make state interchangeable.

## Consequences

- Header-derived Official exhaustion blocks only the owning Official binding in the generic
  pre-lease quota registry. A Build binding remains independently schedulable.
- No Build/Web failure, account, quota, or affinity transition can be reached through the Official
  API surface. Conversely, no Official 401/403/429 changes a Build runtime record.
- The controlled-recovery policy remains Router-owned. P8-05 records evidence; it cannot make an
  elapsed reset admit ordinary traffic.
- The small production dependency on `gateway-router` is one-way and limited to sanitized
  Router-owned quota types. It neither gives Provider code a route/model selector nor introduces
  a Router dependency on a concrete Provider.

## Alternatives considered

- Reuse Build state/affinity because model names overlap: rejected; credentials and continuity
  semantics differ and violate `C31`.
- Store raw Headers or an Official account/billing record: rejected; P8-03 does not establish
  either contract and raw values are not needed by the quota registry.
- Treat every `403` as a permanently forbidden Official API key: rejected; without independent
  account evidence a policy/egress denial must remain non-destructive.
- Write a `Retry-After` only snapshot on every response: rejected; it does not identify a quota
  resource or prove a successful response should be blocked.

## Validation and rollback

Synthetic P8-05 integration tests seed Build catalog/quota/affinity state, apply Official Header
and failure observations, and prove the Build state is byte-for-byte unchanged while the exact
Official target alone cools. An Official adapter fixture proves the opt-in transport-header handoff
executes once. Together they cover 401/403/429/503 ownership, fallback behavior, empty metadata,
invalid time, and redacted diagnostics. Formatting, Clippy, full workspace tests, source/crate
boundaries, documentation links, Secret checks, dependency policy, and RustSec audit must pass
locally.

Rollback removes the Official runtime module/test/ADR/contract/report/index links and restores the
previous metadata-only P8-03 boundary. It changes no real xAI request, API key, OAuth file,
server, route, proxy/TUN setting, production traffic, or persisted account state.
