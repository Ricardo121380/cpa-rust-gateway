# ADR-0019 Controlled Mock HTTP aggregation E2E

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-20` |
| Task / Matrix / Contract references | `P3-09`; `C16`, `G05`, `G12-G15`, `G21`, `K03-K06`, `L20-L31`; [BC-E2E-001](../contracts/BC-E2E-001-controlled-mock-http-aggregation-e2e.md) |

## Context

P3-01 through P3-08 independently validate request assembly, DNS-pinned transport, scheduling,
Credential leases, runtime health, bounded pre-first-semantic Attempt retry, Snapshot model
resolution, and non-blocking lifecycle events. They intentionally did not compose those pieces
with a real HTTP peer. A synthetic Driver alone cannot prove that a selected Route sends the exact
upstream model to the exact admitted target, that an HTTP 5xx reaches the Attempt decision, or that
dropping a downstream SSE body closes an in-flight upstream body.

The validation must stay entirely local. It must neither load a Repository nor use a deployed
Endpoint, production Client Key, production upstream Credential, ambient proxy, or unrestricted
loopback access.

## Decision

1. The Router exposes `ResponsesExecution`, a transport-neutral execution handoff with the
   Canonical request, Snapshot-resolved Route ID, client-visible response mode, and the exact
   downstream-owned `TransparentRetryGate`. `ResponsesExecutor::execute_routed` has a legacy
   default, preserving existing P1 embeddings that only implement `execute`.
2. The Actix Responses handler allocates its finite Canonical stream before it invokes the executor.
   The executor therefore observes the same cancellation/FSE gate that the eventual HTTP body owns;
   it never imports `gateway-stream` or an Actix type.
3. P3-09 adds a test-only composition harness in `gateway-http-actix/tests`. Its two independent
   `TcpListener` peers are reached only through a P2 `EgressPolicy` allowing their exact synthetic
   host, port, and `127.0.0.1/32` address. The harness uses the real P3-01 builder, P3-02 client
   pool, P3-04 lease, P3-06 orchestrator, P3-07 Snapshot auth/model projection, and P3-08 sink.
4. The harness accepts only a deliberately small OpenAI-compatible Responses JSON/SSE subset needed
   for the controlled fixtures. Request bodies, full response bodies, and SSE frames have explicit
   finite limits; test records retain only selected upstream model labels and counters.
5. Required integration scenarios are equal-priority round-robin across two peers, HTTP 5xx before
   `ResponseStart` falling through to the second peer, and dropping an unconsumed SSE body closing
   the active peer without contacting a fallback.

## Consequences

The test proves the existing P3 contracts compose without broadening the production HTTP crate's
library dependency graph to concrete Provider types. Its `dev-dependencies` are test-only and are
explicitly called out in the crate-boundary documentation.

This is not a general-purpose production OpenAI response decoder, persistent event writer, route
explain API, real Endpoint probe, or server deployment. P3-10 owns separately authorized real-test
Endpoint validation; P4 owns durable observability and richer response/status policy.

## Validation and rollback

The E2E target runs entirely against per-test loopback listeners and aborts them on test teardown.
Removing the test target and the optional execution handoff reverts the composition seam; no
database, network configuration, service process, or external account state is mutated.
