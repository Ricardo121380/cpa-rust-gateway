# P4-01 Endpoint-Credential Model Catalog singleflight report

| Field | Value |
|---|---|
| Plan | `v1.2` |
| Task | `P4-01` |
| Matrix / behavior | `E20`, `G28`, `L09`, `L10`, `L33`; Plan 10 |
| Date | `2026-07-21` |
| Branch | `codex/p4-01-catalog-singleflight` |
| Rust | `1.97.1` |
| Result | Accepted after local verification, GitHub code Gate, same-ref warm-cache measurement, and docs-only closeout; this final evidence-only record must also pass docs-only Gate. |

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

GitHub Actions [run 29800492406](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29800492406)
passed for implementation commit `7689c92`. It correctly classified the change as `code`, skipped
Docs-only, passed Fast, passed Full, and passed the required delivery gate.

| Job / step | Result and duration |
|---|---|
| Fast gate | PASS; job about 164 seconds; `Run fast gate` about 141 seconds. |
| Full supply-chain gate | PASS; job about 677 seconds. |
| Restore pinned quality-tool cache | PASS as a cache miss; the same key had been saved only on the prior P4-00 branch. |
| Install pinned quality tools | PASS; cold installation about 495 seconds. |
| Run full gate | PASS; about 151 seconds. |
| Required delivery gate | PASS; verified the selected code path. |

The code Gate is valid P4-01 acceptance evidence, but it is not the warm-cache result predicted by
P4-00. GitHub recorded the P4-00 cache on
`refs/heads/codex/p4-00-execution-acceleration`, while this run used
`refs/heads/codex/p4-01-catalog-singleflight`; its log explicitly reported `Cache not found`.
The successful P4-01 Full then saved a cache on its own ref.

To measure the cache rather than infer it, an explicit `workflow_dispatch` code-path rerun on the
same immutable P4-01 commit passed as [run 29801218989](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29801218989).
It also selected code, skipped Docs-only, and passed Fast, Full, and the required gate.

| Full measurement | Cross-ref push run | Same-ref warm rerun | Change |
|---|---:|---:|---:|
| Full job | 677 s | 168 s | 509 s faster (about 75%) |
| Quality-tool installation | 495 s | 1 s | 494 s faster |
| Full checks after installation | 151 s | 117 s | normal runner variance; still fully executed |

The warm rerun restored the exact versioned cache, verified `cargo-deny 0.20.2` and
`cargo-audit 0.22.2` without reinstalling, and met the `<=90s` installation target by a wide
margin. P4-00's docs-only run had already shown the independent documentation path completing in
about 41 seconds with Fast and Full skipped.

## Accepted docs-only closeout

GitHub Actions [run 29801650393](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29801650393)
passed for the P4-01 code-gate/cache-evidence closeout commit `2fb5aca`.

| Job / step | Result and duration |
|---|---|
| Classify delivery gate | PASS; selected `docs` in about 5 seconds. |
| Docs-only gate | PASS; job about 18 seconds, including prerequisites; `Run docs-only gate` about 2 seconds. |
| Fast and Full supply-chain gates | Correctly skipped. |
| Required delivery gate | PASS; verified the docs selection and skipped code gates. |

The entire run completed in about 37 seconds. It proves that P4-01's plan/report/contract/index
closeout did not repeat the roughly 11-minute cold code Gate or the roughly 3-minute same-ref warm
code Gate, while retaining document links, plan-state validation, tracked Secret scanning, and
whole-tree whitespace checks.

## Efficiency conclusion and follow-up boundary

`CR-EXEC-001` produces a material improvement on a cache-visible ref and preserves every Fast,
Full, version, supply-chain, Secret, and plan-state check. It also eliminates the code-gate cost
for a true documentation-only commit. It does **not** automatically accelerate a push on a new
task branch: the observed P4-00 and P4-01 cache entries are branch-scoped, so the first code Gate
on P4-01 was cold.

Before treating this as a cross-task acceleration guarantee, a separately approved execution-plan
change must choose one explicit operational rule: keep sequential code Tasks on a cache-visible
delivery branch, or seed/consume the cache from an approved shared ref such as the default branch.
This P4-01 Task records the evidence but does not alter branch policy, merge policy, GitHub cache
scope, branch protection, or the locked `CR-EXEC-001` rule without that approval.
