# P2-07 immutable RouteSnapshot publication report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P2-07` |
| Date | `2026-07-19` |
| Branch | `codex/p2-07-route-snapshot` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Added `gateway-router::RouteSnapshot`: an immutable, router-safe view of active Public Models,
  Alias mappings, Routes, hard-eligible Candidates, and Access Group grants. Its construction
  rejects duplicate identities, broken references, Alias namespace conflicts, empty Routes, and
  grants to absent Routes.
- Added a lock-free read-side `RouteSnapshotRegistry` backed by `ArcSwap`. Each request loads one
  owned `Arc<RouteSnapshot>` and retains it across the full request or stream lifetime; readers do
  not take the publication mutex or query `SQLite`.
- Added a reservation/commit publication protocol. The control path constructs the replacement,
  reserves the registry, atomically activates its persisted Config Version, then performs the
  infallible `ArcSwap` commit. The registry retains exactly one predecessor for one-step rollback.
- Added `gateway-store` atomic Config Version activation: `draft` or `archived` becomes `active`
  while the former active Version becomes `archived` in the same `SQLite` transaction.
- Added `gateway-control::SnapshotPublicationService`, which converts P2-06's secret-free
  compiler output into router-safe values, coordinates durable publication and rollback, and
  returns typed results/errors without exposing Credential material or Client Key digests.
- Added [ADR-0007](../adr/ADR-0007-route-snapshot-publication.md) and
  [BC-ROUTER-002](../contracts/BC-ROUTER-002-route-snapshot-publication.md).

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test -p gateway-router -p gateway-control -p gateway-store` | PASS; Router integrity, 100-reader version pinning, publish/rollback toggling, safe no-predecessor rollback, durable activation, publication, rollback, and failed-compilation preservation |
| `cargo clippy -p gateway-router -p gateway-control -p gateway-store --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `git diff --check` | PASS |
| `./scripts/check.sh fast` | PASS; workspace format, Clippy, tests, source policy, secret scan, boundary, documentation, and whitespace gates |
| `./scripts/check.sh full` | PASS; Fast gate plus pinned tool verification, `cargo deny check`, and `cargo audit` |

## Review

Review passed. The review checked that the request path calls `load()` once and only retains its
owned immutable `Arc`, so a later publish cannot mutate an in-flight request or stream. It checked
that the publication mutex is held only on the management path, across the matching database
transition, and that every failure before `PreparedSnapshotPublication::commit` drops the
reservation with the existing Snapshot untouched.

The review also checked the durable ordering: compiler output and Snapshot construction happen
before the transaction; `SQLite` commits the target-active/prior-archived transition before the
in-memory swap; and rollback performs the inverse using only the retained predecessor. Candidate
construction was tightened to a named input object rather than retaining a broad argument-list
lint exemption. Debug coverage confirms no synthetic Credential, ciphertext, or digest reaches the
Snapshot representation.

## Scope and deferred work

P2-07 does not authenticate Client Keys, select Candidates or Credentials, create runtime
schedules, query a live Catalog, expose `/v1/models`, run a Provider, bootstrap a registry from an
existing database, or expose management HTTP/CLI. Those remain P2-08, P2-10, P3, and P4. All test
data is synthetic and contains no deployed Credential, Master Key, Pepper, Client Key, or Client
Key digest.

## GitHub CI

GitHub Actions run [29681389853](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29681389853)
passed for implementation commit `bb6a988`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-19T09:21:53Z` |
| Full supply-chain gate | PASS; completed `2026-07-19T09:32:18Z` |

This completes P2-07 implementation acceptance. The verification-record commit below is pushed
separately and must also pass the same GitHub workflow before P2-08 starts.
