# ADR-0103: Validated bootstrap Usage on its exact failed Attempt

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-10-08` |
| Task | Release-readiness `B-04 / P2`; bounded slice of #32 and #57, parent #9 |
| Contracts | [BC-OBS-001](../contracts/BC-OBS-001-bounded-request-attempt-usage-events.md), [BC-OBS-002](../contracts/BC-OBS-002-append-only-sqlite-event-writer.md), [BC-MGMT-010](../contracts/BC-MGMT-010-durable-usage-cost-operations.md), [BC-MGMT-012](../contracts/BC-MGMT-012-billing-materialization-read-model.md) |

## Context

[ADR-0100](ADR-0100-exact-protocol-semantics-and-usage-evidence.md) requires validated counters
preceding a damaged SSE frame to remain observable. Chat and Responses consume an entire body
read during bootstrap. A later frame can fail that read after the decoder cached valid Usage,
before a source is returned to the HTTP observer. Messages stops decoding as soon as its legal
`message_start` queues a start event; its observer is attached before the later damaged frame.

A failed bootstrap can still use the existing legal retry policy. Request-scoped Usage identity
would then reject the successful Attempt's separate Usage, or attribute one to the other. Explicit
Attempt lineage already exists; this decision changes its durable identity and aggregation, without
adding event fields, schema tables, a new service, or a price inference rule.

## Decision

1. Each request-local endpoint driver clears a single bounded failure-Usage slot before its next
   invocation. A failing Chat/Responses bootstrap takes only the decoder's validated Response ID
   and merged known Usage. The original error and retry classification are preserved. No upstream
   read or request is added to collect counters. Missing or invalid Usage produces no record.
2. The orchestrator first confirms the actual failed Attempt, then takes and confirms its Usage
   through the existing Required event port, attaching that exact Request/Attempt identity. Both
   confirmations precede any legal fallback. A recording failure stops execution with
   `RecordingUnavailable`. No successful source, completed response or stored/session root is
   created by this recovery of evidence. Success and post-handoff failure/cancellation retain the
   existing observer path.
3. New explicit Usage uses `usage-attempt:<SHA256(AttemptId)>` as its stable source-event ID.
   Each actual Attempt has at most one final merged snapshot; identical replay is a no-op and a
   changed snapshot for the same ownership conflicts. Different explicit Attempts can each retain
   their own final Usage even if the upstream reuses a Response ID. Legacy request-scoped IDs and
   Response IDs retain their exact replay identity. An unknown legacy lineage cannot be enriched
   or arbitrarily assigned to a new Attempt by replay.
4. Store traversal, in-memory usage compilation, billing materialization and ledger quarantine
   agree on this ownership. Explicit failed Attempts are valid sources. Legacy Usage without an
   Attempt ID retains the previous highest-successful-Attempt rule; missing or conflicting lineage
   fails closed or is explicitly excluded by the existing partial-read policy. Different explicit
   Attempts do not conflict merely because their Request is shared.
5. Account/model usage groups aggregate known facts across distinct Attempts. `request_count`
   counts distinct Requests within a group, while `usage_observations` counts distinct Attempt
   snapshots. Ordered Request traversal permits constant-memory counting within each group.
   Snapshot merging uses the existing non-additive canonical rules. All six nullable counters,
   provenance and input accounting survive to the ledger. Unknown stays unknown; estimated or
   incomplete facts do not establish an exact bill.

## Consequences and alternatives

The router retains no Store dependency; the existing bounded queue and background SQLite writer
confirm durable events, and the existing billing worker independently materializes them. Request
failure remains failure. Preserving all known counters is separate from obtaining final complete
upstream Usage. Responses explicit protocol contradictions remain non-retryable; ordinary
transport EOF still permits the previously legal fallback. This supersedes request-only cardinality
and highest-successful-only wording in the linked historical contracts for explicit lineage.

A generic failure carrier/framework, synchronous request-path SQL, guessed counters/ownership and
an upstream replay to recover Usage are unnecessary or violate the evidence boundary. Attributing
the failed Attempt to the final successful Attempt is rejected because it falsifies both accounts.

## Validation and rollback

Public gateway tests use actual loopback bodies and record the exact bytes supplied to decoder
`push` in the first read. The same admitted events pass through the production fanout, bounded
queue and async SQLite writer; the production billing worker runs against that file. Protected
management HTTP uses a test facade over the production read-only Store/control readers. These
tests do not claim process-restart recovery or the entire deployment composition. Tests cover
known/absent/invalid Usage, snapshots, duplicate confirmation, real usable fallback reception,
same upstream Response ID, failed/successful lineage and Messages' earlier handoff. Existing
success/failure/cancellation, legacy identity and malformed-lineage tests remain required.

No schema migration is needed. Old binaries reject the new source-event identity, so binary-only
rollback after new events is unsafe. A deployment requires its own compatible artifact and full
state snapshot/rehearsal; preserve new accepted history before any authorized restoration.
This local fix grants no release, real-Provider, remote-CI or production acceptance.
