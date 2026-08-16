# ADR-0091: Exact stored Response continuity and gateway-owned compaction

Status: Accepted — formally gated by `phase-p13-responses-complete` / `d419c4678bd2ff563046849cef800c1985d48688` / run `31922870604`

Date: 2026-08-16

## Context

P13-09A/B provide encrypted, exact-Client-Key-owned completed Responses and public
create/retrieve/delete operations. Continuation adds a stronger invariant: a stored response may
only continue through the exact Config Version, Provider, Upstream, Channel, Route Candidate, and
Credential revision that produced it. Ordinary routing, sibling selection, credential conversion,
or Provider-native `previous_response_id` forwarding would violate that ownership boundary.

Compaction also carries sensitive conversation state. A compact token cannot be a caller-provided
opaque value forwarded upstream, because that would create a cross-owner/session oracle and a
second storage authority.

## Decision

1. `previous_response_id` is a gateway control, not an upstream field. The HTTP boundary performs
   an exact `(ClientKeyId, ResponseId)` unexpired lookup and never forwards the identifier.
2. CPAR replays the decrypted stored Canonical request plus the prior successful assistant output
   and appends the current turn. The resulting expanded request is bounded by the existing
   4096-event/8-MiB contract. A later `store:true` response stores the expanded request, so the
   latest completed response remains a self-contained continuation root.
3. Continuation carries an immutable exact-lineage pin into the Router. The serving runtime must
   match Config Version, Provider, Upstream, Channel, Route, Candidate, Credential ID, and
   Credential revision, revalidate Health/Quota/expiry/capacity, and execute at most once. It must
   not use route cursors, siblings, quota recovery, transparent retry, cross-egress, or
   cross-Provider fallback.
4. Endpoint/Route capabilities are explicit. `stored_responses` gates continuation and
   `response_compaction` additionally gates compaction. Compaction implies stored-response
   continuity. Grok Build declares both; Grok Web declares stored-response continuity only; Grok
   Console declares neither. Generic adapters require explicit configuration evidence rather than
   inheriting a broad capability by adapter name.
5. `POST /v1/responses/compact` accepts one owned `previous_response_id`, runs a fixed bounded
   summary request on the exact lineage, and returns a completed Responses object whose output is
   one `compaction` item. It never accepts streaming mode.
6. Compact state is stored in a distinct table and AEAD domain. The public
   `cpar_compact_v1.<opaque>` token is an owner-scoped locator, not ciphertext or an upstream
   session. Exact Client Key ownership, TTL, payload bounds, key rotation, corruption handling,
   and bounded GC mirror stored Responses without sharing AAD.
7. A compact input item is resolved locally, converted into bounded Canonical summary context,
   and pinned to the original lineage. Malformed, missing, foreign, expired, or corrupted blobs
   fail closed and are never sent upstream.
8. Stored reasoning, Tool, Usage, and stop state remain encrypted and retrievable. Replay preserves
   visible assistant text and completed Tool calls. Provider-private encrypted reasoning artifacts
   are not synthesized from plaintext reasoning; a route that requires such native state remains
   unsupported until it declares and implements a lossless capability.

This is a public Responses data-plane extension. It does not change the management OpenAPI,
Prism-generated client, management UI, production deployment, or Provider account pools.

## Consequences

- Missing/foreign/expired response or compact identities are indistinguishable at the public
  boundary.
- A stale Config Version, missing exact Candidate, changed Credential revision, unavailable exact
  account, or missing capability returns a continuity failure without trying another target.
- Compact summaries are CPAR-owned conversation state and cannot be replayed by another Client
  Key or another deployment lacking the corresponding Master Key.
- Normal Responses requests without continuation/compact retain the P13-09B path.

## Rejected alternatives

- **Forward `previous_response_id` to every upstream.** Rejected because ownership and retention
  semantics differ by Provider and would bypass exact CPAR lineage.
- **Allow normal fallback when the original account is unavailable.** Rejected because the next
  account does not own the prior Provider session or Credential-bound state.
- **Put compact plaintext in the public token.** Rejected because URLs, logs, and callers would
  gain conversation contents and owner-independent replay material.
- **Treat adapter kind as universal capability evidence.** Rejected because one adapter label may
  serve unrelated Providers and account formats.
