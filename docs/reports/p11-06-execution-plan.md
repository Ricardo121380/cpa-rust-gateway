# P11-06 Recovery Drills execution plan

| Field | Value |
|---|---|
| Plan version | `v1.45` |
| Task | `P11-06` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Branch | `codex/p11-release-hardening` |
| Task Card | Prove, with only ephemeral loopback HTTP and temporary SQLite files, that a graceful stop drains an already-started stream, a crashed writer has a precise replay boundary, SQLite-full retention recovers after capacity returns, and bounded event-queue pressure does not stall inference. |
| References | [P11 plan](../06-development-plan.md#17-p11---发布加固), [P11-02 Fault Matrix](p11-02-fault-matrix.md), [P4 event-writer contract](../contracts/BC-OBS-002-append-only-sqlite-event-writer.md), [P4 event-writer report](p4-07-append-only-sqlite-event-writer.md) |

## Required acceptance

1. A real ephemeral `127.0.0.1` Actix listener accepts one authenticated SSE response, begins its
   Canonical lifecycle, receives `ServerHandle::stop(true)`, and remains alive until the test
   releases the finite stream. The client must receive a terminal `response.completed` event and
   the server task must join. This is a process-local lifecycle proof, not a production or
   systemd shutdown claim.
2. An injected writer crash while its sole finite pending Required batch cannot be persisted shows
   no fabricated commit. A fresh writer replays the retained source event, persists it once, and a
   reopened store passes `quick_check`. The report must state that pre-commit in-memory data is not
   crash-durable until replayed.
3. A temporary SQLite database constrained with `PRAGMA max_page_count` causes a real SQLite
   full-write failure. The writer retains its bounded batch and records no commit; expanding the
   temporary database limit lets the same batch finish, with no duplicate rows and a passing
   `quick_check`.
4. The existing HTTP event-queue saturation regression remains green: full Required admission is
   explicit and non-blocking, while the streaming response still reaches its terminal event.

## Implementation and validation sequence

1. Add one test-only loopback HTTP recovery harness under `gateway-http-actix/tests`, using a
   gated deterministic `ResponsesExecutor`, synthetic Client Key, and raw local `TcpStream`. It
   reads no environment/configuration and sends no request beyond its own listener.
2. Extend the `gateway-store` writer unit coverage with a temporary `max_page_count` disk-full
   drill and an abort/restart replay drill. Keep the public production writer API unchanged; use
   no host-disk exhaustion, mounted volume, subprocess, or external service.
3. Run the named P11-06 tests plus the pre-existing saturated-queue regression, format and
   crate-focused Clippy. Then run the task's required local Full gate once after focused checks
   pass.
4. Write `p11-06-recovery-report.md`, independently review each claim against the test code and
   receipt, change P11-06 to `LOCAL_PASS_PENDING_PHASE_GATE`, and commit. Completed: the local
   Full gate and docs-only closeout both pass; P11-07 may now start on its own execution plan.

## Explicitly out of scope

No Provider, OAuth, API key, endpoint, public DNS/TLS, proxy/TUN, deployed server, systemd,
container restart, filesystem exhaustion, production database, migration/downgrade, or release
packaging is used. P11-07 owns migration/downgrade/rollback drills; P12 owns a real server,
service manager, deployment, and production recovery behavior.
