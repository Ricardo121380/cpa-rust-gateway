# P2-10 Local management lifecycle report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P2-10` |
| Matrix / behavior | `H05`, `J03`, `J18-J20`; Behavior 20 |
| Date | `2026-07-19` |
| Branch | `codex/p2-10-management-api-cli` |
| Rust | `1.97.1` |
| Result | PASS locally; GitHub acceptance pending |

## Delivered scope

- Added migration 4's append-only `management_audit_events` table. A record contains only a
  monotonic ID, action, bounded actor label, Unix-millisecond timestamp, target Config Version,
  and optional replaced Version; it contains no Secret, Client Key, digest, ciphertext, URL,
  request body, or error detail. SQLite triggers reject ordinary update/delete changes to preserve
  audit and rollback-predecessor evidence.
- Added transaction-bound Repository operations: creating a complete draft and recording
  `config_created` commit together; activation and a `config_published` or `config_rolled_back`
  event commit together before an `ArcSwap` change can occur. A failed audit write leaves both
  durable state and the live Snapshot unchanged.
- Added restart bootstrap for `SnapshotPublicationService`. It recompiles the active Version and
  reconstructs the exact archived one-step predecessor from the latest successful audit event.
  A database with no active Version has only an empty synthetic Snapshot, which exposes no models,
  Routes, Access Groups, or Client Keys and cannot be treated as a rollback target.
- Added `gateway-control::ManagementService`: a typed, transport-neutral management lifecycle API
  for draft creation, validation, publication, rollback, and safe audit listing. The service uses
  injected Catalog/capability evidence and keeps the Repository out of the request path.
- Added local `gateway admin create|validate|publish|rollback|audit` commands. They accept only
  explicit command fields, reject duplicate/unrelated flags, and do not add a whole-file YAML/JSON
  overwrite path or a management HTTP listener. The built-in local CLI compiler has empty Catalog
  and capability evidence, so it safely supports empty draft scaffolding rather than fabricating
  eligibility for a populated Route graph.
- Added [ADR-0010](../adr/ADR-0010-local-management-lifecycle.md) and
  [BC-CONTROL-002](../contracts/BC-CONTROL-002-local-management-lifecycle.md), plus traceability,
  ADR, contract, report, and plan status updates.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway -p gateway-control -p gateway-router -p gateway-store` | PASS; CLI parser, management lifecycle/restart/rollback E2E, audit persistence, Snapshot predecessor, migration, secret-redaction, and prior P2 regressions |
| `cargo clippy --locked -p gateway -p gateway-control -p gateway-router -p gateway-store --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| Local CLI smoke: create v1 → validate v1 → publish v1 → create/publish v2 → fresh-process rollback → audit | PASS; emitted five ordered safe audit events, with v2 publication replacing v1 and rollback restoring v1 |
| Duplicate local `create` after v1 already exists | PASS; SQLite rejected the graph and did not append an additional audit event |
| `./scripts/check.sh fast` | PASS; full workspace format, Clippy, tests, source policy, secret scanner, crate boundary, documentation, and whitespace gates |
| `./scripts/check.sh full` | PASS; Fast gate plus pinned-tool verification, dependency policy, and RustSec audit |

## Review

Review passed. The review checked the state-transition order end to end: Snapshot construction and
registry reservation happen before the durable activation; activation and audit append share one
SQLite transaction; the infallible `ArcSwap` commit happens only after that transaction succeeds.
It also checked that bootstrap does not guess an archived rollback target: it accepts only the
specific predecessor named by the latest successful audit transition and fails closed if its stored
state is inconsistent.

The review strengthened the implementation with an explicit publisher-level audit-action check, so
a caller cannot record a rollback action during publication (or the reverse) before any activation
is attempted. It confirmed that audit `Debug`/CLI output contains only safe metadata, that the
synthetic no-active Snapshot cannot become a database rollback target, and that no Repository,
Secret, Client Key, upstream socket, HTTP client, proxy, TLS, or P3 Provider behavior crosses into
the request path.

## Scope and deferred work

P2-10 deliberately does not implement a remote management HTTP/OpenAPI surface, Management Key
authentication, authorization, localhost/private-network enforcement, CSRF/CORS, full entity CRUD,
configuration export, Web UI, or audit-query UI; P10 owns those capabilities. It does not discover
Catalogs, persist capability evidence, query an upstream, open a socket, configure timeouts/proxy/
TLS, or perform aggregation/retry behavior; P4 and P3 own those tasks. No P3 work has begun.

All fixtures and the CLI smoke database are synthetic. No deployed Credential, Master Key, Pepper,
Client Key, URL query secret, Authorization header, request body, or production traffic was used.

## GitHub CI

GitHub Actions run [29687760913](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29687760913)
passed for implementation commit `fccdc74`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-19T12:53:18Z` |
| Full supply-chain gate | PASS; completed `2026-07-19T13:03:57Z` |

This completes P2-10 implementation acceptance. The P2/G2 verification-record commit is pushed
separately and must also pass the same GitHub workflow before P2 is declared complete.
