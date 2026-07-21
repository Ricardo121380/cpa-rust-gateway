# P4-01 Endpoint-Credential Model Catalog singleflight report

| Field | Value |
|---|---|
| Plan | `v1.2` |
| Task | `P4-01` |
| Matrix / behavior | `E20`, `G28`, `L09`, `L10`, `L33`; Plan 10 |
| Date | `2026-07-21` |
| Branch | `codex/p4-01-catalog-singleflight` |
| Rust | `1.97.1` |
| Result | `LOCAL_PASS_PENDING_CI`: implementation, review, local tests, Fast, and Full passed; `DONE` remains prohibited until the GitHub code Gate passes. |

## Delivered scope

- Added `gateway-catalog::DiscoveredModel`, which admits only non-empty upstream Model identities,
  and `ModelCatalogTarget`, whose exact identity is the non-secret pair
  `(EndpointId, CredentialId)`.
- Added a Provider-owned `ModelCatalogSource` capability and `ModelCatalogScheduler`. The scheduler
  shares exactly one in-flight discovery for an equal target, but never combines separate
  Credentials on the same Endpoint.
- The detached Tokio discovery task survives initiating-caller cancellation, sends a sorted and
  deduplicated result to current subscribers, removes its in-flight entry before notification, and
  retains neither successful nor failed results after completion.
- Added [ADR-0022](../adr/ADR-0022-endpoint-credential-catalog-singleflight.md) and
  [BC-CATALOG-001](../contracts/BC-CATALOG-001-endpoint-credential-catalog-singleflight.md), plus
  deterministic no-network concurrency, Credential-isolation, cancellation, and failure-retry
  tests.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-catalog` | PASS; 7 tests, including equal-key sharing, Credential separation, initiator cancellation, and failure non-retention. |
| `cargo clippy --locked -p gateway-catalog --all-targets --all-features -- -D warnings` | PASS. |
| `cargo fmt --all -- --check` | PASS inside Fast and Full. |
| `./scripts/check.sh fast` | PASS in about 8 seconds after a reviewed crate-boundary policy correction; all Workspace checks, links, Secret scan, and whitespace passed. |
| `./scripts/check.sh full` | PASS in about 22 seconds; Fast checks, quality-tool versions, `cargo deny check`, and `cargo audit` passed. |

The Full audit emitted a non-blocking crates.io yanked-registry lookup timeout, then exited `0`
after completing its advisory scan. This is the same safe command behavior recorded by P4-00; the
GitHub Full gate remains the acceptance evidence for this code change.

## Review

Review corrected one result-lifecycle race before local acceptance: publishing a failure before
removing the in-flight key would allow an arrival in the small cleanup window to subscribe to a
completed result. The scheduler now removes the key first, then wakes existing subscribers. This
preserves same-flight sharing while ensuring neither success nor failure acts as an accidental
P4-01 cache. The first Fast run also caught that the explicit crate-boundary policy had not yet
listed Tokio for `gateway-catalog`; the policy was updated, then Fast and Full were rerun cleanly.

Review also confirmed that the sharing key includes both stable identifiers; the test source returns
Credential-specific synthetic Models to demonstrate that same-Endpoint calls remain independent.
No raw URLs, Credentials, HTTP clients, provider request bodies, response bodies, real endpoints,
or network calls occur in the scheduler tests.

## Scope and deferred work

P4-01 creates no Catalog snapshot, clock/freshness calculation, last-success retention, diff,
Preview/Apply management operation, public-model exposure, health/circuit signal, quota state,
Route Explain output, persistence, or observability record. P4-02 owns snapshot and failure
fallback semantics; P4-03 owns diff/removal policy; P4-04 through P4-08 own dynamic state and
observability. No real Provider request was authorized or sent.

## GitHub CI

The implementation commit will require the P4-00 `code` path: Fast followed by Full. Its Full job
is the first meaningful warm-cache measurement for `CR-EXEC-001`; the accepted run ID, job timing,
and comparison against the 90-second quality-tool installation target are added only after remote
evidence exists. A separate docs-only closeout must then pass before this Task is marked `DONE`.
