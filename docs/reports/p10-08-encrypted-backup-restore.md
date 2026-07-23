# P10-08 Encrypted backup and empty-target restore report

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-08` |
| Date | `2026-07-24` |
| Branch | `codex/p10-control-plane` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope | Encrypted control-plane SQLite artifact, protected restore preflight, configured empty-target restore, schema compatibility and operator-key guidance. |

## Delivered boundary

`gateway-store` creates a consistent SQLite Backup API snapshot, bounds it to 16 MiB, and encrypts
it under an independently configured, redacted and zeroized 32-byte Backup Key using
XChaCha20-Poly1305. Its fixed magic/version/schema/nonce header is authenticated as AEAD
associated data and a fresh 24-byte nonce is obtained for every artifact. Snapshot plaintext and
decrypted material use zeroizing storage; no artifact is returned after a failed snapshot,
randomness, encryption, authentication or bound check.

Preflight decrypts only bounded material into a staging file, runs `quick_check`, foreign-key
validation and supported-schema checks, then exposes only source schema, mandatory quick-check
and compatibility. Restore has no caller-selected path: it stages, validates and migrates before
using a hard-link create into the configured absent target. Existing targets cannot be replaced.
If cleanup after target creation fails, the implementation makes a best-effort unlink of the
just-created target and returns a value-free failure rather than reporting a completed restore.

The protected P10-02 routes are `POST /admin/backups/preflight`,
`POST /admin/restores/preflight` and `POST /admin/restores`. They require the existing management
admission boundary, accept only bounded `application/octet-stream` restore material, zeroize it
after handling, serialize operations and return closed error classes. Artifact creation is an
operator/embedding method only: the frozen API has no backup download endpoint. Neither the API
nor SPA accepts a Backup Key, Master Key, arbitrary destination, active-database replacement or
Provider operation.

The SPA invokes only generated-client `previewBackup`, `previewRestore` and `restoreBackup`
operations. A selected `File` is passed directly once and cleared in `finally`; it is never read,
rendered, serialized or persisted. The Backup Key is distinct from the credential Master Key;
the latter remains a separately managed post-restore requirement for credential-envelope use.

## Verification

| Evidence | Result |
|---|---|
| `cargo test --locked -p gateway-store` | PASS — store unit, repository and encrypted-backup regressions. |
| `cargo test --locked -p gateway-store --test encrypted_backup` | PASS — encrypted populated-source recovery into a separate empty target, schema/configuration preservation, wrong-key/tamper rejection, bounded malformed input and existing-target refusal. |
| `cargo test --locked -p gateway-control` | PASS — control-plane regressions, including the configured backup facade dependency. |
| `cargo test --offline -p gateway-http-actix --test p10_08_management_backup` | PASS — 2 protected HTTP tests covering concealment, binary type/bounds, safe preflights, completion and no replacement. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p gateway-store -p gateway-control -p gateway-http-actix --all-targets -- -D warnings`, `git diff --check` | PASS. |
| `npm --prefix web/admin-ui run check` | PASS — 65 generated operations and reproducible static build; static policy rejects browser storage, direct `fetch`, `FileReader` and artifact byte reads. |
| `./scripts/check.sh docs`, `./scripts/secret-scan.sh --all` | PASS — 292 Markdown files, one active task, links, tracked/all-file Secret scans and whitespace. |

## Browser E2E

The loopback fixture at `127.0.0.1:4183` served built assets and deterministic, value-free
backup metadata only. It had no database, artifact persistence, Backup/Master Key, Provider
transport, proxy or external egress. With its page-local management session, Chromium confirmed:

1. Configured-source preflight returns only schema `9` and `secret_key_required: true`.
2. A selected local non-secret binary passed directly to restore preflight returns only schema
   `9`, `quick_check_required: true` and `compatible: true`; the file input is then empty.
3. Re-selecting the same file for restore returns only `state: complete`; the file input is again
   empty.
4. Both `localStorage` and `sessionStorage` have length zero, and a reload clears the session and
   returns the page to `Not connected`.

The browser fixture verifies client handling and safe projections, not the cryptographic artifact
format; the real encrypted artifact/recovery path is covered by the store and protected HTTP
tests above.

## Review and limits

Focused review checked authenticated header/AAD handling, fresh nonces, error redaction, bounded
decrypt/body paths, same-directory staging, no-clobber target creation, cleanup behavior,
schema/integrity order, no route/key/path expansion, generated-client-only UI behavior and no
browser persistence. No release-blocking issue remains.

P10-09 exclusively owns static-resource embedding and inference-hot-path performance evidence;
P11 owns in-place migration, downgrade and rollback drills. P10's single local Full/Phase review,
commit, GitHub Delivery Gate and final `DONE` transition remain pending until P10-09 completes.
