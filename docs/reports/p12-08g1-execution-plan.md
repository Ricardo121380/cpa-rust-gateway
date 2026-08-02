# P12-08G1 controlled production-graph E2E plan

Status: `IN_PROGRESS`

Date: 2026-08-02

This plan is value-free. It records no endpoint, client key, upstream model, credential, request or
response body, source hash of Secret material, or token fingerprint.

## Exact runtime boundary

- Initial runtime source revision `02571e180c9991e632b1809479e10adb3cd01144` used private
  dual-architecture GitHub OIDC/Sigstore run `30754762093`.
- The first strict-compatibility correction was revision
  `d738b838419a1186fa692a271e34f216df8cdfab`, run `30756000209`; both architecture jobs and
  independent local verification passed before installation.
- Later exact corrections were revisions `1470a847ae4eb3c9f03ef8f8b44b8ac47c8aa253` (run
  `30756738441`), `e45f1a1c91acd725ed07dacd7079b77071785c93` (run `30757155860`) and
  `6c011cb2e761eefac5ab5bf00da0a1a81abf3d2b` (run `30757648684`). Every dual-architecture
  job, OCI smoke and independent Sigstore verification passed before its bounded loopback install.
- The terminal-delta correction was revision `85f0d2a90575110c44bf217bd0187190e754e94f`, run
  `30758498095`; both signed targets passed, but the replacement tuple still failed and rolled back.
- The delta-free-summary correction was revision `e297fa1fe7f3d0f3b200c723c9ecd4d42f9a52e6`, run
  `30759133160`; Chat SSE Text passed before the matrix stopped at Chat JSON Tool.
- Any later compatibility successor must be a new exact revision and private dual-architecture run;
  its SHA/run are recorded before activation, and only its independently verified
  `aarch64-unknown-linux-gnu` artifact may replace the disabled-at-boot CPAR binary.
- Preserve the installed binary, database, unit state, active Config Version and all five systemd
  credentials before replacement. A failed provenance, install, restart, readiness, graph,
  request, receipt, or rollback assertion stops the sequence.
- The incumbent CPA, production hostname, Caddy, Cloudflare, DNS, public traffic and CC Switch are
  outside this transaction.

## Credential and channel boundary

- Codex/OpenAI-compatible: read the existing OpenClaw Provider configuration through its `0600`
  regular file without printing values. Transfer the three required values only through an SSH
  standard-input stream into the loopback management transaction; do not place them in argv,
  environment, logs, reports, or a retained plaintext file.
- Claude/Anthropic-compatible: the independent Claude CLI reports OAuth login metadata, but no
  non-expired exportable access credential is currently available outside CC Switch. Do not import
  metadata as a Credential and do not borrow CC Switch state. Mark this channel
  `SKIP_CREDENTIAL_UNAVAILABLE` unless a separate non-CC-Switch credential becomes available.
- Kiro, Grok Official and Grok Web are not part of G1 and consume zero requests.

## Graph and tuple matrix

Create one immutable successor Config Version containing the currently available Codex channel:
one Upstream and encrypted Credential, separate Chat and Responses Endpoints, one binding per
Endpoint, two public model aliases and Routes, one Candidate per Route, one Access Group and one
new one-time Client Key. Validate, publish, restart, and require Route Explain to select exactly one
Candidate for each Route.

The fixed live matrix contains twelve single-send tuples:

| Inbound protocol / target adapter | JSON text | SSE text | JSON tool | SSE tool |
|---|---:|---:|---:|---:|
| Chat / OpenAI-compatible Chat | 1 | 1 | 1 | 1 |
| Responses / OpenAI-compatible Responses | 1 | 1 | 1 | 1 |
| Messages / lossless bridge to OpenAI-compatible Responses | 1 | 1 | 1 | 1 |

Each tuple must produce one bounded 2xx terminal lifecycle, the expected text/tool projection and
non-negative Usage. There is no retry. The first failure stops later tuples, records only a fixed
safe category, and immediately enters rollback.

## Mandatory rollback and cleanup

Regardless of success or failure:

