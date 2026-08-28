# BC-PROTOCOL-008 OpenAI Chat Completions strict codec

| Field | Value |
|---|---|
| Contract | `BC-PROTOCOL-008` |
| Task | `P12-08A` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Domain | OpenAI Chat Completions protocol codec |

## Entry and boundary

`protocol-openai-chat` is a protocol-pure boundary:

```text
Chat Completions JSON request -> CanonicalRequest + response mode
CanonicalResponse -> chat.completion JSON
CanonicalEvent -> ordered chat.completion.chunk SSE frames
GatewayError -> safe OpenAI-shaped error object
```

It does not authenticate a Client Key, read an HTTP body, select an Endpoint, execute a Provider,
write to a client, or mark First Semantic Event delivery. P12-08B owns those HTTP and delivery
boundaries; P12-08C-D own outbound Chat and cross-protocol admission.

## Request admission

- `model` and a nonempty ordered `messages` array are required. System, developer, user, assistant
  history, function Tool calls, and Tool results map to Canonical without reordering.
- Function Tool definitions retain their schema and optional description. Tool call arguments must
  be duplicate-free valid JSON encoded as a string.
- Duplicate JSON names at any nesting level, `n != 1`, legacy `functions`/`function_call`, ambiguous
  content, and unsupported Tool controls fail as `ClientRequestError/Request` before routing.
- Representable but not yet interpreted native Chat options are retained only in explicit
  `openai.chat.*` raw-extension namespaces. Retention is not execution: later route/adapter admission
  must preserve them losslessly or reject the request before network I/O.

## Response and stream invariants

- Exactly one choice is emitted. Text and ordered function Tool calls retain their Canonical order,
  opaque response ID, public model label, and stop reason. Reasoning and generic raw extensions are
  rejected rather than exposed as assistant text or silently discarded.
- Tool argument delta concatenation must equal the final canonical arguments. Splitting the same
  argument string at any UTF-8 boundary yields the same completed Chat message.
- Usage requires reported input/output counts and uses checked addition. Overflow, separate
  cache-read/cache-creation counts, and other unrepresentable semantics fail closed as
  `UpstreamProtocolError/Stream`; no missing count is filled with zero and no total is saturated.
- A normal stream emits the assistant-role chunk, semantic deltas, one finish chunk, then an
  optional usage-only chunk when requested and available, and finally exactly one `data: [DONE]`.
  No codec frame is a transport keepalive.
- Error envelopes contain only the stable safe core message/code. Debug output redacts response
  payloads and the public-model field.

## Corresponding tests

- Strict request fixtures cover text/history, Tool definitions/calls/results, native extension
  retention, duplicate names, legacy fields, multiple choices, and invalid assistant content.
- Finite and streaming fixtures cover text, Tool lifecycle, Usage, stop reasons, usage ordering,
  and the terminal marker.
- Exhaustive split-point regression proves Tool argument fragmentation does not change the completed
  response; dedicated regressions reject reasoning, missing Usage counts, incompatible cache
  counts, and Usage-total overflow.
