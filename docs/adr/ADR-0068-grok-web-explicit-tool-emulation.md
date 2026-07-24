# ADR-0068: Grok Web explicit Tool emulation

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P9-08` |
| Matrix / Contract | `C30`、`C31`、`D28`、`F17`; [BC-PROVIDER-021](../contracts/BC-PROVIDER-021-grok-web-explicit-tool-emulation.md) |

## Context

The Web fixture Chat adapter has no proven native function-calling protocol. A prompt convention may be useful for isolated experiments, but advertising it as native Tools would cause invalid routing/capability decisions and silently alter client prompts. Default execution must preserve the P9-03 text-only request byte shape.

## Decision

1. Introduce `GrokWebToolEmulation`, defaulting to disabled. A disabled configuration emits no prompt addendum, even when a caller supplies Tools.
2. Keep native semantic capabilities fixed to `Streaming`; neither `Tools` nor `ParallelTools` is advertised in either flag state.
3. When explicitly enabled, expose the separate metadata value `Emulated` and prepend only a bounded, structured `mode=emulated` Tool convention for validated extension-free Tool definitions with object schemas.
4. The existing no-flag request builder remains text-only. The emulation-aware builder rejects Tool declarations while disabled, validates them before any addendum while enabled, and never creates a Tool executor, native Tool event decoder, browser action, or transport.

## Consequences

- Default requests have no hidden Tool instruction; routing cannot mistake emulation for native support.
- An operator can opt into a bounded fixture-only prompt convention while diagnostics redact Tool/prompt values.
- P9-09/G9 may test a separately authorized live convention only if it preserves the `emulated` metadata and does not claim native Tool semantics.

## Alternatives considered

- Advertise `SemanticCapability::Tools` under the flag: rejected because a prompt convention is not lossless native Tool Calling.
- Always add Tool instructions when a request contains Tools: rejected because it changes default prompt semantics and hides an operational feature flag.
- Parse model text into Tool calls now: rejected because no current Web protocol/response contract proves safe exact extraction.

## Validation and rollback

Synthetic tests prove default-off no-addendum and byte-for-byte text-request preservation, disabled Tool rejection, enabled `emulated` metadata/prompt convention with no native Tools, bounded unsafe Tool rejection, and redacted diagnostics. Rollback removes the flag/composer and its opt-in builder method only; default P9-03 behavior remains unchanged and no external action occurs.