1. roll back the active Config Version through the authenticated loopback management API;
2. restart CPAR and require the exact predecessor to be active and ready;
3. require the G1 Client Key to be rejected and the predecessor Client Key to remain accepted;
4. restore the preimage binary and restart if the candidate artifact itself caused the failure;
5. remove the one-time Client Key file, input stream helper, downloaded artifact and temporary
   scripts; retain only value-free receipts and the inactive encrypted Config Version audit record;
6. require CPA, Caddy, DNS, CC Switch and production traffic to remain unchanged.

Success means only that the available Codex production graph passed its current live protocol
matrix and rollback. It does not claim Claude, Kiro, Grok Official or Grok Web live availability,
and it does not authorize P12-09 production cutover while any required client/key ownership item
remains open.

## CR-P12-08G1-001 — failed-tuple replacement

The first run stopped after Chat JSON Text passed and Chat SSE Text returned the combined safe
category `chat_stream_lifecycle`. Both durable upstream Attempts were `succeeded/completed` and both
Usage events were present. The graph was rolled back before diagnosis; the new Client Key was
rejected and the predecessor key remained accepted.

The combined category could not distinguish missing `[DONE]`, missing finish, or an invalid Usage
projection. Before any replacement request, the harness now emits separate fixed categories and
accepts a `0600` value-free prior receipt only when every earlier tuple is an exact ordered PASS and
the final tuple is the single FAIL. It resumes at that failed tuple, never resends an earlier PASS,
and retains the global stop-on-first-failure rule. This CR authorizes one replacement of the failed
Chat SSE Text tuple and, only if it passes, the ten not-yet-attempted tuples. Maximum new sends are
eleven; there is still no per-tuple retry.

## CR-P12-08G1-002 — one direct structural classifier

The replacement produced `chat_stream_finish_missing` while the CPAR wrapper still emitted DONE;
the upstream Attempt and durable Usage again completed. Source review shows this shape is consistent
with CPAR converting a strict upstream decoder error into one public stream error frame followed by
DONE, but retained evidence cannot identify which upstream Chat event violated the decoder.

One direct OpenClaw-backed Chat SSE Text classifier is authorized. It sends exactly once, performs
no retry, and persists only event/choice/delta key sets, bounded counts, finish-reason classes,
DONE/error/Usage presence and a strict-compatibility boolean. It never stores or prints endpoint,
key, model, IDs, text, request body or response body. This classifier does not count as a passing
G1 tuple; it only determines whether a code compatibility fix is justified.

## CR-P12-08G1-003 — extension value-class follow-up

The first direct classifier proved that the upstream sends a valid `stop` and `[DONE]`, but also
uses three shapes outside the strict decoder contract: `choice.message`,
`delta.reasoning_content`, and Usage on a non-empty choices event. Key presence alone cannot justify
dropping either extension. One final direct Chat SSE Text classifier may repeat the same single-send
shape and add only null/empty/non-empty/type classes for the two extension fields plus a count of
Usage-with-choices events. No string content, number, ID, endpoint, model, key, request body or
response body is retained. A non-empty reasoning class forbids compatibility relaxation.

The follow-up classified `reasoning_content` as empty, Usage as sharing the sole final choice
event, and `choice.message` as a non-empty object. The first two are sufficient to justify a
closed compatibility rule; object non-emptiness alone is not sufficient to discard the message.

## CR-P12-08G1-004 — redundant-message relation classifier and strict port

The pinned CLIProxyAPI v7.2.101 native Chat translator passes each JSON SSE payload through and
therefore accepts the observed provider extension, but it does not prove that the final
`choice.message` is redundant. Before changing the Rust decoder, one final same-shape single-send
classifier may retain only the nested message key set, value classes and booleans comparing its
content with the already accumulated delta content. It retains neither compared values nor any
credential, endpoint, model, body, ID or fingerprint.

The classifier proved that the final object contains only `role` and `content`, that the role is
assistant, and that content exactly equals the prior ordered delta concatenation. The Rust decoder
may therefore accept only a final-message object whose closed fields exactly repeat the already
decoded text and completed Tool calls. Empty/null reasoning extensions may be ignored; non-empty
reasoning, refusal, unknown fields, non-final messages, mismatched text/Tools, duplicate Usage or
Usage before MessageEnd remain protocol failures. Usage on the final choices event is decoded once
after MessageEnd. Unit tests cover text, Tool and every new rejection edge before any replacement
G1 tuple is sent.

