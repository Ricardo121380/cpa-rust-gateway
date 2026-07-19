# P2-04 Client Key HMAC credential report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P2-04` |
| Date | `2026-07-19` |
| Branch | `codex/p2-04-client-key-hmac` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Added `gateway-auth::client_key`, a storage-neutral Client Key issuance and verification
  boundary. It does not query SQLite, expose an HTTP endpoint, or replace P1's live in-memory
  authenticator.
- Issued canonical high-entropy Keys as
  `rgw_<16 lowercase-hex Prefix>_<64 lowercase-hex Secret>`, using OS randomness for both the
  8-byte public Prefix and 32-byte Secret. The persistable Prefix is the safe `rgw_<Prefix>` part.
- Added an external exact-32-byte `ClientKeyPepper` file loader, distinct from P2-03 Master Keys.
  It rejects symbolic links, non-regular paths, short/long material, and reads on Mac/Linux with
  `O_NOFOLLOW` so a symbolic-link Pepper is not followed during the check-to-open window.
- Added `HMAC-SHA256(Pepper, exact complete Key)` digest calculation, a fixed 32-byte opaque
  digest type, and explicit `subtle` constant-time equality on the only verification path.
- Added persistable Client Key record fields for ID, Access Group, Prefix, digest, active/disabled/
  revoked status, and optional expiry. A verifier first validates the canonical Prefix, then
  calculates and compares the HMAC before lifecycle state decides success.
- Added non-cloneable redacted presentation wrappers plus zeroization for Pepper, presentation,
  digest, and temporary generated Secret material. P2-10 remains responsible for an API/CLI that
  displays the complete Key exactly once.
- Added [ADR-0004](../adr/ADR-0004-client-key-hmac-credential.md) and
  [BC-AUTH-002](../contracts/BC-AUTH-002-client-key-hmac-credential.md), and clarified P1's
  contract: P2-04 creates primitives while P2-08 replaces live authentication.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-auth` | PASS; 10 tests cover P1 auth plus P2-04 issuance, format, HMAC verification, tampering, wrong Pepper, expiry, disable/revoke, strict Pepper files, and redaction |
| `cargo clippy --locked -p gateway-auth --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo deny check` | PASS; advisories, bans, licenses, and sources pass (only the existing `socket2` duplicate-version warning remains) |
| `cargo audit` | PASS; 0 RustSec advisories for the locked dependency graph |
| `scripts/check-source-policy.rb` | PASS; no unsafe/unwrap/expect/panic escape hatch |
| `scripts/check-crate-boundaries.rb` | PASS; HMAC, SHA-2, constant-time, randomness, and zeroization dependencies are explicit `gateway-auth` leaf dependencies |
| `ruby scripts/check-doc-links.rb` | PASS |
| `scripts/secret-scan.sh --all` | PASS; no tracked complete Key, Pepper, or credential path |
| `./scripts/check.sh fast` | PASS; full Workspace format, Clippy, tests, source, secret, boundary, and document checks |
| `./scripts/check.sh full` | PASS; Fast gate plus pinned tool version, `cargo deny`, and RustSec audit checks |

## Review

Review passed. The focused review verified that the format parser rejects non-canonical complete
Keys, only the verifier uses `subtle::ConstantTimeEq`, and any ordinary `PartialEq` derives remain
value semantics outside the authentication path. It also verified digest calculation occurs before
active/disabled/revoked/expiry admission, so those lifecycle states cannot skip an otherwise
candidate comparison. The review checked all custom `Debug` and error output for complete-Key and
Pepper leakage, confirmed P1's runtime `ClientKeyAuthenticator` remains unchanged until P2-08,
and retained the P2-05 Repository boundary for persistence and Prefix-conflict retry.

The initial implementation review corrected two local issues before final verification: a value
type needed equality traits for non-auth record tests, and a redaction assertion initially treated
the intentionally visible Prefix as a complete Key. Neither change altered the HMAC verification
path; the final test/Clippy runs pass.

## Scope and deferred work

P2-04 does not persist a Client Key, issue an API/CLI response, retry a unique Prefix collision,
load a route Snapshot, replace the P1 HTTP authenticator, enforce Access Group Route permissions,
generate management credentials, rotate a Pepper, perform rate limiting, or query SQLite from a
request. P2-05, P2-08, and P2-10 own those actions. Test bytes are synthetic fixtures and not
deployed Pepper or Key material.

## GitHub CI

GitHub Actions run [29676361709](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29676361709)
passed on commit `0bff7f2`.

| Job | Result |
|---|---|
| Fast gate | PASS |
| Full supply-chain gate | PASS |

The Fast gate completed at `2026-07-19T06:25:18Z`; the Full supply-chain gate completed at
`2026-07-19T06:35:30Z`. This completes P2-04 acceptance.
