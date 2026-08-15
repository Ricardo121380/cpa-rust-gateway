# BC-RESP-001: Client-owned stored Response foundation

Status: `P13-09A LOCAL_PASS_PENDING_PHASE_GATE`; P13-09B/C public operations are not yet enabled

## Scope

This contract governs the Provider-neutral encrypted storage boundary introduced by P13-09A. It
does not authorize a public retrieval, deletion, `previous_response_id`, or compact route.

## Ownership

1. The durable key is the exact `(ClientKeyId, downstream ResponseId)` pair.
2. Access Group, Provider family, model alias, email, account pool, or Credential kind cannot grant
   ownership.
3. Missing, foreign-owner, and expired lookups all return absence to the future HTTP layer.
4. The store never searches another owner, account, Credential, Channel, or Provider.

## Encrypted payload

The only clear fields are owner ID, downstream response ID, creation/expiry instants, payload
version, key version, and ciphertext. All of the following remain inside AEAD:

- canonical request messages, Tools, thinking/cache extensions, and raw protocol extensions;
- complete successful canonical events, including visible text, reasoning, Tool calls/arguments,
  Usage, stop reason, and stop sequence;
- public model and response creation metadata;
- exact Config Version, Provider, Upstream, Channel, Route, Candidate, Credential ID/revision, and
  optional upstream response ID.

AEAD associated data is domain-separated and length-prefixed with the exact Client Key owner,
downstream response ID, payload version, creation instant, and expiry instant. Authentication
failure, unknown key version, malformed envelope, owner/identity/time rewrite, row swap, invalid
canonical lifecycle, or identifier mismatch fails closed and returns no partial plaintext.

## Lifecycle and limits

- TTL is exactly `2,592,000,000 ms` (30 days) from the durable creation instant.
- The expiry instant is exclusive: `now_ms >= expires_at_ms` is absent.
- The serialized plaintext maximum is 8 MiB.
- A successful stored lifecycle contains 1 through 4096 canonical events and must pass
  `CanonicalResponse::try_new`; `StreamError`, truncation, invalid order, or missing `ResponseEnd`
  cannot be stored as success.
- One GC transaction accepts 1 through 4096 rows and deletes the oldest expired identities first.
- Public callers cannot select or extend TTL, payload version, key version, or GC limits.

## Write and replay

- A new exact identity inserts one encrypted row atomically.
- Replaying identical owner, response ID, creation/expiry, payload version, and plaintext is an
  idempotent success even though fresh AEAD sealing would use another nonce.
- Reusing the identity with different durable content is `ConflictingReplay`; the original row is
  unchanged.
- No partial row is committed after input, serialization, AEAD, conflict, or SQLite failure.
- The temporary serialized plaintext byte buffer is zeroized after seal/replay comparison; opened
  envelope bytes remain in the existing zeroizing `PlaintextSecret` wrapper.

## Restart and key rotation

- Reopening the same database with the same external key ring restores exact owner state.
- An expanded key ring can read old versions and uses its active version for new writes.
- Removing a still-needed key makes the affected record unavailable; the store does not downgrade
  authentication or copy ciphertext into another owner.
- Physical expiry cleanup is bounded and independent from read-time invisibility.

## Non-goals in P13-09A

- no changes to `POST /v1/responses` parsing or `store` rejection;
- no `GET/DELETE /v1/responses/{id}`;
- no `POST /v1/responses/compact` or `previous_response_id`;
- no Provider request, lease, retry, refresh, reauth, egress change, or cross-Provider fallback;
- no management OpenAPI/Prism/UI change and no production/staging migration.

## Verification

Required evidence includes migration up/down and exact table inventory; encrypted round trip;
foreign-owner absence; expiry boundary and bounded GC; idempotent/conflicting replay; AAD row-swap
failure; ciphertext corruption; payload/lifecycle bounds; file reopen and old/new Master Key
versions; redacted Debug output; full `gateway-store` tests, strict Clippy, format, docs/secret/diff
checks, and independent review before P13-09B begins.
