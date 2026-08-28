# BC-HTTP-002 Actix Chat Completions boundary

| Field | Value |
|---|---|
| Contract | `BC-HTTP-002` |
| Task | `P12-08B` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Domain | Public Chat Completions HTTP and SSE delivery |

## Entry and pre-header order

`POST /v1/chat/completions` is registered on the existing data-plane listener. It requires exactly
one Client Key in the same Bearer or `x-api-key` admission boundary as Responses and Messages.
Authentication completes before any request body is buffered, decoded, or sent to an executor.

After authentication, the handler reads raw bytes under the fixed 4 MiB inference-body bound and
30-second receive timeout. It preserves duplicate JSON names for BC-PROTOCOL-008, resolves the
requested public model against the authenticated Snapshot, creates a request context, starts the
router executor, requires the first upstream canonical event to be `ResponseStart`, and constructs
public Chat metadata before committing success headers.

All failures before success headers use the safe OpenAI error envelope. Oversize bodies return 413;
invalid Chat JSON returns 400; missing/ambiguous/invalid Client Keys return 401 with
`WWW-Authenticate: Bearer`. None may start Provider execution.

## Bounded JSON and SSE delivery

- Both modes reuse the existing bounded `CanonicalEventStream`; no Chat-specific unbounded queue,
  detached producer, or direct Provider dependency is introduced.
- Non-streaming mode validates a complete successful Canonical response and hands one JSON chunk to
  Actix through `JsonDeliveryBody`.
- Streaming mode proves `ResponseStart` encodable before headers and uses the shared
  `ProtocolSseBody`. The first semantic delivery is committed only when Actix polls the first
  semantic bytes, never on source pull, encoding, queueing, or the shared `: keepalive` comment.
- Body drop shares the same cancellation token and drops the in-flight source. The 15-second
  keepalive remains transport-only and cannot commit FSE or appear after terminality.
- Normal Chat SSE order is content/Tool frames, finish, optional usage-only frame, and exactly one
  `[DONE]`. A post-header failure uses the Chat safe error frame and terminates rather than
  fabricating a completion.

## Observability and exclusions

Accepted requests emit `GatewayProtocol::OpenAiChatCompletions` so Chat traffic is not mislabeled as
Responses. Request and Usage events remain value-free and correlated through the same request ID.

This contract creates no `openai/chat-completions` upstream `ApiFormat`, outbound adapter, or
cross-protocol admission claim. P12-08C-D own those boundaries; therefore P12-08B alone is not a
four-channel or production-readiness result.

## Corresponding tests

- In-process Actix E2E covers authenticated non-streaming JSON, Request/Usage observations, and
  streaming content→finish→usage→`[DONE]` ordering.
- Negative E2E proves unauthenticated and 4 MiB overflow requests stop before executor start and
  retain the Chat/OpenAI error envelope.
- The shared body regression suite covers keepalive/FSE, cancellation, source EOF, stream error,
  encoding failure, queue saturation, and existing Responses/Messages behavior.
