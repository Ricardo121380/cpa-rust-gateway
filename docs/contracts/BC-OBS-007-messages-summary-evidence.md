# BC-OBS-007 Confirmed Build summary for standard Messages

| Field | Value |
| --- | --- |
| Contract | BC-OBS-007 |
| Task | #63; parent #58 T02/T03/T05–T11 |
| Decision | [ADR-0114](../adr/ADR-0114-confirmed-build-messages-summary.md) |

## Delivery qualification

Only the actual GrokBuildResponses adapter, standard Messages target and selected CanonicalBridge
qualify. This is a limited standard delivery contract, not complete LosslessBridge. Passthrough
remains exact. No User-Agent exception exists. Responses retains summary, Chat and other adapters
remain strict, and native-v1 remains a separate promise. Request controls/history and tool constraints
are unchanged; known incompatibility is rejected before sending. Explicit standard text/tool history
is the controlled continuation mode. Diagnostics never supply hidden client history.

## Payload and confirmation

MessagesSummaryEvidence is independent of NativeUsageEvidence, GatewayEvent and billing. Payload
v1 holds source (`build_json`/`build_sse`), native response ID, local confirmation time, completeness,
and ordered `{output_index,item}` entries. An item is the complete confirmed reasoning item with
id/type/completed status and ordered summary_text parts. Missing `content` remains absent; explicit
empty content remains empty. No body, cipher, signature, citation, unknown part or extra Usage gains omission rights. A body-only
item continues through the existing strict projector; mixed summary/body remains unsupported. Empty legal summary arrays remain empty; illegal empty snapshots that
contradict deltas/done state still fail native validation. The aggregate item representation is at
most 64 KiB; it is never truncated. This preserves decoded summary semantics, not whole-wire bytes.

The request-local sink supplies its actual confirmed Attempt to one private bounded queue slot.
The Attempt event is an exact idempotent transaction anchor, not a new upstream Attempt or telemetry
observation. Before committing the companion, the worker authenticates all canonical event indexes,
owner/Request/Attempt, original execution checksum/channel and JSON/SSE source. A newly captured
summary outside the admitted request lifetime or from interrupted recovery cannot backfill history.
Same-Attempt identical evidence is idempotent; different evidence conflicts and preserves the original.

Schema 36 stores `messages_summary_evidence`: Attempt primary key, Request, exact Client Key owner,
creation/exclusive expiry, payload/key versions and ciphertext. Encrypted v1 repeats owner, original
Attempt/execution and the complete evidence. AAD is `cpar:messages-summary:v1\0`, followed by
u32-length-prefixed owner/Request/Attempt and big-endian i64 version/creation/expiry. Use the existing
key ring and active key for new writes, retain old keys for reads, and never implicitly reseal.

Only a committed transaction returns Persisted. Enqueued/Disabled, absent writer/key, encryption,
storage, identity, replay conflict or bounded receipt failure cannot release JSON success bytes or
SSE message_stop. Projection/lifecycle validation also precedes successful closure. After headers,
fail exactly once through the legal Messages error path, without a later successful terminal.

## Cancellation and Usage

Snapshot only already-confirmed items and already-validated Usage; release the upstream source and
actual lease before completing a pending local receipt. Finish that same future once, with its
single-submission flag already set. Never fetch more data, re-enqueue, retry inference or use client
termination as evidence of resource release. A native CompletedResponse summary can coexist with
cancelled/failed client delivery. ConfirmedItems asserts only the listed done items; absent unfinished
parts and unreceived Usage remain UNKNOWN. Private recording does not create a Usage or ledger row.
Known Usage uses the existing exact Attempt/source identity; no native price becomes a settlement.

## Authorized read, migration and recovery

SqliteMessagesSummaryReader opens read-only, never migrates and checks exact owner/Request/Attempt,
current exclusive TTL, AEAD/payload, original events and execution metadata. Missing, foreign,
expired and historical evidence all return UNKNOWN. Corruption, wrong/missing keys and damaged
lineage fail closed with no plaintext in errors.

```sh
gateway messages-summary-evidence \
  --db /absolute/private-copy/control.sqlite3 \
  --master-key-dir /absolute/keys --active-master-key-version 2 \
  --client-key-id exact-owner --request-id exact-request --attempt-id exact-attempt \
  --output /absolute/operator-private-0700/new-summary.json
```

The canonical 0700 directory must already exist. Output creation is descriptor-relative, exclusive,
no-follow and 0600, with owner/single-link checks and sync. Plaintext never goes to stdout, logs,
public responses, audit, telemetry, quarantine or Git. Only synthetic fixtures may be published.

Retention is 30 days with `now >= expires` unavailable; GC deletes at most 128 expired rows per
transaction, independently of event/ledger retention. Replay does not renew expiry. Migration
35 -> 36 creates an empty namespace without rewriting history. New readers treat schema 34/35 as
UNKNOWN; old binaries are not qualified for schema 36. Recovery preserves a coherent encrypted
SQLite snapshot and all external key versions with a compatible reader. Destructive down migration
and binary-only rollback are not operational recovery. Actual clients, real Provider, remote CI,
release and deployment remain separate acceptance layers.

## Corresponding tests

- `public_build_messages_json_retains_summary_before_success`
- `public_build_messages_sse_retains_summary_before_success`
- `public_build_messages_json_and_sse_keep_two_standard_tool_cycles`
- `public_build_messages_known_controls_and_histories_reject_before_send`
- `public_build_messages_unknown_parts_empty_state_and_identity_conflicts_fail`
- `public_build_messages_storage_crypto_conflict_and_confirmation_fail_closed`
- `public_build_messages_success_waits_for_confirmed_summary_receipt`
- `messages_summary_owner_replay_conflict_rotation_expiry_and_reopen`
- `messages_summary_ciphertext_swap_tamper_and_event_indexes_fail_closed`
- `messages_summary_schema35_unknown_backup_restore_and_private_cli_reopen`
- `messages_summary_requires_writer_exact_live_lineage_and_bounded_payload`

- `public_build_messages_fin_rst_cancel_headers_idle_and_keep_confirmed_items`
- `public_build_messages_cancel_pending_usage_releases_source_before_same_receipt`
- `public_build_messages_eof_idle_and_total_deadlines_fail_once_without_resend`
- `public_build_messages_nonqualified_modes_reject_before_send`
- `public_build_messages_policy_preserves_responses_and_other_adapters`
- `public_build_messages_body_only_keeps_existing_strict_projection`

- `public_build_messages_cancel_pending_summary_keeps_native_complete_delivery_cancelled`
