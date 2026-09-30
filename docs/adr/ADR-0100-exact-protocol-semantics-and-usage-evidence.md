# ADR-0100: Exact protocol semantics and source Usage evidence

Status: Accepted for CPAR batch B, 2026-09-30.

Batch B implements CPAR-18–29 (#27–38) on the registered Chat Completions,
Responses and Messages paths. This decision supersedes the lossy reasoning,
signature and Usage restrictions in ADR-0034/0036/0037/0038 where they conflict
with the rules below. The transport ownership, bounded queues, explicit
capability admission and exact continuity rules of ADR-0016/0090/0091/0092 remain
applicable.

Every execution mode validates known request semantics before acquiring an
upstream attempt. An exact native payload is insufficient authority to accept
an unknown control. Tool declarations, correlation IDs, complete object
arguments and ordered call/result history must be valid. Reviewed `auto`,
`none`, `required`/`any`, named function selection and parallel/serial controls
have explicit equivalents across all three protocols. Explicit parallel use
participates in capability admission, the chosen upstream request retains the
control, and response validation rejects undeclared tools or a violated
selection/serial constraint. Interleaved tool fragments retain independent
bounded buffers; transparent retry cannot replay delivered semantic output.

Qualitative OpenAI effort and Anthropic token budgets have no provable numeric
equivalence. OpenAI effort is retained exactly between its two protocols;
numeric budgets remain exact in Messages. Converting an enabled budget into
an effort bucket, or an effort into an approximate budget, is rejected. Chat
visible thinking uses `reasoning_content`, separate from answer text, including
reasoning-only assistant messages with null answer content in later history. An
endpoint's reasoning capability is not lowered to hide an output mismatch.

Native Responses output retains item IDs, part indices, summary/content
hierarchy, phase and reviewed annotations. Native Messages output retains
ordered thinking blocks, signature fragments and reviewed citations. The
request codecs validate the closed citation schemas without fetching referenced
URLs/files. Messages signatures and document citations, Responses annotations
and structured reasoning have no equivalent representation in the other
supported response shapes: those bridges fail explicitly. They cannot end
successfully or become a saved/session replay root. Foreign encrypted reasoning
also fails without an ownership/decryption path, including through the legacy
Codex suppression entry points. We accept this stricter compatibility boundary
instead of returning an answer with silently omitted reasoning evidence.

Usage carries six optional source counters plus `provenance` (`measured`,
`estimated`, `unknown`) and `input_accounting` (`inclusive`, `exclusive`,
`unknown`). OpenAI input totals include cache subsets; Messages input totals
exclude its cache read/creation counters. Protocol output uses exact known
arithmetic where the target uses a different input definition. If a required
counter is absent, the target aggregate remains absent. The public `usage`
object includes `cpar_usage`, with all six nullable source counters and both
evidence fields, so protocol conversion cannot erase known details. Legacy
canonical Usage without evidence fields defaults to unknown. Protocol framing
may use an initial Messages output placeholder of zero; it is not recorded as
an observed final counter.

The observer merges snapshots and durably emits the last known source Usage on
completion, stream failure or cancellation, with its exact attempt ID. Decoder
buffers also expose validated Usage without reading more upstream data, so a
content projection failure cannot discard counters in a complete JSON body or
an already decoded SSE batch. Validated counters preceding a malformed frame
remain observable even if both frames arrive in one transport chunk. The runtime
does not narrow source Usage according to the downstream protocol. Read models
and the billing ledger retain that evidence. Schema 0033 adds evidence
metadata to the existing six-counter ledger without guessing historical
provenance. Legacy ledger fingerprints remain stable when both new fields are
unknown. Estimated or unknown Usage has no exact bill. Measured pricing respects
input accounting and subtracts priced cache/reasoning subsets before charging
the base totals; it never charges a subset twice. Missing applicable counters
produce an explicit partial/unknown amount. Conflicting attempt lineage remains
excluded rather than assigned to another attempt.

Opt-in `store:true` requires `StoredResponses` on the selected candidate before
any lease or upstream invocation, separately from `ResponseCompaction` and
`ResponsesWebSocket`. Global repository availability does not confer a channel
capability. Responses payloads and compact tokens retain the existing exact logical
Client-Key owner, route/candidate/channel, credential revision, AEAD and TTL
contracts. A completed response must be publicly encodable before persistence;
a WebSocket root is registered only after successful encoding/delivery.
Persistence failure cannot acknowledge a successful stored terminal. Real
process restart and public GET/continuation/compact acceptance are distinct from
reopening a store connection. Failure/EOF/cancellation terminates once, and
Chat failures do not include its successful `[DONE]` marker. WebSocket close or
pending-turn overflow cancels the shared upstream source and releases capacity.
The deployed data listener disables HTTP/1 read-half continuation: a client FIN
cancels the handler/body, including bootstrap and idle SSE. Clients must keep
both halves open while awaiting inference output. Management listener behavior
is unchanged.

Protocol shape references are the [OpenAI Responses output types](https://github.com/openai/openai-python/blob/main/src/openai/types/responses/response_output_text.py),
[Anthropic citation types](https://github.com/anthropics/anthropic-sdk-python/tree/main/src/anthropic/types),
[Anthropic thinking documentation](https://platform.claude.com/docs/en/build-with-claude/thinking),
and [Kimi thinking documentation](https://platform.kimi.com/blog/posts/kimi-thinking).
These define representations; they do not constitute real-provider acceptance.
Local evidence and remaining acceptance boundaries are recorded in the
[batch B report](../reports/cpar-batch-b-20260930.md).