## CR-P12-08G1-005 — final summary identity compatibility

The first corrected exact-SHA runtime still produced a public Chat stream error frame; the harness
stopped after the one replacement send and completed rollback plus both key checks. A value-free
strict-invariant classifier proved that choice shape, index, logprobs, final message, Usage nested
keys and token arithmetic all satisfy the closed decoder contract. The only remaining differences
were root object and response identity. A final timing classifier proved the first semantic frames
are one stable `chat.completion.chunk` identity, while only the terminal, redundant-message frame
uses `chat.completion` and a different identity.

The decoder may accept that alternate object/identity only after a prior chunk started the stream
and only on a frame with a legal finish plus a message that exactly repeats all accumulated text
and completed Tool calls. It retains the original response identity as Canonical authority. A
summary as the first frame, without a redundant message, without finish, with mismatched semantics,
or any non-summary identity change remains rejected. The next artifact must contain this exact
rule; resumption again replaces only Chat SSE Text and never resends Chat JSON Text.

## CR-P12-08G1-006 — nullable delta absence semantics

The CR-005 artifact still stopped on the same one replacement tuple and completed rollback. The
pinned CLIProxyAPI v7.2.101 Chat-to-Responses stream fixtures explicitly contain `role:null` on
later deltas and `tool_calls:null` on the terminal Tool delta. In OpenAI Chat streaming these nulls
mean that the field contributes no new delta; CPAR previously treated field presence as a required
string/array and rejected the frame before reaching the already validated terminal summary.

The Rust decoder may treat only null `role` and null `tool_calls` as absent. A non-null role must
still be the unique initial assistant role, and non-null Tool calls must still be the bounded typed
array. Wrong types, duplicate assistant roles and all existing Tool lifecycle violations remain
rejected. This source-backed correction requires no additional diagnostic request and receives a
new exact-SHA artifact before the same failed tuple is replaced.

## CR-P12-08G1-007 — idempotent assistant role declarations

The CR-006 artifact still stopped on the same replacement tuple and rolled back. The exact decoder
predicate classifier excluded every other root, choice, delta, message, Usage and terminal rule,
then a count-only follow-up proved the four-event stream declares the same assistant role twice on
non-terminal chunks. The existing Rust state rejected the second declaration even though it carries
no new or conflicting semantic value.

The decoder may accept any number of identical string `assistant` role declarations as idempotent.
Null remains absent; every other string or value type remains rejected. The role does not create a
Canonical event, and this change does not weaken content, Tool, Usage, identity, summary or terminal
validation. The correction receives a new exact-SHA artifact and again replaces only the failed
Chat SSE Text tuple.

## CR-P12-08G1-008 — exact terminal delta de-duplication

The CR-007 artifact passed provenance and loopback installation but the same replacement tuple
still produced one public stream error frame. Rollback again restored the predecessor graph, made
the G1 key return 401, retained predecessor-key 200, and left the service active,
disabled-at-boot and loopback-only. A value-free transport classifier excluded non-data SSE fields.
The production Rust decoder then consumed a private direct response only through an in-memory pipe:
the normalized response failed at the terminal summary frame; removing Usage, minimizing the
summary Message, or restoring the baseline object/ID did not change the failure, while clearing
only the terminal delta made the exact decoder complete.

The decoder may suppress terminal text only when all three values are already proven equal in
memory: accumulated Canonical text, the terminal delta text, and the terminal summary Message text.
The accumulated text must be non-empty and no Tool call may be open. This is an idempotent duplicate,
not a second TextDelta. Any partial, suffix-only, mismatched, empty-state, Tool-bearing or differently
typed terminal delta follows the ordinary decoder and therefore still fails if it contradicts the
summary. The diagnostic retained no endpoint, key, model, request/response body, ID, text or token
value; the next exact-SHA artifact again replaces only Chat SSE Text.

## CR-P12-08G1-009 — delta-free terminal summary

