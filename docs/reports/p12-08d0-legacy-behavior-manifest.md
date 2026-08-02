# P12-08D0 legacy CPA three-protocol behavior manifest

| Field | Value |
|---|---|
| Plan | `v1.98` |
| Task | `P12-08D0` |
| Date | `2026-08-02` |
| Branch | `codex/p12-deployment` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Legacy reference | CLIProxyAPI tag `v7.2.101`, commit `42a00a2a6521b867c27f7ad096d08699db8e6d19` |
| Production behavior baseline | CPA `v7.2.80` |
| Network or deployment change | None |

## Outcome

This manifest freezes the old CPA source and test intent that P12-08D1-D4 will port into CPAR's
Rust protocol, Canonical, router and runtime boundaries. It is a traceability inventory, not a
claim that the nine protocol pairs already work.

The old CPA has explicit request and response translators for eight of the nine Chat Completions,
Responses and Messages pairs. Native Messages-to-Messages has no registered translator and relies
on the registry's raw-body fallback. CPAR will preserve the useful native-payload behavior but will
not copy the unsafe generic fallback: exact same-protocol pass-through is allowed only when the
strictly decoded original payload is retained; a missing cross-protocol mapping is rejected before
an upstream attempt.

## Frozen source boundary

Paths below are relative to the pinned CLIProxyAPI checkout.

### Registry, handlers and runtime shell

- `sdk/translator/registry.go`, `types.go`, and `pipeline.go`: registration, request/response lookup,
  stream/non-stream dispatch, and the old missing-transform fallback.
- `sdk/translator/registry_test.go`: registered-transform precedence, concrete response-kind
  discovery and fallback model rewrite.
- `sdk/api/handlers/handlers_execution.go` and `handlers_stream.go`: entry/response format carriage
  into executor selection.
- `sdk/api/handlers/openai/openai_handlers.go` and `openai_responses_handlers.go`: Chat and Responses
  HTTP entry behavior.
- `sdk/api/handlers/claude/code_handlers.go`: Messages entry behavior.
- `internal/runtime/executor/openai_compat_executor.go`: translated request construction, non-stream
  response translation, SSE data-line handling, Usage observation and synthetic terminal handling.
- `internal/runtime/executor/codex_executor_request.go`, `codex_executor_stream.go`, and
  `codex_executor_execute.go`: Codex Responses request mutation, headers, stream and response path.
- `internal/runtime/executor/claude_executor_request.go`, `claude_executor_stream.go`, and
  `claude_executor_execute.go`: Claude Messages request normalization, headers, compressed response
  handling, Tool-name restoration and stream path.

### Translator matrix

| Client source | Upstream Chat | Upstream Responses/Codex | Upstream Messages/Claude |
|---|---|---|---|
| Chat | `internal/translator/openai/openai/chat-completions/` | `internal/translator/codex/openai/chat-completions/` | `internal/translator/claude/openai/chat-completions/` |
| Responses | `internal/translator/openai/openai/responses/` | `internal/translator/codex/openai/responses/` | `internal/translator/claude/openai/responses/` |
| Messages | `internal/translator/openai/claude/` | `internal/translator/codex/claude/` | registry fallback; no explicit translator |

Every listed translator directory contributes its request, response, `init.go` registration and
tests. The pinned test inventory contains 197 translator tests across these eight explicit pairs:
2 native Chat, 30 Responses-to-Chat, 27 Messages-to-Chat, 37 Chat-to-Codex, 15
Responses-to-Codex, 45 Messages-to-Codex, 16 Chat-to-Claude and 25 Responses-to-Claude.

Representative runtime/handler evidence also includes:

- `internal/runtime/executor/openai_compat_executor_compact_test.go`
- `internal/runtime/executor/codex_executor_translate_test.go`
- `internal/runtime/executor/claude_executor_test.go`
- `sdk/api/handlers/openai/openai_responses_handlers_stream_test.go`
- `sdk/api/handlers/claude/code_handlers_error_test.go`
- `test/thinking_conversion_test.go`, `builtin_tools_translation_test.go`,
  `usage_logging_test.go`, and `claude_code_compatibility_sentinel_test.go`

## CPAR target mapping

| Behavior owner | Current Rust boundary | P12-08D owner |
|---|---|---|
| Strict client decode and native payload retention | `protocol-openai-chat`, `protocol-openai-responses`, `protocol-anthropic`, `gateway-http-actix`, `ResponsesExecution` | D1 |
| Canonical request vocabulary | `gateway-core::{CanonicalRequest, CanonicalMessage, MessageContent, Thinking, ToolDefinition}` | D1 |
| Request pair admission | `gateway-router::protocol_transform` | D1, D3 |
| Upstream request encoding | three protocol crates plus `provider-openai-compatible` and `provider-anthropic-compatible` | D1 |
| JSON/SSE upstream decode | `protocol-openai-chat::upstream_response`, `protocol-anthropic::upstream_response`, Responses decoder/runtime boundary | D2 |
| Client JSON/SSE encode | the three protocol crates | D2 |
| Pair publication and deterministic selection | `gateway-protocol::ApiFormatAdapterRegistry`, router snapshot compiler and `apps/gateway` composition root | D3 |
| Explain rejection projection | `gateway-router` and protected management projection | D3 |
| Golden corpus and cross-implementation comparison | `tests/differential` plus protocol/provider fixtures | D4 |

## Request behavior ledger

