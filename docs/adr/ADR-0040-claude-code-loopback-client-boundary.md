# ADR-0040: Claude Code loopback readiness and client-key boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-22` |
| Task | `P5-07` |
| Contract | [BC-E2E-003](../contracts/BC-E2E-003-claude-code-loopback-bare-compatibility.md) |

## Context

P5-06 made `POST /v1/messages` available through the shared HTTP authentication boundary, but a
controlled local run of Claude Code `2.1.214` established two concrete client behaviors that the
earlier boundary did not admit: it first probes the configured base URL with `HEAD /`, then sends
Messages requests using `x-api-key` rather than `Authorization: Bearer`. The client also uses a
`?beta=true` query, performs an internal title request, and retains forward-compatible request
fields. P5-01/P5-06 already retain or reject those body semantics at their explicit protocol
boundaries; this decision is limited to reachability and key presentation.

The compatibility adjustment must not turn a readiness probe into discovery, accept ambiguous key
sources, weaken duplicate-header protection, or introduce a real Provider request into the client
regression test.

## Decision

1. Register only unauthenticated `HEAD /`. It returns `200 OK` with an empty body and reveals no
   route, model, authentication, snapshot, or Provider state. It is not a general root resource.
2. Every existing client-key HTTP route obtains its key through one shared admission function. It
   accepts exactly one of `Authorization: Bearer <key>` or `x-api-key: <key>`. The key must be a
   single non-empty, ASCII-whitespace-free header value and is then verified by the existing
   authenticator.
3. Missing keys, duplicate `Authorization` or `x-api-key` headers, both schemes together,
   malformed Bearer syntax, whitespace-bearing values, and unknown/disabled keys fail as the
   existing safe unauthorized error before body decode or executor/Provider work.
4. The Claude Code compatibility proof is an ignored, explicitly enabled Rust test. It starts an
   Actix listener only on `127.0.0.1`, clears the child environment, supplies a synthetic local
   model and key, and uses only fixed local `printf` Tool calls. It retains in-memory aggregate
   counters, never raw request/response bodies, headers, or key material.

## Consequences

- Claude Code can complete its base-URL reachability check and use its native key header without a
  protocol-specific authentication fork.
- OpenAI Responses, Models, Messages, and count-token routes share the same strict client-key
  ambiguity rule; no route silently prefers one supplied key over another.
- The local E2E proves real client behavior against the gateway boundary while remaining separate
  from a real Provider, deployed endpoint, credential, or product configuration.

## Alternatives considered

- Require Claude Code to send a Bearer header: rejected because the observed supported client
  behavior uses `x-api-key`.
- Accept both headers and select one by precedence: rejected because an attacker-controlled second
  value would make authentication provenance ambiguous.
- Make `GET /` or a rich unauthenticated discovery endpoint available: rejected because the probe
  needs only a body-free reachability response.
- Replace Claude Code with an in-process synthetic decoder: rejected because it cannot prove the
  actual CLI's probe, header, title request, Tool, and Plan Mode behavior.

## Validation and rollback

Unit tests prove the empty public `HEAD /` response, native `x-api-key` Messages success, and
duplicate/mixed/invalid key rejection before decode/execution. The ignored E2E invokes the explicit
local Claude Code binary and proves normal dialogue, one Tool, parallel Tools, and Plan Mode using
only the loopback gateway. Reverting this task removes the root probe, `x-api-key` admission, and
test target; it requires no data migration, credential rotation, server cleanup, or external
request rollback.
