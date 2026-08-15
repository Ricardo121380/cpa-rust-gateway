# ADR-0089: Client-owned encrypted stored Responses

Status: Accepted — `P13-09A` local implementation; aggregate `P13-09` remains `IN_PROGRESS`

Date: 2026-08-16

## Context

The public data plane currently implements `POST /v1/responses`, but deliberately rejects
`store`, `previous_response_id`, retrieval, deletion, and compaction controls. Existing durable
gateway events retain value-free lifecycle observations only. Grok Build continuity retains an
account binding and encrypted replay state, but it is Provider-specific and does not contain a
complete protocol-neutral response. Neither boundary can safely become a generic stored-response
repository.

P13-09 must eventually support retrieval and compact without weakening these existing rules:

- Client Key ownership is exact; Access Group membership is not sufficient ownership.
- response text, reasoning, Tool arguments/results, usage, stop data, request history, Provider
  lineage, and upstream response identifiers are sensitive payload.
- `previous_response_id`, retrieval, deletion, and compact must never switch Provider, account, or
  Credential when the owner is unavailable.
- partial, truncated, or `StreamError` lifecycles cannot be presented as a completed stored
  Response.
- persistence may not reuse the append-only value-free event log.

## Decision

P13-09 is delivered in three explicit slices.

1. **P13-09A — encrypted foundation.** Migration `0017` creates an independent
   `stored_responses` namespace and `SqliteStoredResponseStore`. It provides exact Client Key
   ownership, a fixed 30-day TTL, read-time expiry, bounded GC, idempotent exact replay, conflict
   rejection, restart recovery, and external Master Key rotation compatibility.
2. **P13-09B — public store/retrieve/delete.** Only after A is reviewed may `store:true`,
   `GET /v1/responses/{id}`, and `DELETE /v1/responses/{id}` be enabled. Successful JSON and SSE
   lifecycles must durably store before claiming retrievability. Missing, foreign-owner, expired,
   and deleted IDs must share one safe not-found projection.
3. **P13-09C — continuity and compact.** `previous_response_id` and
   `POST /v1/responses/compact` require an explicit Provider capability and exact stored lineage.
   They do not use implicit sibling, account, Credential, egress, or Provider fallback.

P13-09A stores only the following clear columns:

- `client_key_id` and downstream `response_id` for exact ownership lookup;
- `created_at_ms` and exclusive `expires_at_ms` for lifecycle/GC;
- payload/key format versions and opaque ciphertext.

The AEAD plaintext contains the canonical request, successful canonical event sequence, public
model metadata, exact Config Version, Provider/Upstream/Channel/Route/Candidate, Credential ID and
revision, and optional upstream response ID. Associated data is domain-separated and
length-prefixed with Client Key ID, response ID, payload version, creation time, and expiry time so
rows cannot be swapped and clear lifecycle metadata cannot be extended or rewritten undetected.

The fixed bounds are:

- 30-day local TTL; clients cannot extend it;
- at most 4096 canonical events;
- at most 8 MiB serialized plaintext;
- at most 4096 rows per GC transaction;
- opaque IDs are independently bounded before SQL or AEAD work.

The store is Provider-neutral and performs no routing, lease acquisition, Provider call, retry,
refresh, compaction, or account conversion. A process can read ciphertext written under an older
Master Key only when that key remains present in the external key ring; new writes use the active
version. The temporary serialized plaintext buffer is zeroized after the immediate seal/compare
operation; decrypted envelope bytes already use the existing zeroizing `PlaintextSecret` boundary.

## Consequences

- Public retrieval/compact is not exposed by P13-09A; current request rejection remains intact.
- Sensitive Response state is not written to `gateway_event_log` or management audit rows.
- Identical replay is safe, while response-ID collision with different content fails closed.
- Expired state becomes invisible immediately at read time; physical removal is a separate bounded
  operation and cannot change public existence behavior.
- Backup/restore of stored Responses requires the matching external Master Key ring. A missing key,
  modified ciphertext, malformed envelope, or row swap fails closed.
- P13-09B must capture exact successful-attempt lineage and prove JSON/SSE durability ordering;
  P13-09C must freeze capability-specific compact semantics before any Provider request is enabled.

## Rejected alternatives

- **Store full Responses in `gateway_event_log`.** Rejected because that log is value-free,
  append-only, and has no TTL/ownership deletion contract.
- **Reuse Grok Build continuity tables.** Rejected because they are Provider-specific and do not
  contain a complete canonical Response.
- **Keep response content in plaintext SQLite columns.** Rejected because reasoning, Tool state,
  messages, and Provider lineage are sensitive.
- **Use Access Group as the owner.** Rejected because two Client Keys in one group must not retrieve
  each other's state.
- **Expose GET/compact before persistence is reviewed.** Rejected because it would create an API
  whose ownership, expiry, restart, and durability behavior could not be guaranteed.