| Semantic | Old CPA evidence intent | CPAR target | Initial classification |
|---|---|---|---|
| Model and stream mode | selected model replaces client alias; requested stream mode reaches target | trusted route model and strictly decoded mode; native payload otherwise retained | `PARITY` |
| Ordered text history | system/developer/user/assistant turns retain order under target role rules | Canonical ordered messages; target-specific system/developer encoding | `PARITY` |
| Function Tool definitions | schema, name and description are translated with target shape | typed Tool definition; endpoint must advertise Tools and JSON Schema | `PARITY` |
| Tool call/result history | call ID, name, arguments, result and adjacency remain associated across turns | typed `ToolCall`/`ToolResult`; ambiguous, orphan or duplicate associations reject | `PARITY` with stricter validation |
| Tool choice and parallel Tools | translated only when declared tools and target capability permit it | explicit request facts plus capability admission | `PARITY` |
| Reasoning/thinking request | effort/budget and supported reasoning text map between protocol forms | typed `Thinking`; only documented effort/text mappings admitted | `PARITY` for typed subset |
| Signed/encrypted thinking | old translators carry or normalize provider-specific signatures in selected paths | no portable Canonical representation | `UNSUPPORTED_FAIL_CLOSED`; same-protocol native payload may preserve it |
| Prompt cache controls | old paths copy or normalize provider-specific cache fields | only mappings re-derivable from typed retention and target encoding may bridge | `UNSUPPORTED_FAIL_CLOSED` until separately proven |
| Images, audio, files and custom/freeform tools | several old translators support provider-specific shapes | current public P12 Canonical bridge has no complete portable contract | `UNSUPPORTED_FAIL_CLOSED`; native same-protocol only |
| Unknown fields/extensions | old registered transforms may ignore fields; missing transforms pass raw JSON | exact native same-protocol payload only | `INTENTIONAL_HARDENING` |
| Malformed or duplicate JSON members | old JSON mutation stack is generally permissive | strict duplicate-name and shape rejection before routing | `INTENTIONAL_HARDENING` |

## Response and stream behavior ledger

| Semantic | Old CPA evidence intent | CPAR target | Initial classification |
|---|---|---|---|
| Text and role | assistant text reaches the requested client envelope | one legal Canonical response lifecycle | `PARITY` |
| Tool fragments | ID/name may arrive separately; argument fragments retain order and calls remain distinct | bounded per-call assembly with unique IDs and ordered deltas | `PARITY` with finite limits |
| Multiple output choices | old Chat translators contain multi-choice/index handling | Release 1 Canonical response is a single selected generation | `UNSUPPORTED_FAIL_CLOSED` |
| Reasoning output | supported reasoning deltas remain distinct from visible text | `ReasoningDelta`; Chat rejects private reasoning it cannot represent | `PARITY` for representable pair, otherwise fail closed |
| Usage | input/output/total plus supported cache/reasoning details are preserved | checked `u64` fields and checked total; incompatible detail rejects | `PARITY` with overflow hardening |
| Stop reason | stop, length/max tokens, Tool use and explicit stop semantics map to target vocabulary | closed per-protocol mapping into `ResponseEnd` | `PARITY` |
| Stream lifecycle | first semantic event, Tool completion, final Usage and terminal event keep protocol order | Canonical state machine, FSE commit, exactly one legal terminal sequence | `PARITY` with stricter lifecycle |
| Missing terminal marker | old OpenAI-compatible executor may synthesize `[DONE]` after upstream EOF | EOF before a proven terminal is truncation | `INTENTIONAL_HARDENING`; no synthetic success |
| Late Usage after terminal | selected old tests suppress late Usage | terminal Canonical state rejects later semantic events | `PARITY` |
| Unknown event/block/status | old paths may pass through or ignore unhandled data | unknown semantic upstream data fails closed | `INTENTIONAL_HARDENING` |
| Chunking and buffering | old translators operate on complete SSE data lines/chunks | every legal transport split yields the same final semantic projection; all residues and Tool assembly are bounded | `INTENTIONAL_HARDENING` plus parity projection |

## Mandatory D1-D4 decisions

1. D1 must replace the current blanket rejection of historical Tool calls/results and Thinking with
   pair-specific typed admission only where the frozen ledger proves a lossless target mapping.
2. D1 must not weaken exact native pass-through: opaque native fields remain available only for a
   same-protocol target and never become Canonical instructions.
3. D2 must compare semantic event sequences, not old CPA's byte formatting, generated IDs or SSE
   packet boundaries. Tool association, Usage, stop reason and terminal order are binding.
4. D3 must publish only explicit pair registrations. There is no equivalent of the old registry's
   cross-protocol raw fallback or plugin guess path.
5. D4 corpus inputs must be newly minimized and value-free. Old credentials, endpoints, account
   data, production bodies and logs are outside the repository boundary.
6. A behavior found only in the old source but outside the Release 1 Text/Tool/Reasoning/Usage/
   History contract remains fail-closed unless a separate approved task extends Canonical.

## Review conclusion

- The pinned tag resolves exactly to the recorded commit; the inventory is reproducible while the
  temporary checkout exists, and all durable references use repository-relative upstream paths.
- The nine-pair matrix is complete: eight explicit old translators plus one native fallback case.
- Old raw fallback, synthetic `[DONE]`, permissive unknown handling and unbounded behavior are not
  accepted as parity requirements.
- No implementation, production graph, credential, endpoint, server, Caddy, DNS or traffic changed.

## Verification

| Command | Result |
|---|---|
| `git -C /tmp/cliproxyapi-v7.2.101-codex rev-parse HEAD` | PASS; exact pinned commit |
| `git -C /tmp/cliproxyapi-v7.2.101-codex describe --tags --exact-match HEAD` | PASS; `v7.2.101` |
| translator registration and test inventory review | PASS; nine pairs accounted for |
| `./scripts/check.sh docs` | PASS |
| `git diff --check` | PASS |

## Next boundary

P12-08D1 may implement only the request-side typed mappings admitted by this manifest. Response/SSE
translation, runtime registry publication, live credentials and production changes remain outside
that slice.
