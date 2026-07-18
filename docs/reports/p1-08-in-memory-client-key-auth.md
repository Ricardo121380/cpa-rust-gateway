# P1-08 In-memory Client Key authentication report

| Field | Value |
|---|---|
| Plan | `v1.0` |
| Task | `P1-08` |
| Date | `2026-07-18` |
| Branch | `codex/p1-08-client-key-auth` |
| Rust | `1.97.1` |
| Result | PASS; independent review and GitHub Fast/Full CI complete |

## Delivered scope

- Added a transport-neutral `ClientKeyAuthenticator` port and stable non-secret
  `AuthenticatedClient(ClientKeyId)` result in `gateway-auth`.
- Added immutable enabled/disabled in-memory Client Key records with duplicate/empty validation
  and secret-redacted `Debug` output.
- Made authentication mandatory in `ResponsesHttpState`; public health remains outside the auth
  path.
- Added exact-one `Authorization: Bearer <key>` admission for `/v1/responses` before body decode,
  request context, router, bounded transport, or Provider execution.
- Normalized missing, malformed, unknown, and disabled credentials to the same safe `401` JSON
  envelope and `WWW-Authenticate: Bearer` response.
- Added [BC-AUTH-001](../contracts/BC-AUTH-001-client-key-auth-port.md) and updated the P1-07 HTTP
  contract to point to the current authenticated admission behavior.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-auth -p gateway-http-actix` | PASS; 4 auth unit tests and 13 HTTP/body tests |
| `cargo clippy --locked -p gateway-auth -p gateway-http-actix --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `ruby scripts/check-crate-boundaries.rb` | PASS |
| `ruby scripts/check-doc-links.rb` | PASS |
| `scripts/secret-scan.sh --all` | PASS |
| `git diff --check` | PASS |
| `./scripts/check.sh fast` | PASS; format, Clippy, full workspace tests, source/secret/boundary/doc checks |
| `./scripts/check.sh full` | PASS; fast gate plus dependency policy and RustSec audit |
| GitHub Actions run `29648767066` | PASS; Fast gate and Full supply-chain gate |

## Independent review

Review passed. `gateway-auth` remains independent of HTTP and persistence; its only normal
dependency is `gateway-core`. The review confirmed that the in-memory ordinary-string comparison
is explicitly P1-only and does not claim P2's HMAC/constant-time properties. It also corrected
the contract wording so auth is accurately stated as preceding raw-body interpretation/decoding,
not Actix's `web::Bytes` extraction itself. Missing, malformed, duplicate, unknown, and disabled
credentials have identical safe HTTP envelopes and cannot invoke the counting Provider fixture.

## Limits and P2 seam

P1-08 intentionally does not add persistence, key issuance, prefix indexing, HMAC/pepper,
constant-time verification, expiry, Access Groups, model permission checks, quotas, rate limiting,
management APIs, P1-09, P2, or deployment. P2 replaces the in-memory implementation behind the
same port with an immutable persisted/snapshot-backed view and resolves `ClientKeyId` to its
Access Group policy.
