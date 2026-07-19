# BC-AUTH-002 Client Key HMAC credential

| Field | Value |
|---|---|
| Contract | `BC-AUTH-002` |
| Task | `P2-04` |
| Status | DONE |
| Domain | Client Key issuance, storage-safe HMAC verification, and lifecycle admission |

## Entry and boundary

`gateway-auth` supplies a Client Key issuance/verification primitive independent of `SQLite`,
Actix, HTTP headers, and RouteSnapshots. It produces the logical fields for P2-02's `client_keys`
row but does not insert, query, publish, or expose them. P1's live in-memory
`ClientKeyAuthenticator` remains unchanged; P2-08 adapts compiled persisted records to that port.

## Preconditions

- A Client Key Pepper is a dedicated exact 32-byte raw external regular file, never an upstream
  Secret Master Key. The loader rejects a symbolic link, a non-regular path, read failure, or any
  length other than 32 bytes.
- A caller supplies non-empty `ClientKeyId` and `AccessGroupId` values, plus an optional
  non-negative expiry timestamp in milliseconds. Persistence later enforces a unique Prefix and
  digest inside one Config Version.
- The verifier receives the complete presented Key, one Prefix-selected record, and a
  non-negative current timestamp. It returns only a success/failure result to its caller.

## Issuance and verification sequence

```text
issue(client_key_id, access_group_id, optional expiry)
  -> OS randomness: 8-byte public Prefix + 32-byte Secret
  -> complete key: rgw_<16 hex Prefix>_<64 hex Secret>
  -> HMAC-SHA256(Pepper, exact complete key bytes)
  -> persistable record: id, access_group_id, Prefix, 32-byte digest, active, expiry
  -> non-cloneable redacted one-time complete-Key result

verify(presented key, Prefix-selected record, now)
  -> parse canonical ASCII form and compare public Prefix
  -> HMAC-SHA256(Pepper, exact presented key bytes)
  -> constant-time 32-byte digest comparison
  -> require active status and now < expiry when expiry exists
  -> return true only when every check passes
```

## Invariants

- A complete Key is never a field of the persistable record. Its exact HMAC digest is exactly 32
  bytes, and the Prefix is the `rgw_<16 lowercase-hex>` component, not the Secret.
- The complete-Key wrapper and Client Key Pepper have redacted `Debug` output. The Pepper and
  presented wrapper zeroize on drop, and temporary random/encoded Secret buffers are zeroized
  before issuance returns. HMAC operations do not retain the complete Key in a record, error, or
  custom debug representation. Error text does not contain a complete Key, Pepper bytes, digest
  bytes, or a raw external file's contents.
- The nonce-like random values are generated internally. Callers cannot select the Prefix or
  Secret through the normal issuance API.
- A different Key, malformed form, wrong Prefix, invalid digest length, wrong Pepper, disabled
  record, revoked record, or `now >= expires_at_ms` all fail without revealing which condition
  caused rejection. Digest calculation and constant-time comparison occur before lifecycle status
  can make an otherwise matching record succeed.
- Client Key Pepper rotation, record persistence, duplicate-Prefix retry, Snapshot compilation,
  `Authorization` parsing, live authenticator replacement, Access Group enforcement, and API/CLI
  one-time display remain P2-05, P2-08, and P2-10 work.

## Error semantics

```text
malformed external Pepper file, invalid expiry/current timestamp, or invalid persisted digest
  -> safe local configuration/record error; no key material in text

malformed/unmatched/disabled/revoked/expired presented key
  -> verification false; caller maps all cases to the existing safe ClientUnauthorized behavior

OS-randomness or HMAC initialization failure
  -> safe issuance/verification infrastructure error; no created record or complete key returned
```

## Corresponding tests

- Issuing a Key yields a canonical complete Key, a Prefix, and a 32-byte HMAC digest; only the
  presented wrapper sees the complete Key, and its debug output is redacted.
- The correct Key verifies; a tampered Key, same-Prefix different Key, malformed Key, and a
  different external Pepper fail.
- Disabled, revoked, and expiry-bound records fail at their lifecycle boundary without changing
  the public failure form.
- Exact-size external Pepper file loading succeeds; wrong length, non-regular, and symbolic-link
  inputs fail; errors and debug output never reveal the Pepper or complete Key.
