# ADR-0109: Encrypted native Usage evidence on rejected Build Attempts

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | `2026-10-18` |
| Task / Matrix / Contract references | #59, parent #58 T06/T07/T08; ADR-0003/0027/0100/0103/0105/0107; BC-OBS-001/002 and BC-PROVIDER-003 |

## Context

A public Build Responses call correctly rejects unrepresented Usage. Its validated counters
survive, but the native object is currently reduced to UsageSummary before persistence. The
ordinary event log is deliberately value-free. Saving a rounded Value serialization or merely
admitting a queue entry would not establish complete private retention.

## Decision

Preserve the exact bounded Usage object slice from the already received JSON document or SSE
terminal event, after existing strict decoding and counter validation. Object member order and
whitespace are retained as received; the containing HTTP envelope is not retained by this feature.
Missing fields, null, zero, nested values and numeric lexemes remain distinct. This slice adds no
field admission, downstream extension, new read, retry, request or client delivery strategy.

Use a dedicated sensitive confirmation method on the existing event port. The ordinary event
remains the same sanitized UsageSummary. Its optional private companion occupies the same bounded
queue slot and pending batch, is excluded from telemetry/quarantine payloads, and is sealed only
on the existing blocking SQLite worker. A persistence receipt requires the transaction containing
both the source event and encrypted companion to commit. No attached writer/key, queue pressure,
write failure, timeout, invalid lineage, conflict or authentication failure can acknowledge retained
evidence. The public call still fails; recording failure is explicit through RecordingUnavailable.

Schema 35 adds `native_usage_evidence`, keyed by exact Attempt, with exact Request, Client Key,
source-event identity, format/key versions, creation/expiry and ciphertext. Resolve owner from the
durable admitted Request and verify that the durable Attempt belongs to it. The encrypted v1
payload repeats that identity, the response identity, verified summary and source (`build_json`
or `build_sse_terminal`), and labels completeness `complete_usage_object`. This asserts completeness
of that received object, not completeness of the request/Provider bill or of unread upstream data.
Domain-separated, length-prefixed AAD authenticates owner, Request, Attempt, source-event, version
and lifecycle timestamps. Identical replay authenticates and compares the original evidence;
different evidence or ownership conflicts, including an identical summary with changed raw bytes.

Reuse SecretStore/XChaCha20-Poly1305 and the configured external MasterKeyRing. New records use the
active key. Old keys remain necessary for retained rows; no implicit re-encryption is performed.
Retain evidence for the existing local 30-day window, with exclusive read-time expiry and bounded
physical GC. Replays never extend TTL or resurrect expired evidence. Lifecycle events/ledger remain
independent and are not deleted by evidence GC.

Provide a read-only, owner-scoped repository and a local `usage-evidence` CLI. Local operator
authorization requires access to the database and matching external keys, plus explicit owner,
Request and Attempt selection. It exposes no new HTTP endpoint. Plaintext is written only to a
new exclusive mode-0600 output file in an operator-owned mode-0700 directory, never stdout,
ordinary events/logs, management audit or public errors. Missing, foreign and expired records yield
the same UNKNOWN result. Historical records without companions remain UNKNOWN; replay must not
backfill old source events from current inventory.

## Consequences

The failed standard request can have confirmed complete private Usage without becoming a successful
client response. Its existing single Attempt-scoped source event and ledger keep all verified
counters; native price fields remain non-settlement evidence and cannot establish a Provider bill.
A confirmation timeout is UNKNOWN even if the bounded writer later commits; authorized readback
resolves retention without sending anything upstream.

## Alternatives considered

- Full native JSON in gateway_event_log or audit: violates the ordinary event privacy contract.
- A separate unbounded writer or synchronous SQL in HTTP: violates existing bounds and ownership.
- Value/to_string or UsageSummary as evidence: loses numeric lexemes, presence or unknown fields.
- Relax projection after saving: would implement a later ticket's delivery policy without admission.

## Validation and rollback

Use public Responses JSON/SSE through the production compiler/factory/Build adapter with owned
loopback upstream receive counts; retain rejection RED and verify exact authorized readback,
source/ledger idempotence, safe errors and no extra reads. Supplement with store boundary tests for
owner, row swapping/tampering, replay/conflict, storage/confirmation failures, expiry, file reopen,
schema-34 UNKNOWN migration and old/new/wrong/missing key authentication. Only synthetic material
is used; this does not qualify real Provider, remote CI, release, deployment, #60 or the parent spec.

An older reader may read its supported legacy schema without evidence and report UNKNOWN; it must
reject an unsupported newer schema. After schema-35 records exist, binary-only rollback is unsafe.
Minimum recovery preserves a coherent SQLite snapshot (including accepted events, ledger and
evidence), all referenced external key versions and a schema-35-compatible reader. Verify reopen,
owner/AAD, expiry and keys on an isolated copy before any separately authorized recovery. Never run
the down migration against retained evidence as an operational rollback or erase historical FAIL.
