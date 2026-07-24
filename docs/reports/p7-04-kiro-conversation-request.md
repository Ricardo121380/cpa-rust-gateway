# P7-04 Kiro Canonical conversation request report

`conversation_request` is a pure Canonical-to-Kiro envelope builder. It constructs explicit
`conversationState` current/history entries, takes IDE/CLI origin from endpoint policy, and emits
the Kiro Tool declaration envelope. The public requested model is not forwarded.

The current scope is intentionally text conversation plus declared Tools only. Historical Tool
calls/results, AskUserQuestion, Plan Mode, Thinking, profile insertion, EventStream, model
discovery, network, and error classification remain outside this task. Unsupported canonical
semantics fail closed instead of being collapsed into a user prompt.

Fixture coverage asserts IDE and CLI multi-turn structures, Tool schema wrapping, selected-model
substitution, origin propagation, invalid context/schema handling, and redacted diagnostics. The
local Full Gate passed. Review found that a structurally present but empty text block could violate
the final-user-message contract; it was corrected with a regression assertion before this task
entered `LOCAL_PASS_PENDING_PHASE_GATE`.
