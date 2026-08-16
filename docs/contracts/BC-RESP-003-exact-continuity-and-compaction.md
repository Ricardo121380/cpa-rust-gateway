# BC-RESP-003: Exact stored Response continuity and compaction

Status: `P13-09C LOCAL_PASS_PENDING_PHASE_GATE`

## Scope

This contract governs `previous_response_id`, `POST /v1/responses/compact`, and gateway-owned
compaction input items on the Client-Key-authenticated public Responses surface. BC-RESP-001/002
remain authoritative for encrypted stored responses and create/retrieve/delete durability.

## Admission and ownership

1. `previous_response_id` is optional, non-empty, NUL-free, and at most 512 bytes. It is mutually
   exclusive with a compact input item.
2. Lookup is exact by authenticated Client Key plus response/blob identity. Missing, malformed,
   foreign, expired, deleted, corrupt, or unavailable owner state is not sent to a Provider and
   cannot reveal whether another owner has the same identifier.
3. Requested public model must equal the stored public model. Conversation ownership never moves
   through an alias, sibling model, Provider, account, Credential format, or egress fallback.
4. Continuation requires `stored_responses`; compact requires both `stored_responses` and
   `response_compaction` on the exact Candidate.

## Exact execution

- The immutable pin contains Config Version, Provider, Upstream, Channel, Route, Candidate,
  Credential ID, and Credential revision from the encrypted record.
- Immediately before leasing, the current runtime must match every field and revalidate hard
  eligibility, capability, Health, Quota, expiry, and capacity.
- Selection is direct by Candidate/Credential and does not advance ordinary weighted cursors.
- One admitted continuation starts at most one Attempt. Transparent retry, sibling fallback,
  quota recovery, cross-egress, cross-Provider, and credential conversion are prohibited.
- The exact Attempt continues to use the normal canonical response lifecycle, value-free event
  projection, runtime failure ownership, lease RAII, and downstream cancellation boundary.

## Canonical history

- The previous stored request and successful assistant output are replayed before the current
  messages. Visible text and complete Tool calls retain order and exact call IDs/arguments.
- Usage, reasoning, stop reason/sequence, and the complete previous events remain encrypted in the
  source record and retrieval projection. Plaintext reasoning is never fabricated as a portable
  upstream reasoning token.
- The expanded request must remain inside the fixed 8-MiB serialization boundary. A successful
  `store:true` continuation stores that expanded request as the next self-contained root.

## Compaction

1. `POST /v1/responses/compact` is non-streaming and requires one exact owned
   `previous_response_id` plus the current public model. Unknown fields and ambiguous input fail
   before execution.
2. The fixed bounded summary request is executed once on the exact lineage. Incomplete,
   `StreamError`, empty, or oversized summaries create no compact row.
3. The result contains exactly one `compaction` output item with an opaque
   `cpar_compact_v1.*` token. It contains no plaintext summary, endpoint, Credential, URL, Secret,
   or client-key digest.
4. The compact row uses a separate AEAD domain, exact Client Key owner, fixed TTL/read-time
   expiry, independent AAD, bounded replay/conflict behavior, restart/key-rotation support, and
   bounded GC.
5. A later compact input item is resolved locally and never forwarded as an unknown upstream
   blob. Foreign/missing/expired/corrupt tokens fail closed before lease/network.

## Provider capability matrix

| Channel | stored-response continuity | compaction |
|---|---:|---:|
| Grok Build | yes | yes |
| Grok Web | yes | no |
| Grok Console | no | no |
| Other/generic adapters | only with explicit capability evidence | only with explicit capability evidence |

## Non-goals

- no Provider-native response retrieval/deletion;
- no cross-account continuation or implicit credential refresh/reauth;
- no management OpenAPI/Prism/UI change;
- no production/staging deployment or real Provider probe in this local slice.

## Verification

Evidence must cover strict decoding; capability implication/matrix; exact Candidate and Credential
revision leasing without cursor mutation; Config/Provider/Channel/model mismatch; owner isolation;
history text/Tool ordering; encrypted reasoning/Usage/stop retention; compact AEAD/AAD/TTL/restart/
key rotation/corruption/GC; compact blob reuse and foreign rejection; no retry/fallback; complete
JSON/SSE/store regression; strict Clippy/format/docs/source/crate/secret/diff checks; and a final
structured review.
