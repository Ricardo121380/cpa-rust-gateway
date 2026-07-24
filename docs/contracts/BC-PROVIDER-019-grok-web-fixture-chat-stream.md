# BC-PROVIDER-019 Grok Web fixture Chat request and stream

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-019` |
| Task | `P9-03` |
| ADR | [ADR-0063](../adr/ADR-0063-grok-web-fixture-chat-stream-boundary.md) |
| Matrix | `C29-C31`、`D28`、`E27-E29` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001`; synthetic-only and zero-network |
| Domain | Browser-egress-bound fixture request and strict incremental SSE-to-Canonical codec |

## Preconditions and bounds

1. The caller supplies a non-expired immutable `GrokWebBrowserEgressSession`, a printable selected model of at most 512 bytes, and explicit non-negative observation time.
2. The target is permanently the non-routable fixture host `grok.example.test` and fixture path `/api/web-chat`; no caller can select a live URL in this contract.
3. One SSE record is at most 64 KiB excluding its `\n\n` or `\r\n\r\n` delimiter. JSON is strict: duplicate object names are invalid.

## Required behavior

| Concern | Required behavior |
|---|---|
| Request admission | Only one extension-free `user` message with one non-empty Text part is accepted. Tools, Thinking, cache, opaque parts, historical turns, provider extensions, invalid model, and expired/unscoped sessions fail before any transport capability is available. |
| Fingerprint use | The fixture request gets its Cookie only through the P9-02 HTTPS scope matcher and copies the explicit P9-02 User-Agent into zeroizing request-scoped storage. It does not read a browser/profile, environment, proxy/TUN configuration, server file, or clock. |
| Fixture boundary | Request target, wire body, Cookie, User-Agent, selected model, and message are redacted in `Debug`. The blueprint has no URL/client/DNS/TLS/socket/HTTP/proxy-send API. It is not real-provider evidence. |
| Stream grammar | Only `web.response.start`, `web.message.start`, `web.text.delta`, `web.message.end`, `web.response.end`, `web.response.error`, and terminal `done/[DONE]` are legal. Each payload `type` equals its SSE event; response IDs match; message role is assistant. |
| Canonical lifecycle | The decoder applies every emitted event to `CanonicalEventState`. It rejects duplicate starts, empty deltas, incomplete messages, duplicate/unknown fields/events, malformed UTF-8/JSON, oversize records, data after terminal state, and repeated/missing `done`. |
| Chunk property | Any valid split of identical bytes produces the same Canonical event sequence. EOF before a complete terminal marker returns `StreamTruncated/Stream`; malformed data returns `UpstreamProtocolError/Stream`. |
| Ownership | P9-04 owns Conversation/Parent-ID binding, P9-06 owns Web quota, P9-07 owns WAF/account attribution, P9-08 owns Tool emulation, and P9-09 owns real Web endpoint/account/protocol validation. |

## Corresponding tests

- `fixture_request_binds_the_immutable_browser_fingerprint_and_redacts_values`
- `later_web_semantics_or_an_unusable_session_are_rejected_before_any_transport`
- `synthetic_sse_is_chunk_invariant_and_has_one_complete_canonical_lifecycle`
- `malformed_unknown_premature_or_post_terminal_sse_fails_closed`
