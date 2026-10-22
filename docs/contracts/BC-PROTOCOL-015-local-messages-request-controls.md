# BC-PROTOCOL-015: Local Messages request controls

| Field | Value |
| --- | --- |
| Contract | BC-PROTOCOL-015 |
| Task | #64 M3/M4, D1-A Claude subset only |
| Decision | [ADR-0121](../adr/ADR-0121-local-messages-request-controls.md) |

## Sources and responsibility

Observed input is the original Claude Code 2.1.302 request in the
[actual-client report](../reports/cpar-64-claude-messages-20261020.md) and its raw request archive.
Current primary definitions were checked on 2026-10-22:
[Messages metadata](https://platform.claude.com/docs/en/api/messages/create) defines optional
user_id, opaque external association and maxLength512; the upstream schema also allows null.
[Context editing](https://platform.claude.com/docs/en/build-with-claude/context-editing) defines
keep:all and JSON/SSE applied_edits placement. These define source semantics, not fixed Build
service support. CPAR deliberately admits only the narrower subset below.

Only authenticated standard Messages HTTP, actual GrokBuildResponses, Responses upstream and
CanonicalBridge qualify. All exact-model matching candidates must qualify before M4 consumption;
empty/mixed/other adapter, target or mode sets reject. No User-Agent/header/request switch confers
qualification. No new generic capability, native field allowlist or D2 profile is introduced.

## M4 complete field

| Input | Result |
| --- | --- |
| metadata absent | No capture; absence remains absence |
| metadata={} | Capture complete empty object as Claude v2 |
| metadata.user_id absent | Accepted only as the empty object |
| user_id="" | Capture unchanged empty string |
| metadata null/array/scalar, or user_id null/non-string | Reject, no retention promise |
| Any other metadata member | Reject, even when user_id is valid |
| Duplicate root/metadata names, including escaped-equivalent names | Reject before Value merging |
| Unicode/escapes/whitespace | Preserve original field JSON and decoded string exactly; no normalization |
| user_id longer than512 Unicode scalar values | Reject without truncation; UTF-8 is at most2048 bytes |
| Complete raw metadata JSON longer than65,536 UTF-8 bytes | Reject including whitespace, keys and escaping |

The field value is the original JSON token, including interior whitespace/member order/escapes;
whitespace outside that value token is HTTP framing, not part of metadata. User strings that
contain JSON remain opaque strings: even duplicate names or device_id/account_uuid/session_id
inside them are not parsed or interpreted. Client identifiers never derive authenticated owner,
Request/Attempt, permissions, credential/session headers, route or model identity.

CPAR is the final local diagnostic recipient. It does not send metadata to Build/Anthropic,
model input, tracking headers or public responses, and supplies no upstream association,
abuse-detection input, continuation, authentication or routing service. Ordinary event/log/audit,
telemetry, quarantine and billing payloads remain free of the private field.

## M3 identity subset

The complete admitted value is {"edits":[{"type":"clear_thinking_20251015","keep":"all"}]}.
Exactly one edit, both members required, no extra keys/strategies. Missing/null/other keep values,
extra edits, unknown type or malformed content reject. Duplicate keys reject before merging.
The complete effective history must pass existing call/result correlation and contain no
Reasoning, Opaque, thinking, redacted_thinking or other unrepresented native reasoning. Tool
results are string text or closed text-block arrays; arbitrary native blocks are not text.
Tool arguments and text strings remain user data, not recursively interpreted native blocks.

Recheck every request, including later tool rounds. Keep all visible messages, roles, sequence,
tool calls/results and associations under the existing exact bridge. Consume the policy locally;
do not clear, reorder, synthesize placeholders or send a pretend Build context-management field.
Only a successful requested M3 identity rule adds context_management:{applied_edits:[]} to the
JSON response root and final SSE message_delta root. No cleared-turn/token statistics or native
execution receipt is invented. Absent M3 leaves the existing response shape unchanged.

## Persistence, reader and recovery

One original authenticated Request and private evidence commit atomically through the existing
finite queue before any lease/send. Only Persisted releases execution. Missing key/writer,
overflow, lost/timeout confirmation, crypto/write/conflict failures return RecordingUnavailable;
no upstream request, retry or duplicate capture follows. Confirmation timeout is UNKNOWN even
when the same pending local write subsequently commits. Valid M4 plus M1/M2/M5 or invalid M3
can retain Request-only evidence with zero actual Attempts.

V1 is unchanged Codex C4. V2 explicitly labels anthropic_messages_metadata and binds the original
AnthropicMessages Request. Schema38 permits only payload versions1/2. V2 AAD starts with
cpar:request-metadata:v2 followed by NUL and length-prefixed source, protocol, owner and Request,
then big-endian i64 payload version/creation/exclusive expiry. V1 AAD is unchanged. Payload source,
version and durable protocol must agree. Other protocols cannot relabel captured data.

SqliteRequestMetadataReader/private gateway request-metadata-evidence output retain the existing
exact-owner and optional actual-Attempt checks, read-only behavior, exclusive0600 output under
canonical0700 directory, key rotation and 30-day exclusive TTL. Missing/foreign/expired/historical
is UNKNOWN; corrupt lineage/source/AAD/keys fails closed. Identical replay preserves original TTL;
changed raw bytes conflict. GC erases at most128 ciphertexts and retains all expiry receipts.

Schema37 migration preserves old rows/ciphertext/receipts; new reader reads v1 and v2, legacy34-36
remains UNKNOWN. Old readers reject schema38. Restore coherent events/execution/ledger/evidence/
receipts plus all keys, then verify a new real process and authorized readback. No backfill,
reseal, historical UNKNOWN rewrite or destructive rollback. #65's unrelated matrix is not repeated.

## Acceptance boundary

M3, M4 and combined public JSON/SSE loops are independent local duties. Original Claude still
may return400 for M1/M2/M5; prove its Request-only M4 retention separately without editing client
requests. Keep Codex C4/strict Codex/Pi, #63 summary, #59 failed Usage, cancellation/release and
no-resend regression. Full binds immutable source/manifest/lock and explicit inherited overlay.
Real Provider, remote CI, release/deployment and #66/#67 remain outside this slice.

## Corresponding tests

- `public_build_messages_m3_identity_is_local`
- `public_build_messages_m4_metadata_is_private`
- `public_build_messages_local_controls_keep_two_tool_cycles`
- `messages_metadata_v2_owner_recovery_and_legacy_coexistence`
- `native_metadata_keeps_original_whitespace_escapes_and_opaque_string`
