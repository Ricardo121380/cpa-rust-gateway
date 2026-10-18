# BC-OBS-006 Complete encrypted Build Usage diagnostics

| Field | Value |
| --- | --- |
| Contract | BC-OBS-006 |
| Task | #59; parent #58 T06/T07/T08 |
| Decision | [ADR-0109](../adr/ADR-0109-encrypted-failed-build-usage-evidence.md) |

## Boundary and source

The public Responses JSON/SSE handler uses the production compiler, actual leased Build adapter,
strict native decoder and response projector. Unrepresented Usage remains rejected. Its complete,
already received, validated native Usage object takes a separate private confirmation path. This
does not enable metadata separation, summary suppression, native-v1 or a new client success case.
No reader, writer or failure handler fetches more upstream data or starts an additional request.

The raw object span is captured from the strict JSON document's `/usage`, or the strict SSE
terminal data object's `/response/usage`. It retains the original numeric lexemes, whitespace,
missing/null/0, nested arrays/objects and unknown values. Existing strict duplicate, counter,
total, lifecycle and source body/frame limits remain. One Usage companion is bounded to 64 KiB;
it is never truncated to fit. An over-bound companion cannot be acknowledged as retained.

The source labels `build_json` / `build_sse_terminal` and completeness
`complete_usage_object` describe exactly the received object. They do not assert complete
upstream execution, bytes not yet read, settlement, prices or a complete wire response.

## Persistence and failure

`emit_usage_evidence_confirmed` accepts a sanitized UsageEvent and sensitive companion, in one
existing bounded queue slot. RequestEventSink attaches the actual confirmed Attempt. The worker
resolves the exact Client Key from the admitted Request, verifies Attempt and Usage lineage,
and commits both source event and encrypted companion before returning Persisted. Queue-only
admission is insufficient. Raw data never enters GatewayEvent, telemetry, audit or quarantine.

Schema 35 stores this clear index:

| Column | Meaning |
| --- | --- |
| attempt_id | Global primary identity; one companion per exact Attempt |
| request_id, client_key_id | Actual admitted Request and exact logical Client Key owner |
| source_event_id | Existing `usage-attempt:SHA256(AttemptId)` identity shared with ledger |
| created_at_ms, expires_at_ms | Gateway retention clock; fixed exclusive 30-day expiry |
| payload_version, key_version, ciphertext | v1 record and existing AEAD envelope/key ring |

Encrypted payload v1 contains version, owner, source-event, exact UsageEvent, completeness and
`evidence: {source, raw_json}`. `raw_json` is a string containing the received Usage slice, so a
consumer can use an exact numeric parser without a JSON number conversion in the outer record.
AAD is `cpar:native-usage:v1\0` plus u32-length-prefixed owner/Request/Attempt/source-event, then
big-endian i64 version/creation/expiry. Ciphertext exchange, metadata rewriting, wrong or missing
keys and tampering cannot yield plaintext. Identical replay authenticates and compares evidence;
different raw bytes conflict even when normalized counts are equal. Expired replay cannot renew TTL.

A private recording failure is reported as existing RecordingUnavailable before the public
failure terminal; committed SSE headers stay committed and exactly one legal failure is sent.
An encryption/record conflict that leaves ordinary storage usable preserves validated counters in
a sanitized transaction, but never confirms private retention. A transient SQLite outage retains
the finite pending pair. Confirmation timeout means UNKNOWN; a later read may establish whether
the existing pending transaction committed. It cannot justify another Provider send. Known Usage,
source-event and ledger remain Attempt-scoped, non-additive and idempotent. Native prices do not
create a bill; absent pricing remains unpriced/UNKNOWN.

## Authorized reader and retention

`SqliteNativeUsageReader` is read-only and never migrates or routes. Its `get_owned` requires
exact owner, Request, Attempt and current time. Missing, foreign, expired and historical data
without a native companion return the same UNKNOWN. Successful reads authenticate AEAD and
cross-check encrypted identity/summary against the durable lifecycle/source event.
All referenced events reuse BC-OBS-002's canonical decoding and type/id/correlation/time
index checks; a damaged event index fails closed even when its payload and ciphertext remain valid.

