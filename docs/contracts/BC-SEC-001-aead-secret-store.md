# BC-SEC-001 AEAD Secret Store

| Field | Value |
|---|---|
| Contract | `BC-SEC-001` |
| Task | `P2-03` |
| Status | DONE |
| Domain | Upstream Secret encryption, Master Key loading, and rotation |

## Entry and boundary

`gateway-store` provides a storage-neutral `SecretStore` for upstream credential bytes. It
accepts caller-provided associated data (AAD), returns an opaque encrypted envelope and its Key
Version, and requires the same AAD to open that envelope. The Store is not a Repository and does
not read/write `upstream_credentials`, expose plaintext through `Debug`, authenticate Client Keys,
or run in the inference hot path.

## Preconditions

- A `MasterKeyRing` contains an active positive Key Version and a distinct exact 32-byte Master
  Key for every supported Key Version.
- The active Key Version is present in the ring. Callers keep Master Key files outside SQLite and
  supply non-secret AAD that identifies the logical encrypted record.
- A persisted envelope stores its `key_version` in the existing `upstream_credentials.key_version`
  column and its binary envelope in `upstream_credentials.ciphertext`.

## Envelope and event sequence

```text
seal(plaintext, aad)
  -> OS randomness produces a fresh 24-byte nonce
  -> XChaCha20-Poly1305 encrypts plaintext with active Master Key and aad
  -> encrypted envelope = 0x01 || nonce || ciphertext-and-16-byte-tag
  -> return (active key_version, opaque envelope)

open(key_version, envelope, aad)
  -> select matching loaded Master Key
  -> reject unknown version, unknown envelope format, or truncated envelope
  -> authenticate and decrypt with the same aad
  -> return redacted zeroizing plaintext bytes

rotate(old key_version, envelope, aad)
  -> open using the recorded old version
  -> seal using the active version and a fresh nonce
  -> return a new opaque envelope; caller later persists it atomically
```

## Invariants

- New seal operations never reuse a nonce deliberately; every call obtains fresh operating-system
  randomness. The nonce is never accepted from a caller.
- Ciphertext integrity includes AAD. Wrong AAD, changed ciphertext/tag, or wrong Master Key return
  the same authentication failure and never yield partial plaintext.
- Key Version `0`, absent active keys, unknown versions, non-32-byte keys, non-regular or
  symbolic-link key files, malformed file names, duplicate/non-canonical version filenames, and
  unexpected directory entries are rejected before use.
- The Master Key directory uses only direct `<positive-decimal-key-version>.key` files, each
  exactly 32 raw bytes. Key contents, plaintext Secret values, and complete ciphertext bytes are
  redacted from `Debug` and error messages.
- Rotation has no implicit database side effect. Old Key Versions remain available until every
  retained row referring to them has been re-encrypted and verified.
- The Secret Store does not define Client Key HMAC/Pepper handling, Credential persistence
  transactions, refresh/revision rules, snapshots, EgressPolicy, or Provider behavior.

## Error semantics

```text
unknown/malformed key material or key directory
  -> safe configuration/load error

unknown key version, unsupported/truncated envelope, wrong AAD/key, or tampered ciphertext
  -> safe decrypt/authentication error

randomness or AEAD encryption failure
  -> safe encryption error; no envelope returned
```

Error text may name a Key Version or structural condition but must not contain plaintext, Master
Key bytes, nonce bytes, full ciphertext, or an external secret-file's contents.

## Corresponding tests

- A Secret sealed twice with the same input decrypts correctly but produces two different opaque
  envelopes; each returned Key Version is the active one.
- Correct AAD succeeds, while wrong AAD, a different key with the same version, a modified
  envelope, and an unavailable version each fail closed.
- A temporary external key directory accepts only canonical 32-byte regular key files and requires
  the active version to be available.
- A record encrypted with an old Key Version is rotated under a new active version, decrypts with
  the new ring, and cannot be opened by a ring that lacks the new Key Version.
