# P1-06 Deterministic Mock Provider report

| Field | Value |
|---|---|
| Plan | `v1.0` |
| Task | `P1-06` |
| Date | `2026-07-18` |
| Branch | `codex/p1-06-deterministic-mock-provider` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Added core-only object-safe `ProviderAdapter`, `InferenceAdapter`, and pull-only
  `CanonicalEventSource` traits using a boxed standard-library Future.
- Added `DeterministicMockProvider`, immutable validated `MockFixture`, and per-pull
  `MockEmission` delay. Each execution creates a fresh source without inspecting request data,
  using global state, spawning a producer, or buffering unbounded events.
- Validates a complete script through `CanonicalEventState` before execution. Text, Tool,
  stream-error, and delay fixtures are desensitized JSON test evidence.
- Keeps pre-start errors distinct from post-start failure: the former is returned by `execute`; the
  latter is a terminal canonical `StreamError` in the source.
- Added paused-time tests for delay, cancelled pending pulls, and cancelled delayed execution. The
  normal `gateway-provider` feature closure excludes Tokio `test-util`; it is dev-only.

## Verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-provider` | PASS; 7 unit tests plus doc tests |
| `cargo clippy --locked -p gateway-provider --all-targets --all-features -- -D warnings` | PASS |
| `ruby scripts/check-crate-boundaries.rb` | PASS; no HTTP/SSE/stream dependency added |
| `cargo tree --locked -e features -p gateway-provider --edges normal,build` | PASS; no `test-util` in normal closure |
| `./scripts/check.sh fast` | PASS |
| `./scripts/check.sh full` | PASS; dependency policy and RustSec audit pass |
| `git diff --check` | PASS |

## Review

- Independent design review confirmed the capability stays limited to execution and a pull-only
  source; catalog, routing, credentials, bounded transport, Actix, and protocol wire encoding
  remain out of scope.
- The first code review found no blockers and recommended three improvements. The final review
  passed after clarifying that post-start failures use `StreamError`, moving Tokio `test-util` to a
  dev-only dependency, and adding delayed pre-start cancellation coverage.

## Limits and next task

P1-06 does not expose any HTTP endpoint, authentication, provider routing, bounded transport,
SSE/JSON writer, or FirstSemanticEvent delivery commit. P1-07 remains `PENDING` and may begin only
as the sole `IN_PROGRESS` task on its own branch after this task's GitHub CI passes.