The CR-008 artifact still stopped on the replacement Chat SSE Text tuple and completed the same
rollback, key and service-state checks. The earlier decoder frame number was then correlated with a
new value-free per-event sequence rather than assumed to be a delta-bearing terminal frame. It
proved two ordinary non-final chunks followed by one `chat.completion` terminal summary containing
Message, finish and Usage but no `delta` key at all. The production decoder failed because it
required `delta` before applying the already strict terminal-summary checks.

Only a terminal `chat.completion` summary may omit `delta`. It must still follow a started stream,
contain exactly one zero-index choice, a legal finish, a closed-field Message that exactly equals
the accumulated text/Tools, valid Usage when present, and the final `[DONE]`. Ordinary chunks must
retain an object delta, and an explicit null or wrong-type delta remains rejected even on the
summary. The exact event-sequence receipt stores only ordinal, field names, value classes and
identity relations; it contains no response values or Secret material.

## CR-P12-08G1-010 — required Tool choice

The CR-009 artifact made the replacement Chat SSE Text tuple pass. The harness then advanced for
the first time and stopped at Chat JSON Tool with a local 4xx. No second upstream Attempt existed,
while one direct same-shape `tool_choice:required` request returned 2xx, proving the refusal belonged
to CPAR request admission rather than the provider or credential.

Chat and Responses may accept only string `auto` or `required`; `required` is valid only when at
least one typed function Tool is present. Chat retains required under its namespaced raw extension
so the same-protocol OpenAI-compatible builder emits it without inventing a cross-protocol meaning;
Responses already retains its namespaced execution control. Object-valued named choices, unknown
strings, required-without-tools, non-function tools and foreign extension paths remain rejected.
The next exact artifact resumes at Chat JSON Tool, never resending either passed text tuple.

## CR-P12-08G1-011 — same-protocol Tool choice admission

The CR-010 artifact changed Chat JSON Tool from a decoder-owned local 4xx to a local 5xx, still
without creating an upstream Attempt. The graph was rolled back immediately; the G1 key again
returned 401, the predecessor key returned 200, and CPAR remained active, disabled-at-boot and
loopback-only. Code review located the remaining refusal in Router canonical admission: the
same-protocol request was cloned with its validated `tool_choice` extension, but the target root
extension allowlist admitted only the output-token limit and rejected the request before provider
construction.

Canonical admission may now retain only the target protocol's exact Tool choice namespace. Chat
admits string `required` only with at least one Tool; Responses admits `auto`, or `required` with a
Tool; Messages admits the closed object `{type:auto}` or `{type:any}`, with `any` requiring a Tool.
Wrong value types, unknown members, foreign namespaces and required/any without Tools still fail
closed. Provider-builder tests prove each admitted same-protocol value reaches the corresponding
wire field. Cross-protocol projection remains unchanged and rejects every Tool choice because no
reviewed lossless mapping exists. The next exact artifact resumes at Chat JSON Tool and does not
resend either passed text tuple.

## CR-P12-08G1-012 — one non-streaming Tool response classifier

The CR-011 artifact created one upstream Attempt for Chat JSON Tool, proving Router and provider
request construction now admit the tuple, but the Attempt ended `failed/StreamTruncated` in the
decoder and the public result remained 5xx. The harness stopped immediately and rollback restored
G1-key 401, predecessor-key 200, active/disabled-at-boot and loopback-only service state.

One direct non-streaming Chat Tool structural classifier is authorized using the already selected
Krill Codex credential read-only from CC Switch. It must neither rewrite nor activate any CC Switch
configuration. It sends the same bounded `tool_choice:required` shape exactly once, never retries,
and retains only status/content-type classes; closed root/choice/message/Tool/function/Usage key
sets; bounded counts; JSON value-type classes; finish-reason class; and the first strict decoder
gate that rejects. It may not retain or print endpoint, credential, model, request/response body,
IDs, Tool names, arguments, text, token counts or fingerprints. This diagnostic is not a passing G1
tuple and cannot authorize a compatibility relaxation without a closed structural proof.

## CR-P12-08G1-013 — Tool-call index relation follow-up

The CR-012 classifier returned 2xx JSON and passed every strict non-streaming Chat decoder gate
until the sole Tool call key set included `index`; all other closed key sets and value-type classes
were valid and finish was `tool_calls`. The classifier intentionally retained neither the index
value nor its relation to Tool-call order, so it is insufficient by itself to admit the extension.

