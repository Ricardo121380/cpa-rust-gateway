# BC-PROTOCOL-010: Local Codex client_metadata receipt

| Field | Value |
| --- | --- |
| Contract | BC-PROTOCOL-010 |
| Task | #62 C4 only; D1-A Codex subset |
| Decision | [ADR-0116](../adr/ADR-0116-local-codex-request-metadata-diagnostics.md) |

## Source and responsibility

The observed source is unmodified Codex CLI 0.161.0, frozen [ingress](../reports/assets/cpar-62-independent-20261019/liveness-fixed/actual-cli/codex-standard/ingress.json).
Frozen official [responses_metadata.rs](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/core/src/responses_metadata.rs#L224)
lines 224–229, 325–359, 407–453 and 543–600 establish caller-owned snapshot/projections and
optional member types. [common.rs](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/codex-api/src/common.rs#L302)
defines Option<HashMap<String,String>>. Those sources establish client serialization, not target
service behavior or authentic client provenance. Any caller satisfying the closed contract is
treated identically. These are untrusted client assertions, not verified execution facts.

CPAR is the final local diagnostic recipient. It does not provide Codex server tracking,
history ingestion, approval processing or equivalent native consumption. This is a newly
approved limited request policy, not an extension of ADR-0107 response metadata separation.
Claude metadata.user_id and D2 remain excluded.

## Complete field contract

The field is optional. Absent means no capture; a present empty object is retained as such.
Present null/array/scalar or non-string values reject. Each of these seven keys is optional;
no other outer key is accepted. String values preserve Unicode and empty strings. The blob
itself must contain a JSON object, so an empty blob string rejects. The complete retained raw
field JSON is at most 65,536 UTF-8 bytes, including keys, escaping and whitespace. Every ordinary
outer/inner string is at most 1,024 UTF-8 bytes; the blob is subject to the total bound.
Oversize rejects without truncation. Duplicate JSON names reject at every parsed boundary,
including escaped-equivalent names and names inside the string blob.

| Outer member | Actual direct source | Diagnostic use |
| --- | --- | --- |
| root_turn_id | CodexResponsesMetadata.root_turn_id when Some | Client-asserted root-turn correlation |
| session_id | .session_id | Client-asserted session correlation |
| thread_id | .thread_id | Client-asserted thread correlation |
| turn_id | .turn_id when Some | Client-asserted turn correlation |
| x-codex-installation-id | .installation_id | Client-asserted installation correlation |
| x-codex-turn-metadata | turn_metadata_json()/turn_metadata_payload() | Full reviewed snapshot, retained as its original JSON string |
| x-codex-window-id | .window_id | Client-asserted window correlation |

The blob has only the following 20 optional members. No member accepts null; absence remains
absence. Empty strings are allowed for free strings; fixed selectors accept only the listed
observed value. This intentionally bounded first contract does not admit every possible Codex
snapshot. Parsing validates it but never reserializes/replaces the original string.

| Inner member | Type / admitted values | Classification and diagnostic interpretation |
| --- | --- | --- |
| installation_id | string | Untrusted installation correlation |
| session_id | string | Untrusted session correlation |
| thread_id | string | Untrusted thread correlation |
| turn_id | string | Untrusted turn correlation |
| root_turn_id | string | Untrusted root-turn correlation |
| window_id | string | Untrusted window correlation |
| context_window_id | string | Client context-window correlation; not server continuation |
| window_number | u64 JSON integer | Client window sequence, zero preserved |
| agent_name | string | Client agent label; does not select an executor |
| request_kind | string, exactly turn | Declared request kind, not a CPAR operation |
| thread_source | string, exactly user | Declared thread origin; no source/identity authority |
| turn_trigger | string, exactly exec | Declared client trigger; no execution authorization |
| sandbox | string, seatbelt or paired external | Client-reported sandbox implementation; does not prove confinement |
| sandbox_mode | string, workspace-write or paired external-sandbox | Client-reported mode; never applies CPAR sandbox policy |
| auto_review_enabled | boolean | Client-reported review state; never approval consent |
| node_repl_auto_review_required | boolean | Client-reported approval requirement; never CPAR policy |
| node_repl_disabled | boolean | Client-reported execution switch; never enables/disables CPAR tools |
| turn_started_at_unix_ms | nonnegative i64 JSON integer | Client timestamp, distinct from gateway retention/lifecycle clock |
| analytics_enabled | boolean | Client collection-state declaration; no analytics forwarding |
| model | string | Client extra/product model label; never routed/actual/native model identity |

When both a flat and corresponding inner identity are present, exact decoded string equality
is required; missing counterparts are not synthesized. request_kind/thread_source/trigger and
sandbox values above were observed, not granted general upstream semantics. model is flattened
client extra metadata, not a proven model-selection source. history_ingest_requested, compaction,
workspaces, tool_namespaces_info, parent/fork/subagent members and any unclassified key reject.
Their absence in the observed snapshot does not prove other clients never send them.

The authorized 2026-10-22 [ADR-0120](../adr/ADR-0120-bounded-codex-history-and-external-diagnostics.md)
adds only the observed pair: sandbox=external requires sandbox_mode=external-sandbox and conversely.
Original optional selectors remain. These values retain the same untrusted diagnostic meaning;
they confer no policy, confinement proof, approval or Build qualification. Missing/mixed external
pairs and all other values reject.

## Admission, persistence and isolation

Only authenticated standard Responses HTTP requests with nonempty matching route candidates
all using actual GrokBuildResponses, Responses target and Canonical/CanonicalBridge qualify.
Mixed adapters, other protocols/modes, WebSocket and unqualified compositions reject. No header,
User-Agent or generic capability can enable local consumption. The composition consumes exactly
this extension after validation, leaving all other request fields and their blockers intact.
Unconsumed client_metadata remains rejected by direct execution and Build encoding.

One RequestMetadataEvidence is confirmed alongside the original authenticated Request event in
one existing bounded queue slot/transaction, before executor/lease/send. Persisted is mandatory;
Enqueued/Disabled, missing writer/key, overflow, crypto/write/conflict/confirmation failure all
fail RecordingUnavailable without raw values in errors. A timeout is UNKNOWN even if the same
pending local transaction later commits; it never causes an upstream send or recapture.

Schema37 request_metadata_evidence is keyed by Request, with authenticated owner, original
Request event, creation/exclusive expiry, new payload version1, key version and ciphertext.
The raw field text preserves map member order/escaping/whitespace and the complete internal
string. AAD domain cpar:request-metadata:v1 plus length-prefixed owner/Request and big-endian
version/creation/expiry prevents row/owner swapping. Old payload v1 formats are unchanged.
First capture must accompany a new Request event; old events cannot acquire new metadata.
Identical replay authenticates and compares; changed raw bytes or Request/owner conflict fail.

Metadata belongs to the Request once. Its zero or more actual durable Attempt events associate
by request_id; no Attempt is synthesized or copied into the immutable capture. A valid C4 plus
another pre-send blocker can therefore have retained Request metadata and zero Attempts.
No extra Usage/ledger event, retry, source read or upstream request is produced. Client values
never generate/overwrite authenticated owner, Request/Attempt ID, route, credential, approval
decision, sandbox policy or Build tracking headers. They never enter model input, ordinary logs,
public errors, telemetry, quarantine, audit or billing. Diagnostics do not provide saved history.
For `store:true`, the stored continuation/compaction request is captured after local metadata
consumption; StoredResponse payloads contain no C4 copy and cannot extend its retention clock.

## Reader, expiry and recovery

The separate read-only SqliteRequestMetadataReader requires exact Client Key owner and Request;
it verifies AEAD and canonical durable Request indexes. An optional Attempt selector must name
an actual durable Attempt of that Request. Missing, foreign, expired and historical rows all
return UNKNOWN. Corruption and wrong/missing keys fail closed. Plaintext is available only via
authorized local database/external-key access and private exclusive output:

```sh
gateway request-metadata-evidence \
  --db /absolute/private-copy/control.sqlite3 \
  --master-key-dir /absolute/keys --active-master-key-version 2 \
  --client-key-id exact-owner --request-id exact-request \
  --output /absolute/operator-private-0700/new-request-metadata.json
```

Retention is 30 days from gateway persistence time, unavailable at now >= expiry. Bounded GC
erases at most 128 expired ciphertexts per pass while preserving Request receipts; replay cannot
renew or resurrect them, including after restart/rotation. Events and ledger retain independent
lifetimes. New writes use the active external key; retained rows require referenced old keys,
with no implicit reseal. New reader supports legacy 34–36 as UNKNOWN, current 37 authenticated;
newer schemas reject. Older binaries are not schema 37 recovery tools. Restore a coherent SQLite
snapshot including receipts/events/ledger and all keys, then validate on an isolated copy.
No destructive down migration or binary-only rollback is authorized.

## Remaining gates

C4 field-isolated success is not unmodified Codex compatibility. Namespace production activation
stays BUILD_ACTIVATION_BLOCKED; ParallelTools, C5 web_search(false), C6 non-assistant input ID,
Claude controls/D2 and #61 cost_in_usd_ticks NOT_ADMITTED remain separate. #62 stays OPEN and
the three-client first-version requirement remains. Real Provider/accounts/paid requests,
GitHub writes/push/merge, release and deployment are outside this slice.


## Corresponding tests

- `public_build_c4_metadata_is_local_and_answer_usage_are_preserved`
- `public_build_c4_stored_response_excludes_local_metadata`
- `public_build_c4_request_only_rejection_retains_no_attempt`
- `public_build_c4_rejects_unknown_invalid_duplicate_and_overbound_metadata`
- `public_build_c4_recording_faults_never_send_or_report_success`
- `public_build_c4_confirmation_precedes_execution_and_preserves_single_capture`
- `public_build_c4_metadata_keeps_two_related_tool_cycles`
- `public_build_c4_raw_presence_empty_strings_and_exact_size_boundary_are_preserved`
- `public_build_c4_nonqualified_adapters_and_modes_reject_before_send`
- `request_metadata_owner_replay_conflict_rotation_expiry_and_reopen`
- `request_metadata_ciphertext_and_source_indexes_fail_closed`
- `request_metadata_legacy_unknown_restore_and_private_cli_reopen`
- `request_metadata_missing_writer_and_confirmation_timeout_fail_closed`
- `request_metadata_actual_writer_kill_restart_preserves_capture`
