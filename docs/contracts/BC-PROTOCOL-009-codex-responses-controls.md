# BC-PROTOCOL-009: Observed Codex Responses controls and bounded admission

| Field | Value |
| --- | --- |
| Status | PARTIAL; summary-only local implementation, other required controls BLOCKED |
| Task | #62; parent #58 T01/T02/T03/T08/T10 |
| ADR | [ADR-0113](../adr/ADR-0113-responses-summary-without-effort.md) |
| Profile | Fixed Build 1.0.50; unchanged endpoint, model binding and capability declarations |

## Evidence and scope

The observed source is the isolated, unmodified Codex CLI 0.161.0 request recorded in the
[#62 independent report](../reports/cpar-62-independent-20261019.md). Standard definitions,
client source, faithful CPAR encoding, fixed service qualification, and real Provider execution
are different evidence layers. A standard field or a client implementation alone does not prove
that the fixed Build service accepts it. This contract authorizes no real Provider execution.

- [OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning) defines summary
  independently from effort. The official [Codex Reasoning struct](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/codex-api/src/common.rs#L158)
  omits absent effort. The [frozen Grok client](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/client.rs#L1253)
  also constructs summary with no effort. These support retaining absence, not choosing a default.
- [OpenAI function calling](https://developers.openai.com/api/docs/guides/function-calling#defining-namespaces)
  and the [Responses schema](https://developers.openai.com/api/reference/resources/responses/methods/create)
  define namespace declarations and separate namespace/name/call_id information. The frozen Build
  client's function types do not establish the fixed service's namespace support.
- [Codex metadata](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/core/src/responses_metadata.rs#L224)
  and its [HTTP request type](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/codex-api/src/common.rs#L278)
  establish a Codex-owned `map<string,string>`, distinct from standard object `metadata`.
- [OpenAI web search](https://developers.openai.com/api/docs/guides/tools-web-search#live-internet-access)
  gives `external_web_access:false` cache-only meaning. The [frozen Build options](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/tool_overrides.rs#L163)
  do not establish that constraint. Hosted-search code in a public client is not fixed-profile
  qualification for declaration, events, results/citations or all applicable Usage.

Source bytes, hashes, observation time and bounded negative-source checks belong in the delivery
report. The frozen public Build tree has no proven release mapping to production 1.0.50. No
generic `api.x.ai` schema, current stable version, synthetic fixture or prior single sample fills
that gap. #61 cost remains NOT_ADMITTED; no cost re-research or additional metadata admission.

## Field-by-field execution contract

| Field | Input semantics and Canonical representation | Admission and Build output | Return/history and boundary |
| --- | --- | --- | --- |
| summary-only reasoning | A sole supported summary selector, with effort **absent**. Retain the complete object in `openai.responses.reasoning`; typed `thinking` is absent. | Responses target only; existing Reasoning capability required in every mode. Revalidate and emit the same object, with no effort added. | Preserve validated native summary and ordered history under existing ownership rules. This does not invent reasoning body, ciphertext or effective effort. |
| namespace | A named group of function/custom tools; namespace and child name are distinct from correlation call_id. No qualified Canonical execution representation is implemented. | Decoder rejects namespace declarations; known namespaced selectors/history also fail before send. No name concatenation, flattening, child dropping or alternate tool type. | Namespaced output remains unrepresentable and fails once without executable-call completion or retry. It must not be matched solely by bare function name. |
| client_metadata | Codex-owned string map. Observed keys: root_turn_id, session_id, thread_id, turn_id, x-codex-installation-id, x-codex-turn-metadata, x-codex-window-id. The last metadata blob remains a JSON **string**. Root decoder retains the entire field under `openai.responses.client_metadata`. | The executable admitted set for Build is empty: router rejects every shape before acquisition/send. No unknown-root whitelist, silent deletion, standard-metadata renaming, or translation into Build tracking headers. | No response echo, storage/search semantics or Build interpretation is promised. Client-supplied identities never grant authentication, owner or route authority. |
| web_search | Provider-executed hosted tool, including explicit cache-only control; not a client function. No qualified Canonical search contract is implemented. | Reject declarations before send, whether external access is false, true or absent. Do not remove the declaration/constraint, fabricate a result, or infer qualification from Codex. | Search-call output item ID, phase events, search/open/find actions, sources, URL citations and related Usage need a joint fixed-profile contract before admission. |

### Summary-only strictness and collisions

Only objects exactly equal in value to `{"summary":"auto"}`, `{"summary":"concise"}` or
`{"summary":"detailed"}` enter the new slice. Missing summary, null, boolean, unknown values,
extra members and duplicate JSON names reject. Explicit null effort is not absence. Existing
effort-bearing controls keep the original representation; typed Thinking and the new root object
together reject, even if their summaries appear equal. No precedence or merge is inferred.

Chat/Messages do not have an approved equivalent for this summary request with absent effort.
Cross-protocol routes reject; they do not add medium/auto/none, delete the summary, or turn thinking
off. Official/Console and other providers do not inherit a new capability from shared code.

### Namespace qualifications still required

Before this control can enter production Build execution, define and prove the entire chain:
declaration and child schema, namespace/name identity, duplicate namespace/child and cross-group
name conflicts, tool_choice and parallel/serial constraints, returned namespace/name and call_id,
complete object arguments, exactly correlated results, and ordered later history. Current standard
sources do not establish an unambiguous forced namespaced-function selector; CPAR cannot invent
`namespace.function`, concatenate separators, or add an unproven selector field. All such requests
remain blocked. Tests cover explicit pre-send refusal for declarations, overlapping names,
selectors, serial/parallel controls, and namespaced call/result history; output rejection is separate.

## Persistence, upgrade and recovery

No Canonical struct, effort field, schema 35, stored payload version 1, AAD, key or TTL changes.
The extension envelope is already part of encrypted stored requests. A frozen old effort-bearing
Canonical JSON shape retains exact serialization; both it and the new extension form reopen with
exact-owner isolation and idempotent replay after key rotation. This is a storage-boundary test,
not proof of an old production database or deployed process upgrade.

Older readers can retain the existing envelope but older execution code rejects the new control.
Do not infer execution/continuation compatibility from readable JSON. Any recovery keeps coherent
events/ledger/evidence, keys and owner/TTL rules and validates the chosen reader/execution code on
an isolated snapshot. No automatic rewrite, historical backfill or data deletion is introduced.

## Actual CLI completion and remaining blockers

`scripts/acceptance/cpar62-cli.py --require-codex-success --output <new-OS-temp-directory>` adds an
explicit success gate after preparation receipts are written. It requires zero CLI exit, three
completed turns, five incremental public responses, independent receives/exact Attempt/Usage/ledger,
two correlated tool results and actual result.txt value 42. The existing blocked Codex scenario
cannot pass it; HTTP400 preparation PASS remains a separate diagnostic result. A positive actual
Codex scenario still requires the blocked fields' qualification and its complete native streaming,
controls/history, failure/cancel and exact encrypted-evidence oracles. This gate does not certify
those unavailable paths by itself. Full's ignored test is not actual CLI execution.

#62 stays OPEN/BLOCKED while these fields and #61 remain unresolved. Pi's qualified sequential
explicit-history path is regressed separately. Its failed-Usage display, roughly 15-second live
cancel release, one-second diagnostic FAIL and missing complete success companion remain visible.
Real Grok, other client forms, remote CI, release and deployment remain separate.

## Corresponding tests

- `public_build_summary_only_reasoning_preserves_absent_effort`
- `public_build_summary_only_json_and_fragmented_sse_keep_two_tool_cycles`
- `public_build_summary_only_invalid_controls_fail_before_send`
- `public_build_codex_fields_remain_individually_blocked_before_send`
- `public_build_namespaced_output_is_rejected_without_flattening_or_retry`
- `summary_only_reasoning_keeps_effort_absent_and_requires_native_reasoning_capability`
- `summary_only_reasoning_has_an_explicit_extension_and_no_effort_default`
- `summary_only_and_legacy_effort_payloads_reopen_without_rewriting_or_replay_conflicts`
- `summary_only_duplicate_keys_are_rejected_by_router_in_every_mode`
