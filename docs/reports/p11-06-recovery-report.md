# P11-06 Recovery Report

| Field | Value |
|---|---|
| Plan version | `v1.45` |
| Task | `P11-06` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` — implementation, focused verification, Full local gate, and independent review are complete; G11 remains pending P11-07 and P11-08. |
| Branch | `codex/p11-release-hardening` |
| Test boundary | Ephemeral `127.0.0.1` HTTP, synthetic Client Key, deterministic Canonical events, and temporary local SQLite files only. |

## Result matrix

| Recovery case | Fault / action | Observed result | Result |
|---|---|---|---|
| Graceful HTTP stop and stream drain | A real loopback Actix listener emits the first Canonical SSE event, then its source is gated. `ServerHandle::stop(true)` is sent while that response is active. | The server task has not joined while the stream is gated. After the gate is released, the client receives `response.completed`, the connection closes, and the server joins within the test bound. | PASS |
| Crash and writer restart | A writer has one Required event pending because its parent database directory is absent; the task is then aborted. A fresh writer receives the source-held event again. | Before abort, counters show zero committed and one pending event. The restarted writer commits one row; reopen returns one row and `PRAGMA quick_check` is `ok`. | PASS |
| SQLite full and recovery | A populated temporary database is capped at its current `max_page_count`; a new 1,024-event Required batch is attempted. The test-only writer seam reapplies the per-connection cap before each write, then raises it after the failure is observed. | Direct SQLite append is specifically `SQLITE_FULL`. The writer reports no commit and retains exactly 1,024 pending events. Raising the cap permits the same batch to commit once; the reopened database has 2,048 total rows and passes `quick_check`. | PASS |
| Event-queue degradation | Existing HTTP regression uses a Required queue of capacity one for a streaming response that emits a Request and final Usage record. | The queue reports one explicit `RequiredQueueFull`; the response still contains `response.completed`. No request path waits for the queue or writer. | PASS |

## Recovery semantics and limits

- `stop(true)` drains an already-started **local** HTTP stream in the harness. This repository does
  not yet own a production listener, process supervisor, systemd unit, or deployment lifecycle,
  so this is not a claim about an installed server. P12 owns that proof.
- A pending event is deliberately not made crash-durable: after an abort, a source that needs
  durability must replay it to the fresh writer. The writer neither increments a success counter
  nor fabricates a row before its SQLite transaction commits.
- The full-disk drill constrains SQLite's own page allocator rather than exhausting the Mac's
  volume. This is deterministic and verifies the actual `SQLITE_FULL` error category plus the
  production writer's bounded retry path. It does not certify filesystem quota, volume health, or
  a deployed database directory.
- The writer's `max_page_count` control is compiled only for this unit test. It is not part of the
  production writer API or runtime configuration.

## Verification record

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-http-actix --test p11_06_recovery_drills -- --nocapture` | PASS — real loopback graceful drain and join. |
| `cargo test --locked -p gateway-store` | PASS — 31 unit tests plus 4 integration tests, including abort/replay/reopen and deterministic `SQLITE_FULL` recovery. |
| `cargo test --locked -p gateway-http-actix saturated_event_queue_cannot_block_a_streaming_response` | PASS — explicit queue saturation and terminal streaming response. |
| `cargo clippy --locked -p gateway-store --all-targets --all-features -- -D warnings` | PASS. |
| `cargo clippy --locked -p gateway-http-actix --test p11_06_recovery_drills -- -D warnings` | PASS. |
| `cargo fmt --all -- --check` and `git diff --check` | PASS. |
| `CHECK_REPORT_PATH=/tmp/p11-06-full-check.md ./scripts/check.sh full` | PASS — 214 seconds total; all 21 workspace packages passed format, Clippy and tests, followed by source/crate policy, document links, tracked Secret scan, pinned quality tools, Cargo policy, and RustSec audit. |

## Independent review

- PASS — `GatedEventSource` yields its first Canonical event before blocking, and the loopback
  client receives the terminal event only after the explicit release. The `stop(true)` assertion
  checks that the server task cannot join while the active stream remains gated.
- PASS — the aborted writer has a failed pending batch, zero durable-success counters, and no
  database directory. The subsequent fresh writer receives a separately retained source clone;
  the test makes no false claim that an in-memory batch survived the abort.
- PASS — the page-limit hook and its atomic control are both inside `cfg(test)`, private to the
  writer module, and absent from a non-test library build. The direct assertion matches the
  SQLite `SQLITE_FULL` extended code before exercising the same bounded writer retry path.
- PASS — no Provider, account, credential, public endpoint, server deployment, filesystem
  exhaustion, or production configuration enters the tests or report. P11-07 owns migration
  drills and P12 retains real-service recovery proof.