For a local operator with authorized database and external key access:

```sh
gateway usage-evidence \
  --db /absolute/private-copy/control.sqlite3 \
  --master-key-dir /absolute/keys \
  --active-master-key-version 2 \
  --client-key-id exact-owner \
  --request-id exact-request \
  --attempt-id exact-attempt \
  --output /absolute/operator-private-0700/new-diagnostic.json
```

The output directory must already exist, be canonical and mode 0700. A descriptor-relative,
exclusive/no-follow mode-0600 output is created; existing files and symlinks are rejected.
Plaintext never goes to stdout or errors. The result has `status: retained` plus authenticated
record, or only `status: unknown`. Authentication/storage/output errors fail the command and
do not claim retention. No new management HTTP endpoint, frontend or audit body is added.

Read-time expiry applies immediately at `now >= expires_at_ms`; bounded GC removes at most
128 expired rows per transaction without touching lifecycle events or ledger. Only the evidence
retention window changes visibility. Key rotation uses the configured ring's active version for
new writes and requires all referenced old versions for retained reads; no automatic reseal occurs.

## Migration, recovery and rollback

Schema 34 -> 35 creates an empty namespace without inventing historical evidence. Sanitized old
Usage stays byte-for-byte unchanged and UNKNOWN, even when replayed with newly supplied raw data.
The new reader accepts schema 34 as UNKNOWN and schema 35 as authenticated evidence, and rejects
unsupported schemas. An old binary/reader is not qualified to read schema 35.

Minimum recovery is a coherent SQLite snapshot with accepted events, evidence and ledger, all
referenced external keys and a compatible reader. Validate file reopen, exact owner/AAD, old/new
key versions and exclusive expiry on an isolated copy. Do not apply the destructive down migration
or perform a binary-only rollback to remove retained evidence. Production restore/release still
requires separate authorization and applicable artifact/system evidence.

## #61 cancellation and conflict continuation

[ADR-0112](../adr/ADR-0112-cancelled-stream-source-release.md) separates source/lease lifetime
from a pending local confirmation. Cancellation releases network resources before finishing the
same already-started confirmation. It does not reset the observer's single-submission state,
re-enqueue a companion, read more upstream or change Request cancellation into success.
Confirmed complete Usage and the Request's incomplete delivery are distinct facts.

A public production-Build conflict test commits one complete companion and submits competing
same-Attempt bytes with identical known counters at the existing event port. The real writer
rejects/quarantines that conflict, preserves the original exact authorized readback and one
Usage/source/ledger identity, and the public stream has one RecordingUnavailable failure.
This is a controlled conflict injection, not a field admission or extra Provider request.
The user retained cost NOT_ADMITTED; successful separated-metadata timing/fault qualification
remains blocked. Schema, payload, reader, keys, TTL and settlement are unchanged.

## Corresponding tests

- `public_build_disconnect_releases_upstream_before_pending_usage_receipt`
- `public_build_evidence_conflict_fails_once_and_keeps_original_exact_usage`

- `public_responses_rejection_retains_complete_encrypted_usage`
- `public_responses_reports_private_recording_failure_without_losing_counters`
- `native_usage_exact_owner_replay_conflict_and_file_reopen`
- `native_usage_ciphertext_swap_tamper_and_key_authentication`
- `native_usage_rejects_corrupted_source_event_indexes`
- `native_usage_legacy_unknown_rotation_and_exclusive_expiry`
- `native_usage_confirmation_timeout_and_missing_writer_fail_closed`
- `native_usage_cli_reopens_authenticates_and_writes_only_private_output`
- `native_usage_encrypted_snapshot_restores_with_original_keys`
- `native_usage_missing_attempt_is_quarantined_without_blocking_new_requests`
- `worker_cleans_expired_rows_and_stops_before_next_batch`
