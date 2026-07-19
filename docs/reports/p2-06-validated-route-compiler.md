# P2-06 validated Route Compiler report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P2-06` |
| Date | `2026-07-19` |
| Branch | `codex/p2-06-route-compiler` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Added storage-neutral `gateway-catalog` `CatalogView` and `EndpointCapabilityView` value types.
  Their duplicate-record checks and `CapabilitySet` invariant make `parallel_tools` impossible
  without `tools`; P4 remains responsible for Catalog discovery and persistence.
- Added management-time `gateway-control::RouteCompiler`, which compiles one P2-05
  `ControlPlaneConfiguration` into deterministic Public Model, Alias, Route, Candidate, and
  Access Group views. The result contains neither plaintext Credential data, encrypted envelopes,
  Client Key digests, nor complete Client Keys.
- Enforced the P2-06 semantic conflict matrix: namespace and reference validity, duplicate
  Endpoint API format, active Endpoint/Upstream and Credential-binding admission, Catalog state,
  Endpoint capabilities, Candidate non-escalation, and Access Group route publication.
- Defined Catalog hard eligibility as `manual`, `fresh`, or `stale`; missing and expired entries
  require the explicit per-Candidate `allow_unlisted_model` exception.
- Kept disabled Public Models structurally validated but outside hard eligibility. Their candidates
  cannot leak into the compiled view or make a disabled historical model block a publishable
  version solely because its runtime evidence is stale.
- Added [ADR-0006](../adr/ADR-0006-validated-route-compiler.md) and
  [BC-ROUTER-001](../contracts/BC-ROUTER-001-validated-route-compiler.md).

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test -p gateway-control -p gateway-catalog` | PASS; Catalog view invariants plus 10 `gateway-control` tests covering a deterministic multi-Candidate graph, Catalog state, capability behavior, secret-free output, and error snapshots |
| `cargo clippy -p gateway-control -p gateway-catalog --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `ruby scripts/check-source-policy.rb` | PASS; no forbidden panic/unwrap/expect/TODO patterns |
| `ruby scripts/check-crate-boundaries.rb` | PASS; explicit `serde_json` dependency is documented and no request-path dependency is added |
| `git diff --check` | PASS |
| `./scripts/check.sh fast` | PASS; workspace format, Clippy, tests, source policy, secret scan, boundary, documentation, and whitespace gates |
| `./scripts/check.sh full` | PASS; Fast gate plus pinned tool verification, `cargo deny check`, and `cargo audit` |

## Review

Review passed. The review checked that compilation remains management-time only; its output has no
Credential, encrypted-secret, Client-Key-digest, Repository, Provider, or Snapshot handle. It
also checked that all acceptance decisions use stable IDs and ordered maps, Candidate overrides
can only confirm or narrow Endpoint capabilities, and missing/expired Catalog data fails closed
unless the explicitly named configuration exception is present.

The review strengthened the implementation before the final gates: disabled Public Models now
exclude otherwise enabled Candidates from runtime hard-eligibility while preserving structural
validation, and the conflict matrix snapshots the externally visible lower-snake-case error codes
rather than only Rust enum variants. Tests also cover a two-Candidate deterministic order and all
four Catalog freshness states.

## Scope and deferred work

P2-06 does not publish or roll back a Version, construct an `ArcSwap` Snapshot, authenticate a
Client Key, choose a runtime Credential, create weighted schedules, issue an HTTP/CLI management
API, query an actual Provider, or persist/discover Catalog data. Those remain P2-07, P2-08,
P2-10, P3, and P4. All test data is synthetic and contains no deployed Credential, Master Key,
Pepper, Client Key, or Client Key digest.

## GitHub CI

GitHub Actions run [29679742048](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29679742048)
passed for implementation commit `58dbdb8`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-19T08:25:38Z` |
| Full supply-chain gate | PASS; completed `2026-07-19T08:35:52Z` |

This completes P2-06 implementation acceptance. The verification-record commit below is pushed
separately and must also pass the same GitHub workflow before P2-07 starts.
