# BC-DELIVERY-001: Delivery gates and authorized single-probe diagnostic

- Task: `P4-00`
- Change Request: `CR-EXEC-001`
- ADR: [ADR-0021](../adr/ADR-0021-delivery-gate-classification-and-single-probe-diagnostic.md)

## Entry points

- `scripts/classify-ci-change.sh` classifies GitHub path scope.
- `.github/workflows/ci.yml` selects `Docs-only gate` or `Fast gate` plus `Full supply-chain gate`.
- `scripts/check-plan-state.rb` validates task-state and dependency transitions.
- `crates/gateway-http-actix/tests/p4_00_authorized_single_probe_diagnostic.rs` is the ignored
  test-only single-probe diagnostic.

## Preconditions

1. A docs-only candidate changes only `README.md` and/or Markdown below `docs/`; any unknown,
   code, workflow, lockfile, script, fixture, security-policy, or deletion path is code scope.
2. A diagnostic invocation must supply its dedicated `P4_00_DIAGNOSTIC_*` configuration through an
   operator-controlled ignored file or current process environment. It never reads generic
   provider variables or a `.env` file.
3. The diagnostic authorization value must exactly match its dedicated sentinel; its declared
   `MAX_EXTERNAL_REQUESTS` must be exactly `1`; target label, mode, endpoint, credential, model,
   profile, and optional narrow CIDR must all be explicit.

## Delivery-gate sequence and invariants

| Step | Required behavior |
|---|---|
| Classify | `workflow_dispatch`, tags, empty/unknown diffs, and every non-doc path select `code`; deletion is never ignored. |
| Docs scope | Run document links, plan-state validation, tracked Secret scan, and tracked-file whitespace check. |
| Code scope | Run Fast, then Full only after Fast succeeds. Full runs the existing version-verifying quality installer and supply-chain checks. |
| Required job | Pass only when classifier passes and exactly the selected gate set succeeds; skipped jobs are asserted rather than silently ignored. |
| Cache | Cache key includes OS, pinned Rust version, and quality-tool version file. Cache hit still runs tool version checks; mismatch installs with `--locked`. |
| Plan state | At most one Task is `IN_PROGRESS`; active tasks require completed explicit Task dependencies; P4-01..P4-09 cannot activate before P4-00 is `DONE`. |

## Single-probe sequence and invariants

1. Missing or incorrect authorization stops after reading only the authorization variable. No URL,
   endpoint, DNS policy, transport profile, credential header, or outbound request is built.
2. After authorization and cap validation, configuration creates exactly one endpoint and one
   request mode: `non_streaming` or `sse`. `direct` rejects even a leftover SOCKS value; `socks5`
   accepts only the existing local-DNS SOCKS5 parser.
3. The exact request receives P2 egress admission immediately before one call to
   `UpstreamClientPool::send`. It has no retry loop, candidate selection, or failover. The client
   transport itself disables automatic redirects, system proxies, and client retries.
4. Connect, time-to-first-byte, idle, and total timeouts are finite (`5s`, `15s`, `45s`, `45s`),
   the response content type must match the selected mode, and chunks are discarded after a finite
   64 KiB cumulative cap.
5. Console output contains only opaque target label, mode, and safe result category. It never logs
   endpoint URL, credential, upstream model, request body, response body, or raw frame.

## Error semantics

| Condition | Safe result |
|---|---|
| Missing/incorrect authorization | `NotAuthorized`; zero diagnostic setup and zero HTTP request. |
| Cap, mode, target label, profile, proxy, endpoint, credential, CIDR, or timeout invalid | Configuration failure without printing the supplied value. |
| Egress admission or transport failure | `egress_admission_failed` or `transport_failed`. |
| Non-2xx response | HTTP status class only (`1xx`, `3xx`, `4xx`, `5xx`, or `other`). |
| Wrong content type, read failure, or body cap exceeded | Named bounded diagnostic outcome with no body rendering. |

## Corresponding tests

- `scripts/test-ci-change-classifier.sh`
- `scripts/test-plan-state-check.sh`
- `scripts/test-install-quality-tools.sh`
- `cargo test --locked -p gateway-http-actix --test p4_00_authorized_single_probe_diagnostic`
- `./scripts/check.sh fast` and `./scripts/check.sh full`
