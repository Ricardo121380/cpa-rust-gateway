# P3-07 RouteSnapshot public model view report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-07` |
| Matrix / behavior | `A02`, `B26`, `L27-L31`; Behavior 3/17/19/20 |
| Date | `2026-07-20` |
| Branch | `codex/p3-07-models-response-rewrite` |
| Rust | `1.97.1` |
| Result | PASS locally and in GitHub Fast/Full for both the implementation and verification-record commits |

## Delivered scope

- Added a single immutable Public Model projection on `RouteSnapshot`. It filters by the
  authenticated Client Key's Access Group, returns Public Model names in stable `BTreeMap` order,
  excludes aliases, and requires at least one Candidate that remains hard-eligible by Catalog
  admission plus positive active binding count.
- Added exact visible-model resolution: exact Public Model wins over exact Alias, and both resolve
  only to the stable Public Model name when the Access Group may use the Route. Unknown, forbidden,
  zero-binding, and expired-Catalog input all fail closed as the existing safe `RouteNotFound`.
- Added `SnapshotAuthenticatedClient`, which retains the exact `Arc<RouteSnapshot>` used to verify
  the Client Key HMAC. Its model list and mapping therefore cannot observe a newer Config Version
  after a concurrent publication.
- Added explicit Snapshot-authenticated HTTP state, authenticated `GET /v1/models`, and a
  gateway-owned OpenAI-compatible list encoder. The list exposes only `id`, `object`, `created: 0`,
  and `owned_by: gateway`; it does not render Route/Candidate/Endpoint/Upstream/Credential/Catalog
  fields or the upstream model name.
- Updated `POST /v1/responses` in Snapshot mode to reject non-visible input before executor start
  and to use the resolved Public Model in both completed JSON and every emitted SSE response object.
  A legacy P1 generic-authenticator state remains valid for its existing Mock tests but fails
  closed for `/v1/models`, preventing it from becoming a second model-list source.
- Added [ADR-0017](../adr/ADR-0017-routesnapshot-public-model-view.md) and
  [BC-ROUTE-002](../contracts/BC-ROUTE-002-routesnapshot-public-model-view.md), and recorded
  P3-07 as the sole in-progress task before implementation.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-router -p protocol-openai-responses -p gateway-http-actix` | PASS; 35 Router, 14 protocol, and 18 HTTP tests, including Access Group visibility, zero-binding/expired Catalog filtering, Snapshot pinning, Models authorization, JSON/SSE public-name rewriting, and executor non-start on rejection |
| `cargo clippy --locked -p gateway-router -p protocol-openai-responses -p gateway-http-actix --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `scripts/secret-scan.sh --staged` and `git diff --cached --check` | PASS; all implementation files staged and scanned before commit `21fde53` |
| `./scripts/check.sh fast` | PASS; complete workspace Fast gate |
| `./scripts/check.sh full` | PASS; complete workspace Full gate, including dependency policy and RustSec audit; existing duplicate-version notices are policy-allowed warnings |

## Review

Review passed. The request path reads the immutable Snapshot only once during Snapshot Client Key
admission and keeps that Arc through public-model resolution; it adds no SQLite/YAML/catalog/network
read, global lock, scheduler cursor mutation, Credential lease, or Runtime Health lookup. The list
predicate is intentionally separate from transient 429/Cooldown/Circuit/concurrency/retry state,
so those conditions cannot flap `/v1/models`.

The review found that checking only a Candidate's binding count would accept a manually constructed
Snapshot carrying `CatalogModelState::Expired`. The final implementation adds
`SnapshotRouteCandidate::is_hard_eligible`, which requires both a positive active binding count and
`manual`/`fresh`/`stale` Catalog admission (or the explicit unlisted exception). Regression tests
cover both zero-binding and expired-Catalog paths. It also verifies that aliases never appear in the
list or in Responses metadata, while non-streaming JSON and all three SSE response objects carry
the stable Public Model name.

## Scope and deferred work

P3-07 does not discover models, maintain Catalog freshness, query SQLite, call an upstream
`/models` endpoint, persist runtime health, change 429/Cooldown/Circuit policy, acquire a
Credential, choose a Candidate, emit P3-08 Request/Attempt/Usage events, build P3-09 mock HTTP
E2E, contact P3-10 real endpoints, or add a cross-protocol Models view. Later protocol views must
provide their own compiler-proven native/lossless compatibility predicate; P3-07 retains the
OpenAI Responses aggregation slice only.

## GitHub CI

GitHub Actions run [29708113838](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29708113838)
passed for implementation commit `21fde53`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-19T23:36:08Z` |
| Full supply-chain gate | PASS; completed `2026-07-19T23:46:28Z` |

This completes P3-07 implementation acceptance. The separate verification-record commit must pass
the same two jobs before the final status record can be created; that final record must also pass
before P3-08 can become the plan's sole `IN_PROGRESS` task.

GitHub Actions run [29708526975](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29708526975)
then passed for verification-record commit `6a1e35c`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-19T23:51:38Z` |
| Full supply-chain gate | PASS; completed `2026-07-20T00:02:27Z` |

P3-07 is accepted. This final status-record commit is intentionally documentation-only and must
itself pass the same workflow before P3-08 can become the plan's sole `IN_PROGRESS` task.
