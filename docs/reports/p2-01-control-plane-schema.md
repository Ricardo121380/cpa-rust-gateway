# P2-01 versioned control-plane schema report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P2-01` |
| Date | `2026-07-19` |
| Branch | `codex/p2-01-control-plane-schema` |
| Rust | `1.97.1` |
| Result | Local PASS; GitHub Fast/Full CI pending push |

## Delivered scope

- Added `gateway-store`'s reproducible bundled-`SQLite` dependency and versioned migration runner.
- Added reversible migration `0001` for `config_versions`, `upstreams`,
  `upstream_endpoints`, `upstream_credentials`, and `endpoint_credential_bindings`.
- Made configuration rows version-scoped and enforced Endpoint/Credential/Binding relationships
  with composite foreign keys, including the same-Upstream requirement for a Binding.
- Added schema constraints for Config Version status, enablement booleans, JSON tag arrays,
  non-empty bounded configuration fields, opaque ciphertext, credential revision/key version, and
  Binding scheduling values.
- Added [ADR-0001](../adr/ADR-0001-version-scoped-control-plane-schema.md) and
  [BC-STORE-001](../contracts/BC-STORE-001-versioned-control-plane-schema.md).

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-store` | PASS; 3 tests cover idempotent up, valid/FK-invalid trees, and populated parent-version rollback |
| `cargo clippy --locked -p gateway-store --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo deny check licenses` | PASS; the new `rusqlite` dependency graph satisfies the license allowlist |
| `cargo audit` | PASS |
| `./scripts/check.sh fast` | PASS; format, Clippy, full workspace tests, source/secret/boundary/doc checks |
| `./scripts/check.sh full` | PASS; Fast gate plus dependency policy and RustSec audit |

## Review

Review passed after a focused schema and rollback pass. The review found that a direct
`DROP TABLE config_versions` can be blocked by an in-table parent/child `RESTRICT` relationship.
The down migration now clears `parent_id` after dependent tables are removed and before dropping
the Version table; the round-trip test inserts a populated parent/child Version chain to prove the
fix. No next-Task entity or behavior was added.

## Scope and deferred work

This task does not add a Repository, service transaction, Public Model, Alias, Route, Candidate,
Access Group, Client Key, AEAD implementation, plaintext Secret handling, EgressPolicy entity or
URL/SSRF behavior, RouteSnapshot, publication/rollback management operation, or HTTP API. These
remain in P2-02 through P2-10 as assigned by the locked plan.

## GitHub CI

The code is ready to commit and push. This report will be updated to `DONE` only after the branch
receives successful GitHub Fast and Full CI results.
