# P11-02 loopback Fault Matrix report

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P11-02` |
| Status | `DONE` — P11-02 local implementation, verification, and review are complete; the P11 Phase Gate remains pending later P11 tasks. |
| Branch | `codex/p11-release-hardening` |
| Harness | [`p11_02_fault_matrix.rs`](../../crates/gateway-http-actix/tests/p11_02_fault_matrix.rs) |
| Test boundary | Ephemeral `127.0.0.1` listeners plus injected resolver and the real bounded Canonical stream. No public DNS, external HTTP/TLS, server, proxy/TUN, environment credential, or account state. |

## Matrix

| Fault | Injection | Observed safe boundary | Retry / isolation result |
|---|---|---|---|
| Network | A previously bound loopback port is closed before dispatch. | `EgressUnavailable` / `Egress`. | One attempted dispatch ends safely; no hidden transport retry is available. |
| DNS | The injected resolver returns only `EgressDnsError`. | `DnsUnavailable` maps to `EgressUnavailable` / `Egress` before a target is admitted or dialed. | No connection target exists, so no dispatch occurs. |
| TLS | A plaintext loopback listener receives a ClientHello and closes. | `EgressUnavailable` / `Egress`. | TLS failure remains transport-owned and does not become a Credential/Account failure. |
| HTTP 429 | Loopback peer returns one `429` envelope. | Raw status is returned exactly once by the transport. | No client-level retry; the existing Router `rate_limit_records_exact_quota_and_preserves_a_healthy_sibling` fixture confirms exact Endpoint+Credential Quota ownership. |
| HTTP 5xx | Loopback peer returns one `503` envelope. | Raw status is returned exactly once by the transport. | No client-level retry; the existing Router `server_error_cools_the_endpoint_and_falls_back_to_another_candidate` fixture confirms Endpoint-scoped transient cooling/fallback. |
| Truncated stream | A bounded Canonical sender emits `ResponseStart` then closes without terminal event. | Exactly one `StreamTruncated` / `Stream`, then clean local end-of-stream. | Existing Router `pre_semantic_truncation_falls_back_but_post_fse_failure_never_retries` confirms pre/post semantic retry separation. |
| Slow client | Bounded capacity is occupied while a second canonical send is polled. | The second send waits for capacity; no unbounded queue or eager drain occurs. | Releasing one downstream event permits only the queued event to continue. |
| Cancellation | A producer is waiting on bounded capacity when the stream control is cancelled. | Producer receives `Cancelled` / `Request`; retry gate is closed. | Existing Router `cancellation_drops_an_inflight_driver_future_without_a_retry` confirms an in-flight driver is dropped without fallback. |

## Verification

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-http-actix --test p11_02_fault_matrix` | PASS — four deterministic tests cover the eight matrix rows. |
| `cargo clippy --locked -p gateway-http-actix --test p11_02_fault_matrix -- -D warnings` | PASS. |
| Four focused `gateway-router` AttemptOrchestrator tests for quota, 5xx, truncation, and cancellation | PASS. |
| `./scripts/check.sh docs`, source/crate-boundary policy, `cargo fmt --all -- --check`, `git diff --check` | PASS. |
| Focused review | PASS — confirmed exact loopback admission, injected-only DNS, direct proxy mode, no credentials/environment/server path, no transport status retry, strict stream terminal handling, bounded backpressure, cancellation retry closure, and preserved crate dependency direction. |

## Boundary and next task

This matrix verifies fault ownership and bounded recovery behavior. It is not a public-network
reliability test, a Provider protocol classifier, a performance benchmark, or a 24-hour soak.
P11-03 owns the reproducible mock-provider benchmark and approved regression thresholds.
