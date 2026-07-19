# P2-05 versioned control-plane Repository and Service report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P2-05` |
| Date | `2026-07-19` |
| Branch | `codex/p2-05-control-plane-service` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Added `gateway-store::control_plane`, a typed `SqliteControlPlaneRepository` and
  `ControlPlaneTransaction` covering every P2-01/P2-02 table. Graph writes follow foreign-key
  order and commit only when every row succeeds; dropping an uncommitted transaction rolls back
  earlier writes.
- Added typed persisted enums for Version, Credential, Client Key, Public Model, Access Group,
  route policy, Candidate scope/transform, and Endpoint transport. Loaded encrypted envelopes,
  Key Versions, and 32-byte Client Key digests are validated before a graph is returned; all
  sensitive `Debug` values remain redacted.
- Constrained P2-05 mutations to `draft` Config Versions. It cannot create an `active` database
  Version or alter an existing active Version before P2-07 creates and publishes its corresponding
  immutable `RouteSnapshot`.
- Added management-only `gateway-control::ControlPlaneService`. It builds stable length-delimited
  AAD from `(config_version_id, credential_id, upstream_id)`, seals the plaintext Credential with
  P2-03 AEAD before Repository entry, issues a P2-04 Client Key, and persists only opaque AEAD/
  HMAC artifacts in one transaction.
- Added a public Repository integration test for a complete two-version graph, plus service tests
  for AAD authenticity, duplicate Client Key rollback, redaction, malformed persisted crypto, and
  draft-only admission. Provider/Router/HTTP boundaries remain unchanged.
- Added [ADR-0005](../adr/ADR-0005-versioned-control-plane-repository-service.md) and
  [BC-CONTROL-001](../contracts/BC-CONTROL-001-versioned-control-plane-repository-service.md).

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-store -p gateway-control` | PASS; 23 focused unit/integration tests including complete graph reload, crypto-record fail-closed handling, stable AAD, duplicate Client Key rollback, and draft-only mutation admission |
| `cargo clippy --locked -p gateway-store -p gateway-control --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `ruby scripts/check-crate-boundaries.rb` | PASS; Provider/private Provider, Router, and HTTP crates do not gain Store/Control dependencies |
| `git diff --check` | PASS |
| `./scripts/check.sh fast` | PASS; workspace format, Clippy, tests, source policy, secret scan, boundary, documentation, and whitespace gates |
| `./scripts/check.sh full` | PASS; Fast gate plus pinned tool verification, `cargo deny check`, and `cargo audit` |

## Review

Review passed. The focused review checked that the Repository never accepts plaintext Credentials
or complete Client Keys, that all encryption uses a stable domain-separated length-delimited AAD,
and that Client Key insertion occurs after the new Credential in the same SQLite transaction.
The duplicate-key test proves the later unique-constraint failure leaves no newly encrypted
Credential row behind. It also checked redacted `Debug`/error paths, malformed envelope/digest
fail-closed loading, complete graph scoping, and the mechanical crate boundary rule.

The review strengthened two boundaries before final validation: structural enum values are decoded
into typed values instead of returned as unvalidated strings, and all P2-05 graph/Credential/Key
mutations require `draft` status. This keeps P2-06 semantic compilation and P2-07 Snapshot
publication as the only future route to an active runtime configuration.

## Scope and deferred work

P2-05 does not validate Alias/Route/Catalog semantics, compile a Route, publish or roll back a
Snapshot, replace the P1 runtime authenticator, run a request against SQLite, define EgressPolicy,
or expose a management API/CLI. Those remain P2-06 through P2-10. All test data is synthetic and
contains no deployed Credential, Master Key, Pepper, or complete Client Key.

## GitHub CI

GitHub Actions run [29677865862](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29677865862)
passed for implementation commit `7ff1db1`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-19T07:19:51Z` |
| Full supply-chain gate | PASS; completed `2026-07-19T07:29:40Z` |

This completes P2-05 implementation acceptance. The verification-record commit below is pushed
separately and must also pass the same GitHub workflow before P2-06 starts.
