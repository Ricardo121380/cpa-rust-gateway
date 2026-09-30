# BC-PROVIDER-010 Kiro semantic Tool, Thinking, and Claude Code mapping

| Field | Value |
|---|---|
| Task | `P7-07` |
| ADR | [ADR-0052](../adr/ADR-0052-kiro-semantic-tool-thinking-mapping.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |

| Concern | Requirement |
|---|---|
| Historical Tools | An assistant historical Tool call has a bounded, non-control ID/name and a strict JSON-object input. It is represented as Kiro `toolUses`. IDs are unique and a user Tool result must reference a preceding call; unpaired, extended, scalar/array, or malformed history fails closed. |
| Historical results | A valid later user result or Canonical `tool` message containing only correlated results becomes `userInputMessageContext.toolResults` with its exact JSON value and an explicit success/error status. The final message may be such a result. Text is not used to smuggle a Tool result. |
| Thinking | With explicit extension-free Canonical Thinking effort (`low`, `medium`, `high`, `xhigh`, `max`), IDE uses `additionalModelRequestFields.thinking.effort`; CLI uses `outputConfig.effort` and omits the IDE wrapper. Numeric budgets and enabled/disabled flags are not effort. No `-thinking` model alias is made. |
| Lifecycle | The semantic mapper starts one Canonical response/assistant message, maps visible Kiro text/code to `TextDelta` and Kiro reasoning to `ReasoningDelta`, then permits exactly one normal message/response end when no Tool is open and framing is complete. The stop reason is `tool_use` if a Tool completed, otherwise `end_turn`. |
| Tool stream | `toolUseEvent` retains exact input fragment order under a fixed 1 MiB bound. It emits a `ToolCallEnd` only after an explicit stop and strict duplicate-free JSON-object completion. EOF with an open Tool, malformed JSON, duplicate JSON fields, invalid IDs, and unknown events fail closed. |
| Empty input | Only zero-input `EnterPlanMode` and `ExitPlanMode` normalize to `{}`. An ordinary empty Tool is invalid; non-empty partial JSON is never closed or repaired. |
| AskUserQuestion | Every emitted question has non-empty `question`, array `options`, and boolean `multiSelect`. A missing `question` may be filled only from a non-empty `header`; an existing question is preserved. Missing/malformed schema is invalid. |
| Framing relation | P7-05 remains responsible for CRC and byte framing. Once frames are valid, arbitrary transport chunk boundaries must yield the same Canonical semantic sequence. |
| Deferred | HTTP, OAuth/API-key injection, endpoint fallback/retry, Tool execution, account/model/quota/429 classification, scheduler state, public API exposure, and real E2E are outside this contract. |

CPAR C applies these semantics through the public runtime factory; that integration and its
remaining output-limit blocker are recorded separately in [C evidence](../reports/cpar-batch-c-20260930.md).
The P7 local status above remains historical and does not certify full channel acceptance.
