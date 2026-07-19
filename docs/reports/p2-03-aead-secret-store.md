# P2-03 AEAD Secret Store report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P2-03` |
| Date | `2026-07-19` |
| Branch | `codex/p2-03-aead-secret-store` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Added `gateway-store::secret_store`, a storage-neutral `XChaCha20-Poly1305` AEAD boundary for
  upstream Secret bytes, not a Repository or request-path component.
- Defined a versioned opaque envelope: one format byte, one fresh 24-byte OS-random nonce, and
  AEAD ciphertext/tag; the existing `upstream_credentials.key_version` remains its external
  version field, so no SQLite migration is needed.
- Required non-empty caller supplied AAD at both seal and open time. Wrong AAD, wrong same-version
  key material, altered bytes, unknown Key Version, unknown envelope format, and truncation fail
  closed without plaintext.
- Added redacted/zeroizing `MasterKey` and `PlaintextSecret` types, plus redacted encrypted
  envelope debug output. Raw Master Key bytes and plaintext are never returned by `Debug` or error
  messages.
- Added a strict external Master Key directory loader for direct canonical
  `<positive-version>.key` files containing exactly 32 raw bytes. It rejects symbolic links,
  non-regular entries, malformed names, invalid lengths, duplicate in-memory versions, and a
  missing active version. On the Mac/Linux deployment targets it also opens key files with
  `O_NOFOLLOW` so a key-file symlink is not followed during the check-to-open window.
- Added Key Ring rotation: an old envelope authenticates with its recorded loaded version and is
  resealed under the active version with a fresh nonce. Persistence remains a later atomic
  Repository operation.
- Added [ADR-0003](../adr/ADR-0003-xchacha20poly1305-secret-store.md) and
  [BC-SEC-001](../contracts/BC-SEC-001-aead-secret-store.md).

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-store` | PASS; 14 tests cover fresh envelopes, AAD/key/tamper failures, malformed input, strict key-directory loading, symlink rejection, duplicate versions, and old-to-new Key Version rotation |
| `cargo clippy --locked -p gateway-store --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo deny check` | PASS; advisories, bans, licenses, and sources pass for AEAD, OS-randomness, zeroization, and no-follow support dependencies |
| `cargo audit` | PASS; 0 RustSec advisories for the locked dependency graph |
| `scripts/check-source-policy.rb` | PASS; no unsafe/unwrap/expect/panic escape hatch |
| `scripts/check-crate-boundaries.rb` | PASS; crypto dependencies are explicit `gateway-store` leaf dependencies |
| `ruby scripts/check-doc-links.rb` | PASS |
| `scripts/secret-scan.sh --all` | PASS; no tracked Secret or credential path |
| `./scripts/check.sh fast` | PASS; full Workspace format, Clippy, tests, source, secret, boundary, and document checks |
| `./scripts/check.sh full` | PASS; Fast gate plus pinned tool version, `cargo deny`, and RustSec audit checks |

## Review

Review passed. The focused review checked the envelope boundary before decrypt, mandatory AAD,
same-version wrong-key behavior, row-swap resistance through AAD, nonce ownership, strict external
key file admission, zeroizing/redacted plaintext and Master Key wrappers, and the absence of
SQLite/HTTP/Provider operations from the new API. During implementation, a `const` conversion was
incompatible with the locked Rust toolchain and a symlink test initially observed an unrelated
directory-entry error; both were corrected before the final focused test and Clippy run. The final
loader adds `O_NOFOLLOW` on the supported Unix targets so a key-file symbolic link is not followed
between admission and read.

## Scope and deferred work

P2-03 does not create or persist Credentials, construct record-specific AAD, perform an atomic
rotation transaction, issue Client Keys, load a Client Key Pepper, calculate HMACs, authenticate a
request, compile/publish a Snapshot, query SQLite on the request path, or expose an API/CLI.
Those remain P2-04 through P2-10. The synthetic test byte arrays are non-production fixtures and
not deployed key material.

## GitHub CI

GitHub Actions run [29675091607](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29675091607)
passed on commit `cd9d79b`.

| Job | Result |
|---|---|
| Fast gate | PASS |
| Full supply-chain gate | PASS |

The Fast gate completed at `2026-07-19T05:37:46Z`; the Full supply-chain gate completed at
`2026-07-19T05:48:53Z`. This completes P2-03 acceptance.
