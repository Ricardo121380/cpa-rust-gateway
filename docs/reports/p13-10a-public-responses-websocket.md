# P13-10A Public Responses WebSocket report

Status: `DONE_WITH_BOUNDARY`

Date: 2026-08-16

## Outcome

P13-10A adds an authenticated public `GET /v1/responses` WebSocket upgrade and projects the
existing bounded OpenAI Responses Canonical lifecycle as JSON text messages. It does not add a
Realtime API, a second routing path, or a requirement for Provider-native upstream WebSocket.

The implementation reuses the current Client Key, model view, scheduler/lease, Health/Quota,
Attempt, usage, stored response, and exact continuity owners. A candidate must carry the new
`responses_websocket` capability before it can be leased for this ingress.

## Frozen boundary

| Area | P13-10A decision |
|---|---|
| Public route | `GET /v1/responses`; existing POST JSON/SSE unchanged |
| Input | Text-only flat `response.create`; strict Responses decoder; forced `stream:true` |
| Auth/origin | Existing exact Client Key before upgrade; any browser `Origin` rejected |
| Output | One existing Responses lifecycle JSON event per WebSocket text message |
| Concurrency | One active turn + one pending complete request; third closes `1008` |
| Continuity | Connection-local bounded Canonical replay plus existing durable owner lookup; exact lineage |
| Bounds | 4-MiB frame/message, 64 fragments, 4096 events, 8-MiB turn capture/output, explicit write/liveness/session deadlines |
| Cancellation | Disconnect/close/timeout aborts the turn and drops the Canonical source/lease |
| Deferred | `response.append`, `generate:false`, Realtime, Chat/Messages WS, binary/media, upstream-native WS |
| External effects | No real Provider, staging, production, server, DNS, Caddy, or account-pool mutation |

## Implementation evidence

- `protocol-openai-responses` owns strict WebSocket envelope decoding and native-payload
  normalization.
- `gateway-catalog` and `gateway-router` own the explicit semantic capability and downstream
  transport marker.
- Runtime candidate admission enforces the capability before lease/Provider execution.
- `gateway-http-actix` owns upgrade admission, fragment/session state, bounded writes, Responses
  event framing, local completed-turn cache, and cancellation.
- P13-09's Canonical replay helper is shared between durable and connection-local continuity.
- `actix-ws` is runtime-only; `tokio-tungstenite` is used only by real loopback tests.

## Verification

| Check | Result |
|---|---|
| Strict `response.create` decoder and upstream normalization | PASS |
| Catalog capability dependency | PASS |
| Real loopback auth/Origin/upgrade/turn-state/lifecycle/two-turn continuity | PASS |
| Fragment/message/type bounds, UTF-8 and close-code mapping | PASS |
| Pending-request `1008` close and active source cancellation | PASS |
| Same-session cache idempotence and response-ID collision rejection | PASS |
| Runtime capability fail-closed and HTTP/SSE regression | PASS — gateway 106, HTTP 65, router 138, Responses codec 30, catalog 15 |
| All affected packages / all targets | PASS — all executed unit, integration and benchmark targets; only existing explicitly authorized harnesses ignored |
| Workspace Fast Gate | PASS after review fix — every preceding step passed; crate dependency allow-list was updated, then crate/docs/contracts/secret/diff tail re-run passed |
| Dependency/license/RustSec | PASS — 360 dependencies / 1216-advisory database; allowed duplicate-version warnings only |
| Strict Clippy / fmt / docs / source / crate / secret / diff checks | PASS — 544 Markdown files, 107 contract references, 21 workspace package boundaries |
| Final local review | PASS — no remaining P1/P2 blocker |
| Aggregate local Full preflight | PASS — 43/43 steps, `2026-08-16T04:26:59Z` through `04:28:58Z` |
| Formal Delivery Gate | PASS — `phase-p13-websocket-complete` @ `dc48ec40e4fb38961925f203bf3cd0f7434a34a0`; run [31926927914](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31926927914) |

## Review notes

- The official OpenAI Node documentation confirms that Responses WebSocket is distinct from the
  Realtime API and uses flat `response.create` messages. CPA `v7.2.80` is only a behavioral
  compatibility reference; its permissive browser-origin policy is not adopted.
- Control-frame writes were included in the same bounded backpressure policy as data messages.
- Actix frame overflow and invalid UTF-8 are mapped to `1009` and `1007`; application-level
  fragmentation, binary and pending-request violations retain their closed codes.
- Connection-local completed state is bounded and tied to one authenticated connection; durable
  continuation continues to use exact Client Key ownership and AEAD.
- A repeated response ID is accepted only for an exactly identical completed turn. Conflicting
  model/request/response/lineage is rejected rather than replacing connection state.
- The management OpenAPI and `web/prism/**` are unchanged. A cross-boundary entry tells Claude
  Code about the public client surface and deferred UI/client documentation work.
- The first Fast Gate run found that the architecture allow-list had not yet named `actix-ws` and
  test-only `tokio-tungstenite`. The allow-list and crate-boundary document were corrected; the
  failed boundary and every remaining tail check then passed without repeating already-passed
  workspace tests.

## Closeout boundary

Annotated tag `phase-p13-websocket-complete` resolves to exact commit
`dc48ec40e4fb38961925f203bf3cd0f7434a34a0`; formal run
[31926927914](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31926927914)
passed Authorize, Fast, Full supply-chain and Required in `4s` / `5m51s` / `1m33s` / `3s`.
P13-10A and the aggregate therefore close as `DONE_WITH_BOUNDARY`. Provider-native upstream
WebSocket, browser support, `response.append`, Realtime, Chat/Messages WebSocket and any real
external validation remain separate work and are not implied by this report. This Gate did not
perform a Provider request, staging/production deployment or server mutation, and did not start
P13-11.