One same-shape replacement classifier may send exactly once and retain only Tool-call count,
whether every index is an unsigned integer, whether indices are unique, and whether they equal the
zero-based array positions. It keeps the same read-only CC Switch and no-value boundary and cannot
retain any other response field. Only a unique zero-based relation permits the decoder to ignore
the redundant wire index in a non-streaming response; any other result remains fail-closed.

The replacement proved one unsigned, unique index equal to its zero-based array position. The
non-streaming decoder may therefore accept an absent index or an unsigned index exactly equal to
the current Tool-call position. Wrong types, null, duplicates and displaced indices remain strict
protocol failures. The streaming decoder and every other Tool field are unchanged.

## CR-P12-08G1-014 — one streaming Tool structure classifier

The CR-013 artifact made Chat JSON Tool pass, then the harness advanced to Chat SSE Tool and stopped
on one public stream error frame. Exactly two new sends occurred, so the passed JSON Tool tuple will
not be resent. Rollback again restored G1-key 401, predecessor-key 200 and active,
disabled-at-boot, loopback-only service state.

One direct Chat SSE Tool classifier is authorized with the same read-only Krill configuration and
one-send/no-retry boundary. It may retain only ordered event numbers; root/choice/delta/message/Tool
and function key sets; bounded Tool counts; null/empty/non-empty type classes; fixed finish/DONE/
Usage classes; and index unsigned/unique/zero-based relations. It may compare summary Tool fields
with accumulated deltas only as equality booleans. It must retain no endpoint, credential, model,
body, frame bytes, ID, Tool name, arguments, text, token count or fingerprint. The result only
authorizes a closed compatibility rule at the first proven decoder mismatch.

## CR-P12-08G1-015 — repeated Tool metadata relation follow-up

The CR-014 classifier observed six Tool-bearing delta events before one terminal summary. Every
Tool delta used the full `index/id/type/function{name,arguments}` key set, while the strict decoder
permits identity and name only on the first delta. Argument concatenation exactly equalled the
summary arguments, and the summary index was unsigned, unique and zero-based. The retained receipt
did not classify repeated `id`, `type` and `name` values or compare them with the first non-empty
declaration, so it cannot yet justify treating them as redundant.

One final same-shape single-send classifier may retain, per Tool delta, only the value classes of
`id`, `type` and function `name`, plus booleans stating whether each non-empty value equals the
first declaration. It may also compare the terminal summary identity/name with those first
declarations and retain the already established argument-concatenation equality. All previous
no-value, read-only CC Switch and no-retry restrictions remain. Only absent, null, empty or exactly
equal repeated metadata may be ignored; any conflicting value remains a protocol failure.

## CR-P12-08G1-016 — terminal Tool summary identity

The CR-015 classifier proved that all continuation `id`, `type` and function `name` values are
empty strings, so they carry no declaration and may be treated as absent. The terminal summary's
type, name and complete arguments exactly match the first declaration and accumulated deltas, but
its non-empty call ID differs. This mirrors the already proven terminal response-identity rewrite:
the provider reconstructs a redundant final object after the Canonical Tool lifecycle exists.

Only in that terminal redundant summary, the decoder may retain the original streamed call ID and
ignore a different summary ID when the summary ID is independently non-empty/bounded, its optional
index equals the existing Tool position, and type/name/arguments exactly match the completed Tool.
An ordinary delta with a conflicting non-empty identity/name/type, an empty or invalid summary ID,
wrong position, count, name, type or arguments remains a protocol failure. This exception emits no
second Tool event and does not alter non-streaming or cross-protocol behavior.

## CR-P12-08G1-017 — Responses JSON Text attempt-stage classifier

The independently verified `4d16c3a` artifact made Chat SSE Tool pass, advancing the fixed matrix
to four of twelve. The harness then sent Responses JSON Text once, received one safe 5xx category,
stopped, and restored the predecessor graph with both key boundaries and service state verified.

