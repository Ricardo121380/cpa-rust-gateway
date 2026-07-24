# ADR-0052 Kiro semantic Tool and Thinking mapping

| Field | Value |
|---|---|
| Status | Accepted |
| Task | `P7-07` |
| Depends on | `P7-04`; `P7-05` |
| Contract | [BC-PROVIDER-010](../contracts/BC-PROVIDER-010-kiro-semantic-tool-thinking-mapping.md) |

Kiro's binary EventStream framing is not itself a Claude Code-compatible semantic response. The
provider must retain Kiro Tool input across event frames, distinguish visible and reasoning text,
and avoid making incomplete Tool JSON executable. The read-only frozen-reference review also
showed two endpoint-specific Thinking placements and an `AskUserQuestion` shape that needs a
narrow compatibility projection.

`provider-kiro` therefore extends the pure conversation converter with exact historical Tool
pairs and endpoint-policy-owned Thinking placement. Assistant history uses Kiro `toolUses`, and a
later user message uses `userInputMessageContext.toolResults`; a result must reference an earlier,
unique Tool call. IDE requests put explicit effort under
`additionalModelRequestFields.thinking`; CLI requests put it under `outputConfig.effort` and do
not inherit the IDE wrapper.

The semantic EventStream mapper starts an explicit Canonical response lifecycle, maps Kiro text
and code to `TextDelta`, reasoning to `ReasoningDelta`, and emits Tool start/delta/end only after
strict, duplicate-free JSON object completion. Empty input becomes `{}` only for
`EnterPlanMode` and `ExitPlanMode`; a partial or ordinary empty Tool is an error.
`AskUserQuestion` maps a non-empty `header` to `question` only if `question` is absent, while
requiring the Claude Code `questions[].options` and `questions[].multiSelect` shape. It never
silently replaces a supplied question or fabricates missing choices.

This decision has no network I/O, Credential selection, HTTP/account/quota classification,
Tool execution, scheduler state mutation, route publication, or real probe. P7-08 owns Kiro
runtime error taxonomy; P7-09 owns its bounded real adapter and differential evidence.
