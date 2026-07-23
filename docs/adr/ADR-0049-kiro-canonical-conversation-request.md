# ADR-0049 Kiro Canonical conversation request conversion

| Field | Value |
|---|---|
| Status | Accepted |
| Task | `P7-04` |
| Contract | [BC-PROVIDER-007](../contracts/BC-PROVIDER-007-kiro-canonical-conversation-request.md) |

`provider-kiro` owns a pure, fail-closed conversion from the narrow text-only portion of
`CanonicalRequest` to a Kiro `conversationState` request. The caller supplies a bounded
conversation ID, a selected upstream model, and explicit environment context; the conversion
never reads host state, sends a request, or derives a model from the public requested model.

The final Canonical user message becomes `currentMessage.userInputMessage`. Earlier ordered user
and assistant text messages become Kiro history. IDE and CLI origins come only from the existing
endpoint policy, so CLI conversion does not require a post-serialization origin rewrite. Declared
Tools are placed in the current user-input context with Kiro's `toolSpecification.inputSchema.json`
envelope.

System/developer roles, opaque blocks, historical Tool calls/results, Canonical extensions,
Thinking, prompt-cache controls, profile injection, and transport are intentionally not coerced.
They fail closed or remain in their dedicated P7 task. This prevents a partial request adapter from
silently changing role, Tool, or Thinking semantics.
