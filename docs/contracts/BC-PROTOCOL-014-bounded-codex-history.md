# BC-PROTOCOL-014: Bounded original Codex input history

| Field | Value |
| --- | --- |
| Task | #62 combined controlled original Codex acceptance |
| ADR | [ADR-0120](../adr/ADR-0120-bounded-codex-history-and-external-diagnostics.md) |
| Scope | HTTP Responses -> actual GrokBuildResponses -> Responses, Canonical/CanonicalBridge, owned loopback |
| Activation | Independent local history qualification; fixed Build 1.0.50 BUILD_ACTIVATION_BLOCKED |

## Sources and authority

Retained original Codex 0.161.0 requests and fixed
[models.rs](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/protocol/src/models.rs)
show optional reasoning identity and function-result identity. Complete sources/hashes are frozen
in the [source manifest](../reports/assets/cpar62-codex-combined-20261021/source-manifest.json).
Public create schema still requires reasoning id. This explicitly authorized local compatibility
contract does not claim public-schema compliance or fixed-service support.

## Closed history shapes

Existing reasoning with id is unchanged, including owned-token authentication. The added shape
has type=reasoning, absent id and present summary array of 0–64 summary_text parts, each with only
type/text and no NUL. content/encrypted_content and status may be absent or null only. Existing
1 MiB item bound applies. Unknown/duplicate keys, id:null, malformed identities, non-summary bodies
and non-null encrypted_content reject. Preserve complete native item/order/presence; never invent
an identity or ciphertext, reinterpret summary as a body or bypass owner checks.

function_call_output may have its own optional id, independent of call_id. Present id is unchanged
1–512 ASCII graphic bytes. Null/type/whitespace/control/non-ASCII/overflow reject. It joins the
existing effective-history native ID set across messages, reasoning, functions, hosted items and
other results; equal repeats also conflict. Self-contained replay across requests remains legal.
call_id must still match a pending call exactly once; name/namespace presence/value retain their
separate rules. Unknown result members/status reject. IDs grant no lookup, routing, ownership,
idempotency or authorization rights.

## Qualification, fidelity and evidence

Either shape needs independent local history qualification. Actual candidate admission requires
Build, Responses source/target, HTTP, Canonical/CanonicalBridge and owned test transport. Builder/
adapter default false; only private cfg(test) assembly calls the dependency's feature-gated setter.
Other field flags, JSON, UA, headers, URL and ordinary configuration cannot grant it. Other adapters,
modes, targets and transports remain unqualified. Generic reasoning encoding rejects missing id;
qualified Build retains absent/null without summary normalization.

Validate full effective history after owned continuation expansion. Existing payload v1 preserves
values/presence; owner/model/route/candidate/credential lineage, keys/AAD/TTL/recovery stay intact.
C4 is the independent encrypted local recipient in BC-PROTOCOL-010, never upstream history.
Require actual tools/continuation and per-request receiver/Request/Attempt/Usage/ledger/C4 joins:
three completed turns, five complete incremental 200s, two exact exec_command/write_stdin results
and real 42 newline file. session_id comes from prior actual tool output. Failure/interruption
are separate; cleanup never proves cancellation. Missing usage/cost stays UNKNOWN; #61 separate.

The fixed local acceptance config uses the original client's supported request_max_retries=0
and stream_max_retries=0. This disables transport retries for the bounded scenario, without changing
tool declarations, parallel/search capabilities or request fields. Default client retry behavior is
not claimed. Failure peers return failure on every unexpected retry; independent receive/Attempt
counts must still be exactly one. Codex stops reading at a complete response.completed SSE record;
record HTTP EOF polling separately. Five complete unique successful semantic terminals, exact
usage joins, no error/failed records and actual completed turns remain mandatory.

## Corresponding tests

- `codex_unidentified_reasoning_is_closed_and_preserves_presence`
- `build_codex_history_requires_independent_qualification`
- `public_build_codex_history_keeps_native_values_and_external_c4`
- `public_build_codex_history_invalid_and_unqualified_send_zero`