One exact same-shape, single-send/no-retry Responses JSON Text classifier may reuse the production
request builder and full tuple observer. Before any service restart it may read only the matching
request's protected loopback `GET /admin/requests/{request_id}/attempts` projection and retain the
bounded attempt count, terminal outcome and closed stage enum. It must retain no
endpoint, credential, model, URL, header, body, status number, error text, response value, token,
timestamp, identifier or fingerprint. The graph must then be rolled back and the G1/predecessor
key boundary plus active/disabled-at-boot service state reverified. A successful diagnostic does not
authorize resending any passed tuple; a repeated failure may justify only the smallest repair at
the proven stage.

## CR-P12-08G1-018 — Responses JSON Text structure classifier

CR-017 reproduced the same safe 5xx and the protected same-process projection contained exactly
one failed Attempt at the `decoder` stage. Transport, HTTP status, content type and bounded body
read therefore completed; no source change is justified until the successful upstream JSON shape
is compared with the strict Responses decoder.

One direct exact-shape Responses JSON Text classifier is authorized with the current selected
Krill configuration read from CC Switch without modifying it. It is one send with no retry,
redirect or proxy. It may retain only HTTP/content-type classes; root, output-item, content-part,
reasoning and usage key sets; bounded counts; fixed object/type/status/null classes; and booleans
for each current decoder gate. It must retain no endpoint, credential, model, request/response
value, text, ID, timestamp, token count or fingerprint. The classification is diagnostic only and
does not count as a matrix tuple. Only the first closed mismatch it proves may authorize a minimal
decoder repair; a non-2xx or ambiguous result stops without changing source.

## CR-P12-08G1-019 — Responses extension-value relation follow-up

CR-018 received 2xx JSON with a completed response, one valid text item and consistent total
Usage. The first strict mismatch is six extra root fields; the same response also contains three
extra message fields and one extra input-Usage-detail field. Key sets alone cannot prove whether
these are ignorable metadata, lifecycle semantics or cache-write accounting.

One final exact-shape, one-send/no-retry classifier may retain, for only those extra fields,
null/empty/zero/nonzero/container classes, nested container key and child-value classes, whether
`phase` equals the closed `final_answer` category, and whether `completed_at` is not before
`created_at`. It retains no scalar value, text, identifier, timestamp, token count, configuration
value or fingerprint and keeps CC Switch read-only. A nonempty cache-write count must be mapped to
Canonical cache-creation Usage rather than discarded; an unknown nonempty semantic container or
unknown phase remains fail closed. Only proven null/default/redundant metadata may be ignored.

## CR-P12-08G1-020 — nested Responses metadata relations

CR-019 proved an ordered nonzero completion timestamp, null moderation, `final_answer` phase and a
zero cache-write count. It also found finite-number candidates whose zero relation was not retained,
a known cache-retention candidate not compared with the closed supported values, nested Tool-usage
objects, and two turn-metadata objects with the same single key but no retained equality relation.

One last exact-shape single send may retain only: finite-number and zero booleans for both penalty
fields; whether cache retention is one of `in-memory|24h`; recursively bounded nested key/value
classes and whether all Tool-usage numeric leaves are zero; and whether both single-key turn
metadata objects hold the same independently valid bounded identifier. No identifier or scalar
value is retained. Nonzero Tool usage, unknown cache retention, unequal/invalid turn metadata,
nonzero penalties, or an unexpected nested shape remains fail closed.

## CR-P12-08G1-021 — bounded Responses metadata admission

CR-020 proved both penalties are finite zero, cache retention is a supported fixed category, every
numeric Tool-usage leaf is zero, and the two single-key turn metadata objects contain the same
valid bounded identifier. The already observed cache-write count is also zero. These fields add no
Canonical text, Tool lifecycle, stop reason or nonzero Usage semantics in this tuple.

The buffered Responses decoder may admit only the exact proven shapes: ordered completion time,
zero penalties, null moderation, known cache retention, the closed all-zero image/search Usage
tree, `final_answer`, matching one-key turn metadata, and zero cache-write tokens. Each field stays
optional for standard fixtures. Any nonzero count/penalty/cache-write value, unknown retention,
unknown nested key, invalid or unequal turn identity, non-null moderation or reversed completion
time remains a protocol error. No SSE or request decoder behavior changes.
