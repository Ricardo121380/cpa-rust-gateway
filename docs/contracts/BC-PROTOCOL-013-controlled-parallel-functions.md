# BC-PROTOCOL-013: Bounded controlled parallel function batches

| Field | Value |
| --- | --- |
| Task | #62 C2 remainder; #58 T02/T03/T08/T10 |
| ADR | [ADR-0119](../adr/ADR-0119-controlled-parallel-function-batches.md) |
| Scope | Authenticated standard HTTP Responses -> actual GrokBuildResponses -> Responses, Canonical/CanonicalBridge -> owned loopback upstream |
| Activation | Independent local qualification only; fixed Build profile 1.0.50 BUILD_ACTIVATION_BLOCKED |

## Sources and authority

Freeze complete content, URL, retrieval time, HTTP Last-Modified when supplied, byte count and
SHA-256. An HTTP cache timestamp is not an asserted publication/revision date. Official source
representation, fixed client code, local controlled behavior and serving qualification are separate.

- [Function-calling guide](https://developers.openai.com/api/docs/guides/function-calling):
  multiple calls in one output, call_id result association, choice cardinalities, false's at-most-one
  rule, forced function's exactly-one rule, streaming item/argument correlation, native namespaces.
- [Create schema](https://developers.openai.com/api/reference/resources/responses/methods/create):
  boolean control, flat function selector, item/call/name/namespace and history shapes.
- [Streaming events](https://developers.openai.com/api/reference/resources/responses/streaming-events):
  distinct argument delta/done, item done and response terminal layers.
- Existing [BC-PROTOCOL-009](BC-PROTOCOL-009-codex-responses-controls.md),
  [011](BC-PROTOCOL-011-native-hosted-web-search.md) and
  [012](BC-PROTOCOL-012-native-input-message-id.md) retain their independent boundaries.

These sources do not qualify the fixed proxy/service or map the public Build client revision to
serving profile 1.0.50. Production capabilities remain unchanged; no real Provider call is needed
or authorized to guess that missing qualification.

## Control, count and identity

Preserve explicit true, false and absent independently through decode, Canonical and Build body.
Null and all non-booleans remain rejected, including synthetic Canonical inputs; no default is
inserted. Explicit true requires both actual effective ParallelTools capability and an independent
controlled Build field qualification. Absent does not claim the endpoint's effective default.
The qualified local decoder still guards any returned function batch when the field is absent.

| tool_choice | Allowed client function count in function-only responses | Identity |
| --- | --- | --- |
| absent / auto | zero or more; false narrows to zero or one | Complete declared identity |
| none | zero | No function or hosted output |
| required | one or more; false narrows to one | Complete declared identity |
| flat named | exactly one regardless of parallel presence/value | Exact declared flat name; namespace must be absent |

The exactly-one rule is source Responses semantics; this slice does not silently redefine other
source protocols' named selectors. Existing unsupported namespaced/custom/deferred selectors
remain rejected. Functions in different namespaces may share names. Identity is
(namespace presence/value, name), separate from item ID, call_id and output_index. No flattening,
renaming, ID synthesis or matching by bare name. Calls and output items are unique; delta/done
must refer to the same item/index and any supplied call_id/name must confirm the started call.
Namespace is carried by native function items, not invented on argument events.

Function batching and provider-hosted search are separate. A hosted item has no client call_id or
function result and cannot enter the client function batch. C5 coexistence/declaration, required
choice behavior and sequential hosted phases remain as previously contracted. No hosted execution
or overlap guarantee is inferred from ParallelTools. Namespace, search and input IDs each still
need their own qualification. The gateway executes neither functions nor agents nor searches.

## Stream and completion barrier

Reuse existing item/call maps, bounded argument accumulation and pending completion queue.
Upstream arguments may interleave across started calls. Their done and item.done may finish in
opposite orders. Keep native output array order and each item's original identity. output_index
is an output-array position, not a call ID or sequence counter. This slice retains the existing
bounded registration subset (items registered in output-array order); it does not assert that
all upstream protocols guarantee arrival order. No new global sequence continuity or event-order
rule is invented. Reviewed sequence numbers remain typed observations, never inferred defaults.

Argument deltas are provisional. arguments.done confirms one complete object, not a whole batch;
item.done confirms one native item, not whole-response success. Existing absent separate
arguments.done and same-value repeated arguments.done behavior remains; contradictory repeats,
after-item.done arguments, duplicate item.done and identity/argument drift reject. All argument
objects retain strict duplicate-member/malformed/non-object checks and existing empty-object
normalization. Existing 256-item, argument/frame/body bounds reject without truncating.

In independently qualified parallel Build decoding, hold every semantic ToolCallEnd and every
function OutputItemEnd until the whole successful response.completed snapshot confirms all item
IDs, output positions, full identities and complete arguments. This also gates the public
function_call_arguments.done and function output_item.done produced by the standard encoder.
Applicable final Usage validation/projection and existing durable confirmation precede delivery.
A valid first item cannot authorize execution if the second item, terminal or Usage fails.

EOF, transport truncation, cancellation, response.failed, incomplete function batches, terminal
contradictions, malformed or unadmitted Usage never drain the held completions. They produce no
false response.completed, successful stored continuation root or automatic retry. Provisional
starts/deltas may have reached the client and are not executable completion evidence. Cancellation
does not read ahead for Usage; observe real delta -> lease=1 -> disconnect -> independent upstream
close -> lease=0 before cleanup. Preserve original timeouts and distinguish fast probe from
bounded eventual release. Ordinary unqualified serial decoding keeps its established behavior.

## Multiple results and continuation

Reuse core ordered call/result lifecycle and complete protocol identity validation. Every call
needs one result, identified by call_id. Distinct results may arrive in reverse call order; unknown,
duplicate or missing results reject before lease/send. Supplied result name/namespace must match
the referenced complete identity. Preserve existing independent absent/null/string result members;
null does not assert an identity or grant qualification. Pending results cannot be bypassed by
ordinary conversation or hosted history. Call ID uniqueness remains the existing effective-history
rule, not a new global registry.

Exercise two independent calls in the same first response, both results in the next request,
then a genuinely later function depending on those results, its result and follow-up question.
Do not describe dependent calls across responses as parallel. Explicit history and existing owned
previous_response_id expansion retain original inputs, native output order and then new results.
C6 input IDs, assistant/reasoning/function IDs, role/content order, exact owner/model/route/candidate/
credential lineage, TTL and authorization remain. Duplicate expanded native IDs reject. No SQL,
Canonical/payload-v1/AAD/key/TTL change, backfill or historical rewrite is introduced.

## Qualification and accounting evidence

The owning private cfg(test) assembly must supply a reviewed controlled compiler profile, the
independent private parallel flag and its owned transport. The profile is derived from the stock
ledger with only local ParallelTools added, through the actual RouteCompiler and runtime factory.
Stock p12_adapter_capabilities is unchanged. Builder and adapter default unqualified. An explicit
dependency controlled-test-support feature carries local decoder permission; stock composition
has no qualifying call, including all-features builds. Request/URL/header/User-Agent/configuration,
transport injection or any other field qualification cannot grant it. Other adapter/target/mode
or WebSocket does not inherit the controlled flag.

Pre-send rejection needs independent accept/receiver=0 and actual Attempt=0. Post-send failure
needs one receiver/Attempt, no retry, no executable completion or successful root. Multiple tools
in one upstream response are one Attempt, one applicable Usage and one ledger identity, not a
per-function charge. Known token counters and inclusive measured provenance remain exact. Missing
observations, unknown counters, native success companion and Provider cost remain UNKNOWN.
cost_in_usd_ticks remains NOT_ADMITTED; #61 is not relaxed.

## Corresponding tests

- `public_build_parallel_flat_functions_reach_receiver`
- `public_build_parallel_json_and_interleaved_sse_keep_explicit_and_stored_cycles`
- `public_build_parallel_qualifications_controls_and_results_reject_before_send`
- `public_build_parallel_conflicts_never_complete_or_store_or_retry`
- `public_build_parallel_fin_and_reset_cancel_before_cleanup`
- `parallel_completion_barrier_holds_semantic_and_native_ends`
- `build_parallel_requires_independent_qualification`
- `public_build_parallel_false_absent_and_selection_cardinalities_are_exact`
- `public_build_parallel_flag_does_not_qualify_other_adapters_or_passthrough`
- `public_build_parallel_json_conflicts_fail_without_successful_roots`
- `public_build_parallel_usage_confirmation_failure_never_completes`
- `public_build_parallel_coexisting_c1_c6_fields_keep_independent_recipients`
- `public_build_parallel_stored_multi_results_reject_missing_duplicate_and_wrong_identity`
- `public_build_parallel_qualification_is_http_responses_only`
