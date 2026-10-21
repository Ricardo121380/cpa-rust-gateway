# BC-PROTOCOL-011: Native hosted web search, bounded local implementation

| Field | Value |
| --- | --- |
| Task | #62 C5; parent #58 T02/T03/T08/T10 |
| ADR | [ADR-0117](../adr/ADR-0117-native-hosted-web-search.md) |
| Scope | Standard HTTP Responses, actual GrokBuildResponses, Responses target, Canonical/CanonicalBridge |
| Activation | Local controlled qualification only; fixed Build 1.0.50 BUILD_ACTIVATION_BLOCKED |

## Sources and authority

Freeze retrieval date, complete content SHA-256 and applicable revision in the delivery source
manifest. Official schema defines representation; frozen client source corroborates its use.
Neither qualifies the fixed endpoint or real Provider execution.

- [Responses create/input/output schema](https://developers.openai.com/api/reference/resources/responses/methods/create): native WebSearch declaration and WebSearchCall in both input and output; action's optional members.
- [Responses input-items schema](https://developers.openai.com/api/reference/resources/responses/subresources/input_items/methods/list): output/history shape corroboration, not independent authority for arbitrary POST fields.
- [Streaming events](https://developers.openai.com/api/reference/resources/responses/streaming-events): web_search_call.in_progress/searching/completed and item_id/output_index/sequence_number.
- [Web search guide](https://developers.openai.com/api/docs/guides/tools-web-search): explicit false is offline/cache-only; omitted defaults true; preview ignores this control; sources and citations have different roles.
- Fixed Build revision `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`: [declaration](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/tool_overrides.rs#L163), [native history](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/conversation/responses.rs#L47), [phase and done handling](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/stream/responses.rs#L494).

CPAR still emits `https://cli-chat-proxy.grok.com/v1/responses` with its existing profile 1.0.50.
The fixed tree's SOURCE_REV and version injection do not establish source/release/service mapping.
No current stable pointer is used to silently change that profile. The source is client evidence,
not a serving attestation. No Provider request is authorized by these reads.

## Request declaration and controls

Accept exactly one top-level object with exactly `type:"web_search"` and
`external_web_access:false`. Both members are required and types are exact. Reject true,
absent/null/string/numeric external_web_access, duplicate declaration, duplicate JSON keys at any
depth, unknown keys, filters, location, context/budget options, dated types, preview, x_search,
file_search, code interpreter, custom or other undeveloped tools. Those are explicitly unsupported
subsets, not claims that the public standard forbids them. Do not remove or rewrite a control.

Reuse 256 KiB whole retained declaration JSON, 128 top-level nodes, namespace bounds and 256 total
functions. Limit the hosted declaration itself to 256 bytes. Reject instead of truncating.
Existing body/nesting/parser bounds remain. Namespace children remain function-only.
The entire ordered mixed array uses `openai.responses.tools`; legacy flat-only arrays keep their
existing typed representation. Group/function descriptions, strict presence and parameters remain.

auto/none/required, including absent choice, retain their existing semantics over the whole set.
none forbids hosted output; required can be fulfilled by hosted search. A flat named function
selector permits only that function and forbids hosted output. Forced hosted or namespaced
selectors remain unsupported. Preserve parallel_tool_calls; true independently requires the
actual ParallelTools capability and is not granted by either controlled family qualification.
false retains one client function call per response, while allowing sequential hosted actions.

## Native output, phases and completion

One search item has exactly required `id`, `type:"web_search_call"`, `status`, and optional
`action`. ID is nonempty ASCII graphic, at most 512 bytes; at most 256 total output items retain
their actual order/output_index and unique IDs across message/reasoning/function/hosted items.
Whole hosted item is at most 64 KiB. No client call_id, namespace or synthetic function identity.

Supported successful completed items require status completed. Initial SSE items require
in_progress (buffered JSON derives its internal start from a completed snapshot). Failed or
incomplete search items are explicit protocol failures in this slice; they cannot finish a
successful response. Unrecognized statuses and fields reject. action missing remains missing;
action null is invalid. Action schemas are closed:

| action.type | Required members | Optional members and presence |
| --- | --- | --- |
| search | type | query string (deprecated singular), queries array of strings, sources array of exact {type:"url",url:string}; each may be absent, null rejects |
| open_page | type | url may be absent, explicit null, or supported URL string |
| find_in_page | type, pattern string | url may be absent, explicit null, or supported URL string |

Do not invent `find` as an alias from the Build client's older consumer enum. Queries/pattern are
at most 4096 UTF-8 bytes each, at most 128 queries and 128 sources. URL strings are at most 8192
bytes, absolute http/https without whitespace/control characters; this is CPAR's bounded subset.
Preserve exact strings, empty query/queries/sources and nullable URLs; no fetching/normalization.
Sources are only consulted-source URLs, not invented titles, results, search counts or charges.

Observed phases have exactly type,item_id,output_index,sequence_number. Require all four,
nonnegative integer indices/sequence, stable declared hosted identity and output index, and
in_progress -> optional searching -> completed order with no repeats/regression. Sequence numbers
increase for a given item's observed phases. Phases may all be absent; item.done remains the
completion authority. Once any phase occurs, completed must precede a successful item.done.
Public SSE generates its own coherent stream sequence numbers while retaining event identity,
order and presence. JSON does not fabricate phase observations. Added action, when supplied,
must confirm exactly at done; done and terminal must have identical hosted keys/values including
optional presence. Hold hosted OutputItemEnd and coexisting function executable completion
until terminal checks and standard Usage admission.
Completed phase is not whole-response success. EOF, cancel, mismatch and malformed data fail once;
no synthetic result, completion, retry or evidence read-ahead is allowed.

## Citations, answers and explicit history

Reuse the four existing closed annotation schemas and incremental confirmation mechanism.
URL citations retain exact type,start_index,end_index,url,title and their annotation/part order.
Keep source URLs separate from selected citations; a citation need not equal every source or
cover every consulted source. Validate URL shape and final text bounds, preserve character
offsets without recalculating them, and reject malformed/reversed ranges or changing annotations.
Titles remain exactly supplied strings, including an empty title; no title lookup is performed.

Completed hosted items may appear at their actual position in explicit input history, using the
same closed schema. Retain a sole OpaqueContent on an assistant CanonicalMessage and revalidate
before emitting the exact top-level web_search_call. Its closed `openai.responses.hosted_history:true`
extension marks the validated top-level ingress form; nested content blocks do not receive it.
Reject wrong role, mixed/nested opaque
content, message extensions, duplicate native history IDs and unfinished hosted history. No
function_call_output is expected for hosted actions. Ordinary call/result correlation and ordered
history remain enforced; pending client results cannot be bypassed by hosted history.
History itself independently requires C5 qualification even without a current search declaration;
new hosted output requires a current declaration. Later assistant text retains citations and
native identities through the existing history encoder.

## Usage, statistics and persistence

Known standard token Usage retains exact counters, provenance, inclusive input accounting and
Request/actual Attempt/one ledger identity. Missing counters are not zero. Search counts, result
counts, tool_usage, and provider-specific Usage are independent semantics; no count is derived
from actions, sources or citations. Unreviewed Usage remains rejected with existing complete
encrypted failed-Usage evidence and known-counter recording where already validated. Root search
statistics remain unsupported and BLOCKED; ordinary root rejection is not a promise of a new
encrypted whole-response evidence format. Never remove an unknown statistic from a sample to
make it positive. cost_in_usd_ticks remains NOT_ADMITTED. Provider/search charges remain UNKNOWN.

No database or payload-v1 shape changes. Existing RawExtensions/OpaqueContent and native events
carry the new bounded values; no implicit rewrite/backfill. Old readers can retain envelopes but
old execution code rejects new semantics. Existing encryption, reader, owner, AAD, TTL and recovery
rules remain. Explicit-history support does not grant stored server continuation/compaction modes.

## Activation evidence still needed

Before production activation obtain a version/date-bound serving declaration explicitly applying
to fixed proxy/profile 1.0.50, or a verified source/release/service chain, covering cache-only false,
native declaration, actions/phases/JSON/SSE, citations/sources, explicit history, applicable controls
and all search Usage/statistics. Local loopback isolation cannot establish remote cache-only behavior.
Original Codex retains C6, actual ParallelTools, namespace activation, #61 and other prerequisites.
No result in this contract changes #62 or first-version three-client completion conditions.

## Corresponding tests

- `public_build_search_json_and_fragmented_sse_keep_explicit_history`
- `public_build_search_coexists_with_flat_and_namespaced_functions`
- `public_build_search_invalid_and_unqualified_requests_send_zero`
- `public_build_search_json_and_sse_reject_malformed_native_items_and_citations`
- `public_build_search_sse_rejects_phase_and_terminal_conflicts_once`
- `public_build_search_unknown_usage_retains_exact_failed_evidence_and_never_completes`
- `public_build_search_root_statistics_remain_blocked_in_json_and_sse`
- `public_build_search_truncation_and_eof_fail_once_without_retry`
- `public_build_search_fin_and_reset_cancel_release_and_reuse_capacity`
- `hosted_search_extensions_reopen_in_payload_v1_without_presence_loss`
