# P7-07 Kiro Tool, Thinking, and Claude Code compatibility report

`conversation_request` now preserves safe historical assistant Tool calls and subsequent user
Tool results instead of collapsing them into text. It enforces unique paired IDs and strict
object inputs, and it places explicit Canonical Thinking exactly once through the IDE or CLI
endpoint policy. The public requested model remains absent from the upstream body.

`event_semantics` consumes only CRC-verified P7-05 frames. It creates the Canonical lifecycle,
maps Kiro visible content/code and reasoning separately, buffers Tool input until `stop`, and
accepts a Tool end only after strict JSON-object completion. `AskUserQuestion` gains only the
missing `questions[].question` field from its own `header`; it must still carry `options` and
`multiSelect`. Empty input maps to `{}` only for the two Plan Mode Tools.

No Kiro request was sent. Transport/account/quota/error classification remains P7-08; the
bounded differential and real `--bare` validation remains P7-09.

## Verification and review

| Check | Result |
|---|---|
| `cargo test --locked -p provider-kiro --test p7_04_conversation_request` | PASS; existing P7-04 text fixtures continue to pass and malformed historical Tool state remains rejected. |
| `cargo test --locked -p provider-kiro --test p7_07_claude_code_compatibility` | PASS; paired history, IDE/CLI Thinking, text/code/reasoning, AskUserQuestion normalization, Plan Mode, partial/duplicate JSON rejection, diagnostic redaction, and every byte split regression. |
| `cargo clippy --locked -p provider-kiro --all-targets -- -D warnings` | PASS. |

Review focus: Tool values never appear in public diagnostics; a Tool cannot reach Canonical end
with invalid or invented input; and the small AskUserQuestion normalization does not weaken the
rest of the Claude Code shape. This task is `LOCAL_PASS_PENDING_PHASE_GATE`.
