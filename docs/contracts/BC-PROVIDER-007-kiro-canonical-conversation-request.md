# BC-PROVIDER-007 Kiro Canonical conversation request conversion

| Field | Value |
|---|---|
| Task | `P7-04` |
| ADR | [ADR-0049](../adr/ADR-0049-kiro-canonical-conversation-request.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |

| Concern | Requirement |
|---|---|
| Purity | The converter performs no network, filesystem, process, environment, clock, RNG, credential, or profile lookup access. Conversation ID and environment context are explicit bounded inputs. |
| Model/origin | The caller-selected upstream model is the only `modelId`; `requested_model` is never forwarded. Every user input gets the existing IDE `AI_EDITOR` or CLI `KIRO_CLI` origin directly from `KiroEndpointPolicy`. |
| Conversation shape | One non-empty final user text message is `conversationState.currentMessage.userInputMessage`. Earlier ordered user/assistant text messages become `history` entries with `userInputMessage` / `assistantResponseMessage` wrappers. |
| Context | The current message always contains caller-provided `envState` and, when declared, Kiro Tool specifications under `userInputMessageContext.tools`. The converter does not read ambient working-directory or OS data. |
| Tools | A declared Tool must have a non-empty name, no unscoped extensions, and an object JSON schema. It serializes as `toolSpecification.{name,description,inputSchema.json}`. Missing descriptions become an explicit empty Kiro string. |
| Fail closed | System/developer roles, opaque blocks, historical Tool calls/results, empty/extended message content, Canonical root/message/Text/Tool extensions, Thinking, prompt cache controls, invalid IDs/model/context, and invalid Tool schemas are rejected with value-only errors. |
| Deferred | P7-03 alone injects `profileArn`; P7-05 owns EventStream; P7-06 owns model discovery; P7-07 owns historical Tool/AskUserQuestion/Plan/Thinking semantics; P7-08 owns runtime error classification. |
| Diagnostics | Conversation IDs, environment paths, model IDs, Canonical message content, request body, and Tool values are redacted from the public `Debug` forms. |
