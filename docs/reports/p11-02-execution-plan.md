# P11-02 Fault Matrix execution plan

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P11-02` |
| Status | `DONE` — implementation, focused verification, and review completed; P11 Phase Gate remains pending later P11 tasks. |
| Branch | `codex/p11-release-hardening` |
| Task Card | Test-only, deterministic fault injection across the DNS-pinned upstream transport, bounded Canonical stream, and existing attempt-state matrix. |
| References | [P11 plan](../06-development-plan.md#17-p11---发布加固), [P3 transport boundary](../contracts/BC-UPSTREAM-001-dns-pinned-upstream-client-pool.md), [attempt orchestration](../contracts/BC-ROUTER-003-request-scoped-attempt-orchestration.md) |

## Required acceptance

Run a loopback-only integration matrix for direct network failure, DNS resolver failure, TLS
handshake failure, raw `429` and `5xx` responses, incomplete response stream, bounded slow
consumer, and cancellation. The matrix must prove only the owning safe boundary is affected:

- DNS/network/TLS map to egress-unavailable behavior and never trigger hidden transport retry.
- `429` and `5xx` are returned as one raw transport response; existing Attempt orchestration is
  re-run to prove target-scoped quota versus endpoint-scoped transient handling.
- A stream ending without a terminal Canonical event is exactly one `StreamTruncated` failure.
- A slow consumer applies natural bounded backpressure; cancellation unblocks a producer and
  prevents transparent retry.

## Implementation and validation sequence

1. Add an integration-only `gateway-http-actix` harness over the existing `gateway-upstream` and
   `gateway-stream` boundaries, using an injected resolver and ephemeral `127.0.0.1` listeners;
   no public endpoint, proxy, environment, credential, or server state may be read.
2. Reuse the real bounded Canonical stream for truncation, slow-client, and cancellation cases;
   do not build a parallel queue or synthetic production transport.
3. Run the focused upstream/stream suite and the existing Router tests that classify `429`, `5xx`,
   pre-semantic truncation, and an in-flight cancellation. Run format, Clippy, Secret/docs checks,
   and review before the P11-02 commit.

## Explicitly out of scope

Public-network probes, real DNS/TLS providers, proxy/TUN or server changes, Provider accounts,
load/soak benchmarks, memory profiling, SSRF/security audit, migration/recovery, P11-03 through
P11-08, and P7/P8 deferred external authentication.
