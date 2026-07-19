# P2-08 Snapshot Client Key authentication report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P2-08` |
| Date | `2026-07-19` |
| Branch | `codex/p2-08-snapshot-auth` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Extended `AuthenticatedClient` with an optional `AccessGroupId`. P1's in-memory authenticator
  remains ID-only, while the P2 Snapshot implementation returns the active Access Group without
  exposing a presented Key.
- Added immutable, Prefix-indexed `SnapshotClientKeyView` records to `RouteSnapshot`. The view
  retains only a redacted `ClientKeyRecord`, lifecycle/expiry metadata, and a copied allowed-Route
  set; it contains no complete Client Key, Pepper, Repository, `SQLite` connection, Provider, or
  HTTP type.
- Added Snapshot construction checks for duplicate Key Prefixes and IDs, unknown Access Groups, and
  copied permission sets that diverge from the active Access Group.
- Extended control-plane publication to convert persisted Client Key rows into validated Snapshot
  views. A disabled Access Group contributes no runtime Key; malformed Key material rejects the
  publication before database activation or `ArcSwap` commit.
- Added `SnapshotClientKeyAuthenticator`, which loads one immutable Snapshot per attempt, performs
  canonical Prefix lookup, and delegates HMAC, constant-time comparison, lifecycle, and expiry
  evaluation to P2-04's `ClientKeyService`. It never queries persistence on the request path.
- Kept malformed, unknown, wrong-secret, wrong-Pepper, disabled, revoked, expired, and
  disabled-Access-Group attempts on the same safe `ClientUnauthorized/Request` path. A clock or
  cryptographic infrastructure fault maps to a safe internal request error.
- Added [ADR-0008](../adr/ADR-0008-snapshot-client-key-authentication.md) and
  [BC-AUTH-003](../contracts/BC-AUTH-003-snapshot-client-key-authentication.md), and clarified
  the P2-07/P1 contracts' narrow, redacted Client Key HMAC extension.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-auth -p gateway-router -p gateway-control -p gateway-http-actix -p gateway-store` | PASS; HMAC primitives, Snapshot integrity, hot disablement, exact expiry, safe failures, persistence conversion, and real Actix auth-port injection |
| `cargo clippy --locked -p gateway-auth -p gateway-router -p gateway-control -p gateway-http-actix -p gateway-store --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `ruby scripts/check-crate-boundaries.rb` | PASS; Router may use only storage-neutral `gateway-auth` primitives; no reverse or persistence edge |
| `ruby scripts/check-doc-links.rb` | PASS |
| `scripts/secret-scan.sh --all` and `git diff --check` | PASS |
| `./scripts/check.sh fast` | PASS; workspace format, Clippy, tests, source policy, secret scan, boundary, documentation, and whitespace gates |
| `./scripts/check.sh full` | PASS; Fast gate plus pinned-tool verification, `cargo deny check`, and `cargo audit` |

## Review

Review passed. It verified that authentication loads and retains exactly one current
`Arc<RouteSnapshot>` before parsing/lookup, that HMAC verification remains in the P2-04
implementation, and that the Router has neither a `gateway-store` dependency nor a Repository
call. The review added explicit duplicate-Prefix/ID, unknown-Access-Group, and copied-permission
mismatch tests, plus canonical wrong-secret, unknown, and wrong-Pepper rejection coverage.

It also added an Actix E2E proving the existing `ResponsesHttpState` accepts the actual Snapshot
authenticator through the unchanged port. Debug output is checked through the redacted
`ClientKeyRecord`; no complete Key, digest bytes, or Pepper appears in diagnostics. Historic P2-07
and P1 contract wording was aligned so it no longer contradicts P2-08's deliberately bounded HMAC
view.

## Scope and deferred work

P2-08 does not issue/display management Keys, rotate a Pepper, enforce quotas or rate limits,
generate `/v1/models`, select a Route/Candidate/Credential, run a Provider, bootstrap from an
existing database, or expose a management HTTP/CLI API. Those remain P2-09, P2-10, P3, and later
phases. All test material is synthetic; no deployed Client Key, Pepper, Credential, or Master Key
was written to the repository.

## GitHub CI

GitHub Actions run [29683227429](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29683227429)
passed for implementation commit `ee9a679`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-19T10:24:33Z` |
| Full supply-chain gate | PASS; completed `2026-07-19T10:34:32Z` |

This completes P2-08 implementation acceptance. The verification-record commit below is pushed
separately and must also pass the same GitHub workflow before P2-09 starts.
