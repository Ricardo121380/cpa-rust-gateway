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
