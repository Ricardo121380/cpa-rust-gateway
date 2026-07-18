# BC-HTTP-001 Actix Responses handler

| Field | Value |
|---|---|
| Contract | `BC-HTTP-001` |
| Task | `P1-07` |
| Status | `DONE` |
| Domain | Public HTTP entry and Actix body delivery |

## Entry and boundary

The current P1 surface exposes these endpoints:

```text
GET  /healthz
POST /v1/responses
```

`/healthz` returns `200 application/json` with `{"status":"ok"}` and remains public.
`/v1/responses` requires the P1-08 Client Key admission defined by
[BC-AUTH-001](BC-AUTH-001-client-key-auth-port.md), then accepts raw UTF-8 bytes, passes the
complete text to `protocol-openai-responses::decode_request`, executes a router-owned
`ResponsesExecutor`, and returns either a completed Responses JSON object or typed Responses SSE
frames. It does not add catalog/route selection, real Provider configuration, retries,
persistence, management APIs, deployment, or P2 work.

The HTTP crate depends on `gateway-router` only through `ResponsesExecutor` and
`ResponsesEventSource`, whose public surface contains canonical types and boxed standard-library
futures only. It has no direct `gateway-provider` dependency or Provider trait/type in its public
API. `gateway-router` adapts P1-06's deterministic Mock Provider behind that facade.

## Request admission and pre-header sequence

- Before interpreting or decoding the raw request body, the handler requires exactly one valid
  Bearer Client Key as specified by BC-AUTH-001. Authentication rejection is a safe pre-header
  `401` and does not invoke decoding, context creation, router execution, or Provider execution.
- After authentication, the handler uses `web::Bytes`, not `web::Json`. It validates UTF-8 and invokes
  `decode_request` on the untouched complete body, preserving P1-05 duplicate-name rejection at
  every JSON nesting level.
- Before creating an HTTP success response it performs, in order: authentication, decode,
  request-context creation, executor start, first source pull, first-event validation as
  `ResponseStart`, and response-metadata construction. Streaming also validates that a fresh SSE
  encoder can encode that initial event before it creates headers.
- The request's public `model` is P1's provisional public-model label. A process-local opaque
  request sequence and Unix-second clock are default implementations; `ResponsesMetadataFactory`
  allows deterministic test or later configuration injection.
- Before headers, every safe `GatewayError` becomes the P1-05
  `{ "error": { "type", "code", "message", "param": null } }` JSON envelope. P1 status mapping
  is `ClientRequestError -> 400`, `ClientUnauthorized -> 401`, `RouteNotFound -> 404`, rate/quota
  errors -> `429`, transient/unavailable errors -> `503`, Provider/protocol/truncation and related
  upstream errors -> `502`, and internal/cancelled errors -> `500`. A `401` also includes
  `WWW-Authenticate: Bearer`.

## Bounded execution and cancellation

After the pre-header `ResponseStart` check, P1-07 queues it and all later source events through
one explicit P1-04 `CanonicalEventStream`. The event-count capacity is state-owned and defaults to
eight; it is never an unbounded byte or event buffer. The HTTP-local Tokio producer task uses the
stream cancellation token while awaiting each router source pull, so cancellation drops an
outstanding pull and source rather than allowing detached work to continue.

- A valid `ResponseEnd` or `StreamError` is forwarded once and ends the producer.
- A non-cancelled source EOF before terminality becomes terminal
  `StreamError(StreamTruncated/Stream)`.
- A non-cancelled out-of-band source or bounded-stream validation error becomes terminal
  `StreamError` with its existing safe error.
- Client disconnect/body drop cancels the P1-04 stream. It emits no fabricated `StreamError` or
  `response.failed`; the producer observes cancellation and drops its source quietly.

## Output and FirstSemanticEvent

Non-streaming mode drains the bounded stream before headers, validates a successful
`CanonicalResponse`, and encodes it using P1-05. A terminal canonical `StreamError` remains an
error JSON envelope because no successful JSON body has been handed to Actix.

Streaming mode writes `text/event-stream` typed P1-05 frames. Its custom `MessageBody` owns the
bounded receiver and encoder. `FirstSemanticEventTracker::mark_delivered` is called only inside
that body's `poll_next`, immediately when a semantic byte chunk is returned to Actix. It is not
called on source pull, enqueue, dequeue, initial encoding, or frame queueing. A completed JSON
body uses the equivalent custom body boundary: it marks only when its full JSON byte chunk is
returned to Actix.

## Post-header failure policy

- A canonical post-start `StreamError` encodes exactly one `response.failed` and never
  `response.completed`.
- A source EOF, out-of-band error, receiver truncation error, or post-header encoder/rendering
  error is converted to a safe terminal failure and sent through the existing encoder. It produces
  exactly one `response.failed` and no completion whenever an already accepted `ResponseStart`
  leaves that response representable.
- If the fallback itself cannot be encoded after an otherwise terminal encoder state, the body
  closes. It does not invent a completion, emit a second terminal event, or replace a client
  cancellation with a failure.
- An initial event that cannot be encoded is a pre-header failure and uses the JSON error envelope
  instead.

## Corresponding tests

- In-process Actix HTTP E2E covers public `/healthz`, authenticated non-streaming Responses JSON,
  typed streaming SSE, Client Key rejection before decode/Provider execution, duplicate JSON-name
  rejection, pre-start error JSON, canonical post-start failure, source early EOF conversion, and
  post-header encoder failure conversion.
- Direct body tests prove JSON and SSE FirstSemanticEvent remain uncommitted before body polling
  and commit only when their first semantic bytes chunk is returned to Actix.
- A cancellation test drops an unconsumed SSE body and verifies the delayed router source is
  dropped through P1-04 cancellation rather than left running.
