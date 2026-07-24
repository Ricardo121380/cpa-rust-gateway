# ADR-0057: Grok Official Tool, Reasoning, and Search capability boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P8-04` |
| Matrix / Contract | `B11`、`B12`、`B16`、`C01`、`C03`、`C18`、`C31`、`F02`; [BC-PROVIDER-015](../contracts/BC-PROVIDER-015-grok-official-tool-reasoning-capability.md) |

## Context

P8-02 deliberately accepted only text. The Canonical Request and Event stream already represent
Function Tool declarations, historical Function Calls/Results, object arguments, explicit named
Reasoning, incremental Tool arguments, and Reasoning deltas. They can therefore be converted to
the native Official Responses subset without importing Build OAuth profile/state code.

Native web Search is different. `B21` remains `Later`; the current OpenAI Responses ingress rejects
provider-owned Search tool shapes and Canonical output has no safe semantic type for Search-call or
Search-result payloads. Reporting it as ordinary Function Tool capability would be inaccurate.

## Decision

1. The Official codec natively declares `Tools`, `ParallelTools`, `JsonSchema`, `Reasoning`, and
   `Streaming`; it does not declare Vision or native Search.
2. Convert extension-free Function Tools, historical assistant Function Calls, string Tool Results,
   and `low`/`medium`/`high` Canonical Reasoning effort to explicit Official Responses fields.
   Function schemas and arguments must be JSON objects; Tool-result errors, opaque content, cache,
   unknown extensions, and unrepresentable roles fail before transport.
3. Decode completed/streaming `message`, `reasoning`, and `function_call` items. Tool item/call/name
   identity is fixed from addition to completion; incremental and final object arguments must match.
   Reasoning and Tool values remain redacted in Debug.
4. Declare native Search as `UnavailablePendingCanonicalContract`. Native Search request/output
   objects fail closed; a later explicit Canonical + ingress contract must define their client
   semantics before this value can change.
5. This is a codec/capability task only: it does not issue a real request, select a route, persist
   Reasoning replay, mutate quota/health/account state, or share any Build/Web runtime namespace.

## Consequences

- Tool and Reasoning routes can be admitted only after an Endpoint/Candidate explicitly records the
  returned capability set; future router wiring cannot infer it from an Official model name.
- SSE byte segmentation may vary argument-delta frame count, but its final Canonical Tool/Reasoning
  semantic projection is equal to the non-streaming completed representation.
- Search stays visibly unavailable instead of being silently converted to opaque data or dropped.

## Alternatives considered

- Reuse `build_responses`: rejected because P8 cannot couple API-key Official semantics to Build
  OAuth/CLI profile or runtime state.
- Treat Search as an arbitrary `function`: rejected because provider-executed Search has different
  request/output/attribution semantics and no current Canonical representation.
- Preserve unknown Tool/Reasoning extensions: rejected because the official codec cannot prove their
  meaning or collision safety.
- Advertise all named Reasoning efforts: rejected; only the tested bounded Official mapping is
  accepted until per-model discovery proves another value.

## Validation and rollback

Synthetic tests cover the declared capability set, request/history conversion, strict Reasoning
effort, non-streaming/SSE Tool and Reasoning semantic projection across every tested byte split,
redaction, invalid Tool argument completion, and explicit Search refusal. Formatting, Clippy, full
workspace tests, policy checks, document links, Secret checks, dependency policy, and RustSec audit
must pass locally.

Rollback removes the Official capability module, P8-04 codec/test/doc changes, and index links. It
does not modify an API key, endpoint, server, account, route, proxy/TUN setting, production traffic,
or persisted state.
