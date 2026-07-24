# ADR-0043: Revision-guarded Grok Build OAuth refresh runtime

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-22` |
| Task | `P6-02` |
| Matrix / Contract | `E25`、`E26`、`E29`; [BC-CRED-004](../contracts/BC-CRED-004-grok-build-refresh-runtime.md) |

## Context

P6-01 admits a bounded, redacted Grok Build OAuth Credential and performs one pure mock refresh.
It deliberately does not protect a real runtime against concurrent expiry. Without a durable
revision, two requests can exchange the same refresh token and a late response can overwrite a
newer Credential. Without per-Credential coordination, one expired account can produce a refresh
storm. Tokens must survive a controlled process restart but must never be readable from normal
`SQLite` rows, logs, errors, or diagnostics.

The control-plane configuration graph is immutable once published. An OAuth access-token rotation
is runtime state: it must not republish Routes, change an Endpoint selection, or mutate a compiled
`RouteSnapshot` merely because a token was refreshed.

## Decision

1. Add migration `0006_grok_build_credential_runtime`. Its exact key is bounded to non-blank
   128-byte `(config_version_id, credential_id)` components, and its mutable fields are a
   non-negative revision, AEAD ciphertext, Master Key version, and update timestamp. The table has
   no foreign key intentionally: it is a narrow
   provider-runtime sidecar, not a second control-plane owner. Its lifetime/cleanup when historical
   Config Versions are retired requires a later explicit policy rather than an implicit cascade.
   Only a validated compiled Config Version/Credential identity may be supplied by the caller.
2. Serialize the complete bounded binary Credential representation with the existing XChaCha20-
   Poly1305 `SecretStore`. Associated data length-prefixes the P6 domain, Config Version ID, and
   Credential ID, so a ciphertext copied to another runtime identity fails authentication. The
   plaintext encoding and decrypted value are zeroizing/redacted; errors contain classifications
   only.
3. Insert imported/Device Code state only if absent at revision zero. Refresh uses one atomic
   `UPDATE ... WHERE revision = expected` and increments exactly once. A CAS conflict loads the
   winner and never writes the late refresh result. A winner that is already expired becomes an
   explicit retryable concurrent-state result, never a fabricated transport failure or usable
   Credential.
4. Coordinate refreshes in-process by the exact Config Version/Credential key, not by Provider or
   account globally. One leader performs the injected OAuth refresh; same-key followers wait for
   its result for at most 30 seconds by default. A timed-out follower does not start another
   flight. P6-03 separately owns the leader's network deadline because P6-02 has no HTTP client.

## Consequences

- A process restart can reopen the sealed runtime state with the same Master Key Ring; a different
  identity or altered ciphertext fails closed.
- Older requests cannot replace a newer token, and a refresh failure/timeout does not silently
  mark the Credential unauthorized, forbidden, quota-exhausted, or disabled. P6-07 owns those
  Provider-specific classifications.
- The runtime table may retain state for a superseded Config Version until a future lifecycle
  cleanup decision. It never changes configuration rows and is not read by Router candidate
  selection.
- `provider-grok` gains direct `gateway-store` and `rusqlite` edges solely for this sealed runtime
  boundary. It creates no network client, proxy rule, real Provider request, server mutation, or
  management API.

## Alternatives considered

- Keep only an in-memory token cache: rejected because restart loses refresh state and independent
  processes cannot use a durable revision to reject stale writes.
- Use a single Provider-wide refresh lock: rejected because one blocked account would stall
  unrelated Grok Build Credentials.
- Blindly replace the token after a refresh: rejected because an old response can overwrite an
  external/newer Device Code import or refresh.
- Add an implicit control-plane foreign-key cascade: rejected because P6 does not define safe
  runtime cleanup or Config Version drain semantics; the sidecar must not invent that lifecycle.
- Convert an expired CAS winner into `TransportUnavailable`: rejected because the transport may
  have succeeded and callers need a truthful, safe retry/reload state.

## Validation and rollback

`p6_02_refresh_runtime` uses only synthetic tokens, an in-process blocking transport, and local
temporary/in-memory `SQLite`. It proves AEAD ciphertext does not retain plaintext, restart
recovery, revision CAS conflict, same-key refresh singleflight, bounded follower timeout, and that
both fresh and expired external winners cannot be overwritten. No socket, ambient OAuth config,
server, proxy, or real Provider account is used.

Rollback removes migration `0006`, the Grok runtime module, its synthetic tests, and its direct
dependency edges. It intentionally discards only this runtime sidecar; operators would re-import
or re-authorize a Build Credential after rollback. It does not alter the immutable control plane,
Routes, public API, server, or external account.
