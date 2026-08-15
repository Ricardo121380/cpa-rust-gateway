# ADR-0090: Gateway-owned stored Responses public lifecycle

Status: Accepted — `P13-09B` local implementation; aggregate `P13-09` remains `IN_PROGRESS`

Date: 2026-08-16

## Context

ADR-0089 and P13-09A created an encrypted, exact-Client-Key-owned stored Response namespace, but
left the public Responses protocol unchanged. Enabling `store:true`, retrieval, and deletion adds
three correctness requirements that the storage layer alone cannot prove:

- the encrypted lineage must describe the Attempt that actually won serving selection, rather
  than a configured candidate or a later reconstruction from asynchronous logs;
- a response must not become retrievable until its complete Canonical lifecycle has validated and
  the encrypted row is durable;
- retrieval and deletion must not reveal whether the same Response ID exists for another Client
  Key, has expired, or was already deleted.

Passing the caller's `store:true` upstream would also create two competing storage authorities.
Several existing upstream profiles deliberately force or require `store:false`, and a Provider's
own retention/ownership behavior is not CPAR's Client Key ownership contract.

## Decision

P13-09B enables gateway-owned storage on the existing OpenAI Responses data plane.

1. `POST /v1/responses` accepts `store` only as a JSON boolean. Missing or `false` keeps the
   existing non-stored execution. `true` opts into CPAR storage and is removed from Canonical raw
   extensions.
2. For `store:true`, the validated native Responses payload is copied and normalized to
   `store:false` before serving. The Provider receives no request to become a second storage
   authority. No additional Provider request, retry, account conversion, or fallback is added.
3. A request-local single-assignment recorder crosses the HTTP/router composition boundary. Only
   the real routed executor advertises support and records the successful Attempt's Config
   Version, Provider, Upstream, Channel, Route, Candidate, Credential ID, and Credential revision
   directly from the selected candidate and live Credential lease. A missing or conflicting
   lineage fails closed before any stored row is created.
4. JSON and SSE use the same collected Canonical event sequence. `ResponseEnd` is withheld from
   the bounded downstream stream until `CanonicalResponse::try_new` succeeds and the encrypted
   `put_owned` transaction commits. A `StreamError`, truncation, invalid lifecycle, payload-bound
   failure, encryption error, replay conflict, or SQLite error never creates a completed record.
   For SSE, earlier semantic events may already have been delivered; storage failure therefore
   becomes one terminal failed event rather than a false `response.completed` event.
5. `GET /v1/responses/{id}` authenticates a Client Key first, performs one exact
   `(ClientKeyId, ResponseId)` unexpired lookup, revalidates the encrypted Canonical lifecycle, and
   projects JSON with the existing OpenAI Responses encoder and retained public metadata.
6. `DELETE /v1/responses/{id}` authenticates first and deletes only the exact, unexpired owner row.
   Success returns the closed `{id, object: "response.deleted", deleted: true}` projection.
7. Missing, malformed, foreign-owner, expired, and already-deleted identities return the same
   fixed `404 response_not_found` body. GET/DELETE responses and storage-specific errors carry
   `Cache-Control: no-store`.

The response ID currently exposed by CPAR is the validated `ResponseStart` identity, so it is both
the downstream lookup identity and the observed upstream response identity retained inside the
encrypted lineage. If a later protocol adapter introduces ID remapping, it must record those two
identities separately before P13-09C continuity is enabled.

This is a public inference protocol change, not a management resource. It does not change the
authoritative management OpenAPI, Prism contract/client, or management UI. A future management
storage control or status surface would require its own OpenAPI and Claude Code cross-boundary
handoff.

## Consequences

- `store:true` now means CPAR-local encrypted retention for 30 days; it does not request or claim
  Provider-native storage.
- Normal `store` omission/false requests preserve their existing forwarding and do not allocate a
  stored-response lineage recorder or write a row.
- A stored response is retrievable after restart under the P13-09A database/Master-Key contract.
- A downstream cancellation before a complete successful Canonical terminal prevents the write.
  Once a complete Provider response is durably stored, a later downstream delivery failure does
  not erase that completed owner state.
- Stored text, reasoning, Tool state, request history, Usage, stop data, and routing lineage remain
  encrypted; GET returns only the existing public Responses projection.
- `previous_response_id` and `POST /v1/responses/compact` remain rejected until P13-09C freezes
  Provider capability and exact-account continuation semantics.

## Rejected alternatives

- **Infer lineage from the event log.** Rejected because event fanout is asynchronous/value-free
  and cannot be the authority for the Credential lease that produced the returned source.
- **Pass `store:true` to every Provider.** Rejected because Provider retention does not implement
  exact CPAR Client Key ownership and some supported upstreams reject or override it.
- **Write after sending `ResponseEnd`.** Rejected because the caller could observe
  `response.completed` while no retrievable record exists.
- **Use Access Group ownership.** Rejected because two Client Keys in one group must remain
  mutually invisible.
- **Return different missing/foreign/expired errors.** Rejected because that creates a response-ID
  existence oracle.
- **Enable compact in the same slice.** Rejected because compact requires a separate capability,
  exact upstream account/Credential continuity, and gateway-owned blob contract.
