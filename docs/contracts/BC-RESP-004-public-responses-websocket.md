# BC-RESP-004: Public OpenAI Responses WebSocket

Status: `P13-10A DONE_WITH_BOUNDARY`; aggregate formal Gate `31926927914` passed

Owner: `gateway-http-actix` / `protocol-openai-responses` / `gateway-router` / runtime composition

References: [ADR-0092](../adr/ADR-0092-public-responses-websocket.md),
[BC-RESP-002](BC-RESP-002-stored-response-public-lifecycle.md),
[BC-RESP-003](BC-RESP-003-exact-continuity-and-compaction.md),
[BC-STREAM-001](BC-STREAM-001-bounded-canonical-stream.md)

## 1. Public route and upgrade admission

- The route is `GET /v1/responses`; `POST /v1/responses` JSON/SSE remains unchanged.
- The request MUST authenticate exactly one valid Client Key using the existing public data-plane
  Bearer or `x-api-key` rules before the server returns `101 Switching Protocols`.
- Missing, duplicate, conflicting, or invalid authentication MUST fail before upgrade and MUST NOT
  create a session, lease, Provider request, or stored response.
- Any `Origin` header is rejected in this native-client slice. Absence of `Origin` is required.
- At most one `x-codex-turn-state` header is accepted. Its value is at most 512 bytes and is echoed
  unchanged only on a successful upgrade.
- Upgrade and pre-upgrade responses MUST carry `Cache-Control: no-store`.

## 2. Client event contract

- Input messages are UTF-8 text JSON only. Binary input closes with `1003`.
- The root event MUST have `type: "response.create"`. `response.append`, Realtime events, and
  unknown event types are rejected without Provider execution.
- `type` is an ingress envelope member and MUST NOT be forwarded upstream.
- `stream` may be absent or exactly `true`; CPAR normalizes it to `true`. `false` or another type is
  rejected.
- `generate` is not accepted in P13-10A. The existing Responses duplicate-name, unsupported-field,
  Tool, extension, storage, and continuity checks remain authoritative.
- One frame and one reassembled message are each at most 4 MiB. One text message has at most 64
  fragments. Invalid sequence closes with `1002`, invalid UTF-8 with `1007`, and an exceeded bound
  with `1009`.

## 3. Session and backpressure contract

- Exactly one turn executes at a time. At most one complete request may wait behind it.
- A third pending request closes the connection with `1008` and cancels the active turn.
- Every outbound text/control write has a 15-second bound.
- Active Canonical event idle is 90 seconds; a turn total is 10 minutes; ping cadence is 15
  seconds; missing Pong closes after 45 seconds; no-active-turn idle is 5 minutes; a connection
  lifetime is 2 hours.
- Disconnect, peer Close, timeout, backpressure, protocol close, or server shutdown MUST drop the
  active Canonical source and release its Credential lease. A queued turn MUST NOT start after the
  session closes.

## 4. Execution, capability, and Provider isolation

- The ingress MUST use the existing authenticated model view, RouteSnapshot, scheduler, Health,
  Quota, Credential pool, Attempt orchestration, egress policy, usage observer, and Canonical
  stream. It MUST NOT create a WebSocket-specific Provider selector or lease owner.
- `ResponsesClientTransport::WebSocket` requires the selected candidate to declare
  `responses_websocket`. Missing capability returns a safe request failure before lease/Provider.
- Downstream WebSocket does not imply an upstream WebSocket. HTTP/SSE upstream transports remain
  valid when the declared adapter can produce the bounded Canonical lifecycle.
- Ordinary first-semantic retry rules remain authoritative for a new turn. Exact continuation
  remains one lineage/account and MUST NOT use sibling Credential, Provider, egress, or format
  fallback.

## 5. Output and terminal semantics

- Each Responses lifecycle event is sent as one JSON text message, without SSE `data:` framing.
- Event shape, order, response ID, Tool/Reasoning/Usage/stop projection, and terminal classification
  are produced by the existing OpenAI Responses encoder.
- A Canonical `ResponseEnd` maps to a completed terminal event. `StreamError`, truncation, timeout,
  malformed lifecycle, or output-bound failure MUST NOT be represented as completed.
- A request-level error is one bounded `{ "type": "error", "error": ... }` text message and MUST
  not expose a URL, header, cookie, token, Credential, ciphertext/plaintext, Client-Key digest,
  raw Provider body, or internal debug value.
- One outbound JSON message is at most 4 MiB; total encoded output per turn and total retained
  Canonical capture are each at most 8 MiB; at most 4096 Canonical events are accepted.
- The first-semantic delivery tracker is committed only after the corresponding WebSocket text
  write succeeds.

## 6. Storage and continuity

- `store:true` retains the P13-09 owner, AEAD, TTL, payload, and durability-before-terminal rules;
  the upstream normalized request remains `store:false`.
- A connection keeps at most 16 completed roots and 32 MiB of Canonical state. Only a valid
  successful lifecycle may enter this cache.
- A same-connection owned `previous_response_id` replays the complete Canonical request/response
  and carries exact Config/Provider/Upstream/Channel/Route/Candidate/Credential revision as
  `WebSocketSession` lineage.
- Cache miss may resolve through the existing durable Client-Key-owned store. Missing, foreign,
  expired, corrupt, model-mismatched, route-mismatched, or unavailable exact lineage fails closed.
- Downstream response IDs and compact locators MUST NOT be forwarded to an unrelated upstream.

## 7. Explicit non-goals

- Realtime API sessions/events.
- Chat Completions or Anthropic Messages WebSocket routes.
- `response.append`, `generate:false` prewarm, or binary/media messages.
- Provider-native upstream WebSocket optimization.
- Browser Origin allowlists, management UI controls, or management OpenAPI changes.
- Real Provider, staging, production, Caddy, DNS, or server mutation.

## 8. Required verification

- Real loopback RFC 6455 handshake: unauthorized `401`, browser-Origin `403`, authenticated `101`,
  turn-state echo, and `no-store`.
- Strict `response.create` decoder and normalized upstream payload.
- Lifecycle JSON messages through `response.completed`, plus Tool/Reasoning/Usage regression from
  the shared encoder.
- Same-connection exact `previous_response_id` replay and durable-store fallback invariants.
- Fragment sequence, UTF-8, binary, frame/message/fragment/event/byte limits and close codes.
- One active + one pending bound; third request `1008`; disconnect/close cancels the source and
  releases the lease.
- Ping/Pong, write, event-idle, turn-total, session-idle, and total-session bounds.
- Capability-missing rejection before Provider and unchanged HTTP JSON/SSE paths.
- Format, strict Clippy, dependency/license/source, docs/link/secret/whitespace, and diff checks.
