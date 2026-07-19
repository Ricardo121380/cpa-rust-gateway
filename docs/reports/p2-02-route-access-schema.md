# P2-02 versioned route and access schema report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P2-02` |
| Date | `2026-07-19` |
| Branch | `codex/p2-02-routing-access-schema` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Added migration `0002` for Public Models, exact Aliases, one Route per Public Model, Route
  Candidates, Access Groups, Access Group-to-Route permissions, and Client Key metadata.
- Preserved migration `0001` as historical schema version `1`; the new current version is `2` and
  a test upgrades a populated version-1 database without rewriting migration history.
- Applied Config Version-scoped composite foreign keys throughout routing and access configuration.
- Added unique structural constraints for public model names, aliases, Routes per Public Model,
  Candidate identity, Access Group Route links, and Client Key prefixes/digests.
- Added checked initial route policies, transform modes, initial `endpoint_bindings` Candidate
  scope, JSON-object configuration fields, and a fixed 32-byte opaque Client Key digest column.
- Added [ADR-0002](../adr/ADR-0002-version-scoped-route-access-schema.md) and
  [BC-ROUTE-001](../contracts/BC-ROUTE-001-versioned-route-access-schema.md).

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-store` | PASS; 6 tests cover migration-1 upgrade, all-schema idempotence, FK violations, uniqueness/CHECK constraints, and populated rollback |
| `cargo clippy --locked -p gateway-store --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `./scripts/check.sh fast` | PASS; format, Clippy, full workspace tests, source/secret/boundary/doc checks |
| `./scripts/check.sh full` | PASS; Fast gate plus dependency policy and RustSec audit |

## Review

Review passed. The key migration-runner finding was that migration `0001` formerly derived its
stored version from `CURRENT_SCHEMA_VERSION`; increasing that public constant would have silently
changed version 1 into version 2. The implementation now has immutable historical version
constants and an explicit migration-1-to-2 test. A second review pass verified that
`client_keys` has no plaintext-key column and that `endpoint_bindings` does not choose a Credential
or add a future Credential Scope service.

## Scope and deferred work

P2-02 does not issue Client Keys, generate a Prefix, calculate or compare HMACs, load a Pepper,
decrypt Secrets, enforce expiry or revocation, compile/validate route semantics, resolve Alias
namespace collisions, validate catalog/capabilities, publish a Config Version, create a
RouteSnapshot, execute a route, or expose a management API. Those are assigned to P2-03 through
P2-10. The test digest bytes are fixed non-secret fixtures, not generated or valid client keys.

## GitHub CI

GitHub Actions run [29673886566](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29673886566)
passed on commit `a60bfe1`.

| Job | Result |
|---|---|
| Fast gate | PASS |
| Full supply-chain gate | PASS |

The Fast gate completed at `2026-07-19T04:50:49Z`; the Full supply-chain gate completed at
`2026-07-19T05:00:01Z`. This completes P2-02 acceptance.
