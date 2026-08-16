# ADR-0092: Public OpenAI Responses WebSocket over the Canonical execution path

Status: Accepted — formally gated by `phase-p13-websocket-complete` / `dc48ec40e4fb38961925f203bf3cd0f7434a34a0` / run `31926927914`

Date: 2026-08-16

Task / Matrix / Contract references: P13-10A; A04; B10; B29; B30; D26; D27;
[BC-RESP-004](../contracts/BC-RESP-004-public-responses-websocket.md)

## Context

OpenAI Responses WebSocket mode uses a persistent upgrade on `GET /v1/responses`. A client sends
flat `response.create` JSON events and receives the same Responses lifecycle events used by the
streaming HTTP API. It is not the Realtime API and does not use Realtime session or conversation
events. The [official WebSocket guide](https://developers.openai.com/api/docs/guides/websocket-mode)
and [official Node Responses guide](https://github.com/openai/openai-node/blob/main/docs/responses.md)
document this distinction and the flat event shape. CPA `v7.2.80` also exposes this route, but its
permissive browser-origin behavior and implementation details are not copied.

CPAR already has a bounded Canonical request/event path, Client Key authentication, first-semantic
delivery tracking, exact Provider/Channel/Credential lineage, encrypted stored Responses, and
exact continuity. A separate WebSocket Provider executor would duplicate these owners and risk
changing HTTP/SSE routing, retry, storage, or accounting semantics.

## Decision

1. CPAR exposes `GET /v1/responses` as the public Responses WebSocket upgrade while retaining
   `POST /v1/responses` JSON/SSE unchanged. The first slice accepts text messages whose exact event
   type is `response.create`; `type` is removed and `stream` is normalized to `true` before the
   existing strict Responses decoder runs.
2. The upgrade authenticates an exact Client Key before returning `101`. Browser-origin handshakes
   are rejected in P13-10A; native clients must omit `Origin`. An optional single
   `x-codex-turn-state` value is bounded to 512 bytes and echoed in the upgrade response. Upgrade
   success and all pre-upgrade errors are `Cache-Control: no-store`.
3. Downstream WebSocket is independent of upstream transport. The selected Endpoint may continue
   to use HTTP/SSE or a Provider-specific stream. Every admitted candidate must explicitly carry
   the `responses_websocket` semantic capability in the compiled runtime ledger. Missing
   capability fails before lease or Provider execution.
4. Each turn reuses the normal model resolution, RequestEvent, `ResponsesExecution`,
   `AttemptOrchestrator`, bounded Canonical stream, usage observer, first-semantic tracker, storage
   transaction, and exact lineage recorder. The WebSocket layer only changes the downstream
   framing: every Responses event JSON object is written as one text message without SSE syntax.
5. One connection may run one active turn and retain one complete pending turn. A third pending
   request closes with policy code `1008`. Disconnect, close, timeout, or write failure aborts the
   active task; dropping the Canonical source propagates cancellation and releases its lease.
6. One connection retains at most 16 completed turns and 32 MiB of encrypted-or-in-memory-safe
   Canonical request/response state. Same-connection `previous_response_id` replays the full
   successful Canonical history and carries exact Config/Provider/Channel/Route/Candidate/
   Credential revision as `WebSocketSession` continuity. A miss may use the existing Client-Key-
   owned durable P13-09 store. Neither path forwards the downstream response ID upstream or falls
   back to a sibling account or Provider.
7. Successful `store:true` turns keep P13-09 durability-before-terminal semantics. Partial,
   malformed, timed-out, cancelled, or `StreamError` turns are not inserted into the connection
   cache as completed roots.
8. Input is text-only. One frame/reassembled message is at most 4 MiB and at most 64 fragments.
   Canonical capture remains bounded to 4096 events and 8 MiB; one outbound message is at most
   4 MiB and total turn output is at most 8 MiB. Writes, including control frames, are time-bounded.
   Active-event idle, turn-total, Pong, idle-session, and total-session deadlines are explicit.
9. Close codes follow RFC 6455 classes: `1002` invalid fragmentation/frame, `1003` binary input,
   `1007` invalid UTF-8, `1008` pending-request policy, `1009` size/fragment overflow, and `1001`
   liveness/session expiry. A valid request-level failure remains a bounded Responses-shaped
   `error` text event so the authenticated connection can continue.
10. P13-10A deliberately rejects or defers `response.append`, `generate:false` prewarm, binary
    input, Realtime API events, Chat Completions WebSocket, Anthropic Messages WebSocket, and
    Provider-native upstream WebSocket optimization. Those require separate contracts rather than
    widening this route implicitly.

The public data-plane addition does not change the management OpenAPI or Prism generated client.
The public route and client-integration implications are recorded in the cross-boundary log for
Claude Code.

## Consequences

- Existing HTTP JSON/SSE clients keep the same route and execution behavior.
- A persistent native client can perform multiple Responses turns without opening a new TCP/TLS
  connection, while CPAR retains the same Provider/account ownership and accounting evidence.
- Browser clients are intentionally not supported in this slice; accepting browsers later needs
  an explicit same-origin/allowlist and CSRF-equivalent WebSocket policy.
- Upstream native WebSocket can be added later as a Provider transport optimization without
  changing the downstream protocol or bypassing Canonical validation.
- `actix-ws` is a runtime dependency; `tokio-tungstenite` is test-only and proves a real loopback
  RFC 6455 handshake and message lifecycle.

## Rejected alternatives

- **Reuse the Realtime API protocol.** Rejected because its events and session state are not
  Responses WebSocket mode.
- **Require every Provider to support upstream WebSocket.** Rejected because downstream transport
  projection already works over bounded HTTP/SSE Providers and transport capability is separate.
- **Allow all browser origins like the legacy reference.** Rejected because Client Keys in browser
  contexts need a separately reviewed origin and credential policy.
- **Create a second retry/scheduler/storage path.** Rejected because it would split the owners of
  lease, first-semantic retry, usage, stored response, and continuity state.
- **Buffer an unbounded number of turns or events.** Rejected because a slow or hostile client
  could otherwise retain Credentials, connections, and memory indefinitely.

## Validation and rollback

Validation covers strict event decoding, real authenticated loopback handshake, no-Origin policy,
turn-state echo, lifecycle framing, exact same-connection continuation, fragment/message bounds,
pending-queue policy close, cancellation/source drop, capability fail-closed behavior, HTTP/SSE
regressions, strict Clippy/format, dependency policy, docs, and secret scans. No real Provider,
staging, production, server, DNS, Caddy, or account-pool mutation is authorized by this ADR.

Rollback removes the GET route and `responses_websocket` capability while leaving the existing
POST JSON/SSE route, stored-response tables, and Provider adapters intact.
