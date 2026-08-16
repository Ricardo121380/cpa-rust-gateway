# BC-RESP-002: Stored Response public lifecycle

Status: `P13-09B DONE_WITH_BOUNDARY`; aggregate formal Gate `31922870604` passed

## Scope

This contract governs `store:true`, `GET /v1/responses/{id}`, and
`DELETE /v1/responses/{id}` on the Client-Key-authenticated public data plane. It builds on
BC-RESP-001 and does not authorize `previous_response_id` or compact.

## Create admission and upstream ownership

1. `store` is optional and accepts only JSON `true` or `false`; another type is a client request
   error before routing or Provider execution.
2. Missing/false is the existing non-stored path. It does not create a lineage recorder and does
   not write a stored-response row.
3. `store:true` requires both an encrypted repository and an executor that explicitly supports
   exact successful-attempt lineage. Missing support fails closed before executor start.
4. The native Responses payload for `store:true` is normalized to `store:false` before Provider
   execution. The Canonical extensions do not retain `store` as an upstream option.
5. This feature adds no Provider call, transparent retry, sibling selection, account conversion,
   or cross-Provider fallback beyond the already-authorized serving execution.

## Exact successful lineage

- The request-local recorder is single-assignment. Identical replay is harmless; conflicting
  lineage is an internal failure.
- The production routed executor records only after the final serving Attempt has selected a live
  Credential lease and returned a source.
- The encrypted record contains the exact Config Version, Provider, Upstream, Channel, Route,
  Candidate, Credential ID/revision, and observed upstream response identity.
- No HTTP handler, event-log scan, management projection, configured-first candidate, or account
  sibling may synthesize the winning lineage.
- Recorder Debug output is value-free and must not expose any opaque identity.

## Completion and durability ordering

- JSON and SSE accumulate the same bounded Canonical sequence used for public projection.
- Only a sequence accepted by `CanonicalResponse::try_new` and ending in `ResponseEnd` can be
  passed to `put_owned`.
- The encrypted transaction must commit before `ResponseEnd` enters the bounded downstream
  stream. Consequently JSON cannot return a completed object, and SSE cannot emit
  `response.completed`, before durability succeeds.
- `StreamError`, source EOF/truncation, invalid order, cancellation before completion, payload
  bounds, encryption/store failure, and conflicting replay do not create a completed row.
- If SSE already delivered earlier events and persistence then fails, it terminates with the
  existing safe failed event; it cannot emit a completed event.
- Usage, Tool, reasoning, stop reason/sequence, request history, and public metadata in GET are the
  same encrypted Canonical state, not a reconstructed log summary.

## Retrieval and deletion

1. Both operations authenticate with the existing public Client Key boundary before parsing or
   looking up owner state.
2. IDs are non-empty, NUL-free, and at most 512 bytes before repository work.
3. GET performs one exact unexpired `(ClientKeyId, ResponseId)` lookup, authenticates/decrypts the
   envelope, revalidates the successful lifecycle, and returns the existing OpenAI Responses JSON
   projection with retained public model and creation time.
4. DELETE removes one exact unexpired owner row and returns exactly
   `{id, object: "response.deleted", deleted: true}`.
5. A foreign Client Key cannot retrieve or delete the owner row, including when both keys share an
   Access Group.
6. Missing, malformed, foreign, expired, and deleted IDs use the same fixed 404 projection. A
   foreign delete does not modify the owner's row.
7. GET, DELETE, not-found, authentication, and storage-specific error responses are not cached.

## Persistence and restart

BC-RESP-001 remains authoritative for AEAD fields/AAD, 30-day TTL, read-time expiry, 8-MiB and
4096-event bounds, bounded GC, replay/collision behavior, file reopen, key rotation, corruption,
and row-swap failure. P13-09B opens that same repository in the production composition with the
existing external `SecretStore`; it does not create a second database or key source.

## Non-goals

- no `previous_response_id` parsing or exact-account continuation;
- no `POST /v1/responses/compact` or compact blob;
- no Provider-native stored-response retrieval/deletion;
- no management endpoint, management OpenAPI/Prism/UI change, or billing/event-log payload write;
- no production/staging rollout or real Provider probe in this local slice.

## Verification

Required evidence includes strict `store` decoding and upstream normalization; opt-in/no-write;
missing-lineage pre-execution rejection; exact routed lineage; successful JSON and SSE durability;
StreamError/truncation no-store behavior; owner GET/DELETE projection; foreign/missing/deleted
equivalence; foreign DELETE non-mutation; authentication/no-store headers; P13-09A restart/TTL/AEAD
regressions; existing Responses/Chat/Messages tests; strict Clippy/format/docs/source-policy/crate
boundaries/secret/diff checks; and a structured final review before task closeout.
