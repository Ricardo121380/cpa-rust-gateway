# ADR-0063: Grok Web fixture Chat request and stream boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P9-03` |
| Matrix / Contract | `C29-C31`、`D28`、`E27-E29`; [BC-PROVIDER-019](../contracts/BC-PROVIDER-019-grok-web-fixture-chat-stream.md) |

## Context

P9 needs a bounded request/stream seam after P9-02 binds Cookie, User-Agent, TLS-profile label, proxy choice, and credential version into an immutable browser egress session. There is no authorized Web account or Canary, so local fixtures cannot establish current remote `grok.com` Chat semantics. A guessed live target or browser request would be an uncontrolled external probe.

## Decision

1. Encode only a synthetic, non-routable `grok.example.test` fixture request. It carries an egress-session-scoped Cookie header, User-Agent, selected model, one text message, and streaming intent, but exposes no send/URL/DNS/TLS/client/proxy action.
2. Admit exactly one extension-free Canonical `user` message containing one non-empty text part. Conversation/parent binding remains P9-04; Tools remain P9-08; Thinking, cache, opaque content, historical turns, and unowned provider extensions fail closed.
3. Define a strict bounded synthetic SSE grammar: response start, assistant-message start, text deltas, message end, response end or safe provider error, followed by exactly one `done/[DONE]`. Every JSON `type` must match the SSE event. It is decoded incrementally through `CanonicalEventState` and is invariant under arbitrary valid chunk boundaries.
4. Redact target, Cookie, User-Agent, selected model, client message, and stream contents from debug/error forms. The grammar uses strict duplicate-field rejection and cannot retain raw payloads in a failure.

## Consequences

- P9-04 can introduce conversation/parent state without widening the initial request grammar.
- P9-06 and P9-07 receive a bounded stream seam without claiming that a local fixture proves Web quota, WAF, account, Cookie, or remote protocol behavior.
- P9-09 alone may, after separate authorization, establish an admitted live target and compare actual browser/Web traffic. It must not reinterpret this fixture target as production evidence.

## Alternatives considered

- Guess and hard-code a live `grok.com` Chat endpoint: rejected because it would claim unverified protocol semantics and tempt unauthorized traffic.
- Accept arbitrary host/path for unit tests: rejected because that leaks an endpoint-selection capability into a local fixture task.
- Accept generic SSE or silently skip unknown events: rejected because protocol drift must fail closed for `grok.web` rather than be translated into incorrect Canonical semantics.

## Validation and rollback

Synthetic tests cover fingerprint/header binding, redaction, unsupported Canonical semantics, expiry rejection, arbitrary chunk splitting, strict duplicate/unknown-event rejection, premature EOF, and post-terminal data rejection. Rollback removes only this fixture module/test and its documentation; it does not read a Cookie source/browser profile/proxy/TUN setting/server file or send a Web request.
